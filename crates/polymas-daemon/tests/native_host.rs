use polymas_desktop_host::service::RunningService;
use serde_json::json;
#[derive(Default)]
struct MemoryVault(std::sync::Mutex<std::collections::BTreeMap<String, String>>);
impl polymas_desktop_host::service::CredentialVault for MemoryVault {
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
