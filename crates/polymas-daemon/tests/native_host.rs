use polymas_desktop_host::service::RunningService;
use serde_json::json;

#[tokio::test]
#[ignore = "Opt-in: writes then removes an isolated fixture in the real OS credential vault"]
async fn native_profile_reconnects_through_the_real_os_vault() {
    let dir = tempfile::tempdir().unwrap();
    let namespace = format!("native-smoke-{}", uuid::Uuid::new_v4());
    let vault = polymas_core::secrets::OsVault::new(&namespace).unwrap();
    let profile_dir = dir.path().join("profile");
    let workspace = dir.path().join("workspace");
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_polymas-daemon"));
    let first = polymas_desktop_host::profile::NativeProfile::connect(
        &profile_dir,
        &workspace,
        executable,
        1,
        &vault,
    )
    .await
    .unwrap();
    let pid = first.record.pid;
    let reference = first.record.credential_ref.clone();
    let cleanup_connection = first.connection.clone();
    drop(first);
    let second = polymas_desktop_host::profile::NativeProfile::connect(
        &profile_dir,
        &workspace,
        executable,
        1,
        &polymas_core::secrets::OsVault::new(&namespace).unwrap(),
    )
    .await;
    let second = match second {
        Ok(second) => second,
        Err(error) => {
            cleanup_connection
                .request("/api/control", Some(json!({"action":"shutdown"})))
                .await
                .ok();
            vault.remove(&reference).ok();
            panic!("Reconnect failed: {error}");
        }
    };
    let reused = second.reused && second.record.pid == pid;
    let result = second.stop().await;
    let cleanup = vault.remove(&reference);
    result.unwrap();
    cleanup.unwrap();
    assert!(reused);
}
#[tokio::test]
async fn service_startup_expires_without_a_durable_desktop_adoption() {
    use tokio::io::AsyncBufReadExt;
    let dir = tempfile::tempdir().unwrap();
    let mut engine = polymas_core::engine::Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            polymas_core::workspace::ConfigCommand::CreateRole {
                draft: polymas_core::registry::RoleDraft {
                    name: "Queued".into(),
                    instruction: "Preserve pending work".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            polymas_core::workspace::ConfigCommand::CreateAgents {
                key: "one".into(),
                request: polymas_core::registry::BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "Queued".into(),
                },
            },
        )
        .unwrap();
    let run = engine
        .submit(
            "pending",
            polymas_core::scheduler::TaskInput {
                prompt: "Do not start before adoption".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    drop(engine);
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_polymas-daemon"));
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command
        .args([
            "--workspace",
            dir.path().to_str().unwrap(),
            "--port",
            "0",
            "--adoption-timeout",
            "2",
        ])
        .env(
            "POLYMAS_CONTROL_TOKEN",
            "startup-expiration-fixture-token-32",
        )
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut reader = tokio::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    assert!(
        line.contains("ready"),
        "service should announce an endpoint before waiting for adoption"
    );
    let status = tokio::time::timeout(std::time::Duration::from_secs(8), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success(), "unadopted startup must stop cleanly");
    let engine = polymas_core::engine::Engine::open(dir.path()).unwrap();
    assert_eq!(
        engine.scheduler.task(&run.task_ids[0]).unwrap().state,
        polymas_core::scheduler::TaskState::Queued,
        "an abandoned startup must preserve pending user work"
    );
}
#[derive(Default)]
struct MemoryVault(std::sync::Mutex<std::collections::BTreeMap<String, String>>);
impl polymas_desktop_host::service::CredentialVault for MemoryVault {
    fn remove(&self, key: &str) -> polymas_core::error::Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    fn save(&self, secret: &polymas_core::secrets::Secret) -> polymas_core::error::Result<String> {
        let key = uuid::Uuid::new_v4().to_string();
        self.0
            .lock()
            .unwrap()
            .insert(key.clone(), secret.expose().into());
        Ok(key)
    }
    fn load(&self, key: &str) -> polymas_core::error::Result<polymas_core::secrets::Secret> {
        polymas_core::secrets::Secret::new(
            self.0
                .lock()
                .unwrap()
                .get(key)
                .ok_or_else(|| polymas_core::error::Error::Denied("No credential".into()))?
                .clone(),
        )
    }
}
#[tokio::test]
async fn durable_profile_reuses_the_same_daemon_and_keeps_credentials_in_the_vault() {
    use polymas_desktop_host::profile::NativeProfile;
    let dir = tempfile::tempdir().unwrap();
    let profile = dir.path().join("profile");
    let workspace = dir.path().join("workspace");
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_polymas-daemon"));
    let vault = MemoryVault::default();
    let first = NativeProfile::connect(&profile, &workspace, executable, 1, &vault)
        .await
        .unwrap();
    let pid = first.record.pid;
    assert!(!first.reused);
    drop(first);
    let second = NativeProfile::connect(&profile, &workspace, executable, 1, &vault)
        .await
        .unwrap();
    let reused = second.reused;
    let second_pid = second.record.pid;
    let state = second.connection.request("/api/state", None).await.unwrap();
    second.stop().await.unwrap();
    let reopened = polymas_core::workspace::Workspace::open(&workspace).unwrap();
    drop(reopened);
    let secrets: Vec<_> = vault.0.lock().unwrap().values().cloned().collect();
    drop(second);
    assert!(reused);
    assert_eq!(pid, second_pid);
    assert_eq!(
        secrets.len(),
        1,
        "reconnect must not create a new credential"
    );
    assert_eq!(state["config"]["revision"], 1);
    for file in std::fs::read_dir(&profile)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
    {
        let bytes = std::fs::read(file.path()).unwrap();
        for secret in &secrets {
            assert!(!bytes.windows(secret.len()).any(|w| w == secret.as_bytes()));
        }
    }
}
#[tokio::test]
async fn a_serialized_service_record_reconnects_without_storing_its_credential() {
    let dir = tempfile::tempdir().unwrap();
    let service = RunningService::launch(
        std::path::Path::new(env!("CARGO_BIN_EXE_polymas-daemon")),
        dir.path(),
        1,
    )
    .await
    .unwrap();
    let vault = MemoryVault::default();
    let record = service.remember(&vault).unwrap();
    let text = serde_json::to_string(&record).unwrap();
    for secret in vault.0.lock().unwrap().values() {
        assert!(!text.contains(secret));
    }
    let restored: polymas_desktop_host::service::ServiceRecord =
        serde_json::from_str(&text).unwrap();
    drop(service);
    let connection = restored.reconnect(&vault).await.unwrap();
    let mut wrong = restored.clone();
    wrong.workspace_id = "other-workspace".into();
    assert!(wrong.reconnect(&vault).await.is_err());
    assert!(restored.reconnect(&MemoryVault::default()).await.is_err());
    connection
        .request("/api/control", Some(json!({"action":"shutdown"})))
        .await
        .unwrap();
}
#[tokio::test]
async fn native_launcher_owns_a_real_service_and_reopening_preserves_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_polymas-daemon"));
    let service = RunningService::launch(executable, dir.path(), 1)
        .await
        .unwrap();
    let connection = service.connection();
    let state = connection.request("/api/state", None).await.unwrap();
    let workspace_id = state["config"]["value"]["id"].clone();
    connection.request("/api/configure",Some(json!({"revision":1,"command":{"type":"rename_workspace","name":"Native workspace"}}))).await.unwrap();
    assert!(
        RunningService::launch(executable, dir.path(), 1)
            .await
            .is_err(),
        "a second launcher cannot own the same workspace"
    );
    service.shutdown().await.unwrap();
    let reopened = RunningService::launch(executable, dir.path(), 1)
        .await
        .unwrap();
    let state = reopened
        .connection()
        .request("/api/state", None)
        .await
        .unwrap();
    assert_eq!(state["config"]["value"]["id"], workspace_id);
    assert_eq!(state["config"]["value"]["name"], "Native workspace");
    reopened.shutdown().await.unwrap();
}
#[tokio::test]
async fn dropping_the_window_host_keeps_the_service_available_until_explicit_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let service = RunningService::launch(
        std::path::Path::new(env!("CARGO_BIN_EXE_polymas-daemon")),
        dir.path(),
        1,
    )
    .await
    .unwrap();
    let connection = service.connection();
    drop(service);
    assert!(connection.request("/api/state", None).await.is_ok());
    connection
        .request("/api/control", Some(json!({"action":"shutdown"})))
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if connection.request("/api/state", None).await.is_err() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
}
