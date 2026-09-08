use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{connections::Connection, engine::Engine};
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;
async fn request(state: &AppState, path: &str, body: Value) -> (StatusCode, Value) {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(
                    "authorization",
                    "Bearer test-connection-token-32-characters",
                )
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[test]
fn connector_diagnostics_remove_exact_secret_values_even_without_sensitive_field_names() {
    use polymas_core::secrets::Secret;
    let credentials = polymas_daemon::connection_api::Credentials {
        bearer: Some(Secret::new("fixture-bearer-123".into()).unwrap()),
        environment: [(
            "DB_PASSWORD".into(),
            Secret::new("fixture-password-456".into()).unwrap(),
        )]
        .into(),
    };
    let output = credentials
        .diagnostic("Server says fixture-bearer-123 and fixture-password-456 were rejected");
    assert!(!output.contains("fixture-bearer-123"));
    assert!(!output.contains("fixture-password-456"));
}
#[tokio::test]
async fn a_file_connection_must_be_probed_before_enabling_and_stale_edits_cannot_restore_access() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(
        Engine::open(dir.path()).unwrap(),
        "test-connection-token-32-characters".into(),
        vec![],
    )
    .unwrap();
    let files = tempfile::tempdir().unwrap();
    std::fs::write(files.path().join("note.txt"), "Fixture knowledge").unwrap();
    let connection = Connection::files("Files", vec![files.path().into()]);
    let id = connection.id.clone();
    let (status, saved) = request(
        &state,
        "/api/connections/save",
        json!({"revision":0,"connection":connection}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["value"]["health"], "unconfigured");
    let (status, _) = request(
        &state,
        &format!("/api/connections/{id}/toggle"),
        json!({"revision":1,"enabled":true}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, probed) = request(
        &state,
        &format!("/api/connections/{id}/probe"),
        json!({"revision":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(probed["connection"]["value"]["health"], "ready");
    assert_eq!(
        probed["connection"]["value"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let (status, _) = request(
        &state,
        &format!("/api/connections/{id}/tools"),
        json!({"revision":2,"tools":["read_text","list_files"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &state,
        &format!("/api/connections/{id}/toggle"),
        json!({"revision":3,"enabled":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &state,
        &format!("/api/connections/{id}/toggle"),
        json!({"revision":4,"enabled":false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(
        &state,
        &format!("/api/connections/{id}/toggle"),
        json!({"revision":3,"enabled":true}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        !state
            .engine
            .lock()
            .await
            .workspace
            .connection(&id)
            .unwrap()
            .value
            .enabled
    );
}
