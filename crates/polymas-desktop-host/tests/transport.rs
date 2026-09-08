use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    response::Redirect,
    routing::{get, post},
};
use polymas_core::secrets::Secret;
use polymas_desktop_host::NativeConnection;
use serde_json::{Value, json};
const TOKEN: &str = "fixture-native-control-credential-73";
async fn server() -> (u16, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/api/check",post(|headers:HeaderMap,Json(body):Json<Value>|async move {Json(json!({"authorized":headers.get("authorization").unwrap()==format!("Bearer {TOKEN}").as_str(),"body":body}))}))
        .route("/api/redirect",get(||async {Redirect::temporary("/api/private")}))
        .route("/api/private",get(||async {Json(json!({"leaked":true}))}))
        .route("/api/error",get(||async {(StatusCode::CONFLICT,Json(json!({"code":"conflict","message":format!("Stale revision {TOKEN}")})))}))
        .route("/api/large",get(||async {"x".repeat(16*1024*1024+1)}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (
        port,
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }),
    )
}
#[tokio::test]
async fn native_commands_cannot_change_authority_or_escape_the_api_path() {
    let (port, service) = server().await;
    let connection = NativeConnection::new(port, Secret::new(TOKEN.into()).unwrap()).unwrap();
    for path in [
        "https://example.org/api/check",
        "//example.org/api/check",
        "/gateway/context",
        "/api/../gateway/context",
        "/api/%2e%2e/gateway/context",
        "/api/check#ignored",
        "/api/check\\evil",
    ] {
        assert_eq!(
            connection.request(path, None).await.unwrap_err().code,
            "validation"
        );
    }
    let response = connection
        .request("/api/check?limit=10", Some(json!({"key":"value"})))
        .await
        .unwrap();
    assert_eq!(response, json!({"authorized":true,"body":{"key":"value"}}));
    service.abort();
}
#[tokio::test]
async fn native_bridge_preserves_conflicts_redacts_credentials_and_refuses_redirects() {
    let (port, service) = server().await;
    let connection = NativeConnection::new(port, Secret::new(TOKEN.into()).unwrap()).unwrap();
    let error = connection.request("/api/error", None).await.unwrap_err();
    assert_eq!(error.code, "conflict");
    assert_eq!(error.status, 409);
    assert!(!error.message.contains(TOKEN));
    assert!(connection.request("/api/redirect", None).await.is_err());
    assert!(connection.request("/api/large", None).await.is_err());
    service.abort();
}
