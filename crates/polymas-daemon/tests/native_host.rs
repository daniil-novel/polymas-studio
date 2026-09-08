use polymas_desktop_host::service::RunningService;
use serde_json::json;
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
