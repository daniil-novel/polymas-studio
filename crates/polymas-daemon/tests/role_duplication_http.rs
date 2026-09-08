use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{engine::Engine, registry::RoleDraft, workspace::ConfigCommand};
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn only_the_owner_can_duplicate_and_retrying_a_saved_request_does_not_copy_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let source = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Source".into(),
                    instruction: "Immutable saved source".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let token = "role-duplication-owner-fixture-32-characters";
    let state = AppState::new(engine, token.into(), vec!["http://127.0.0.1:1420".into()]).unwrap();
    let app = router(state.clone());
    let command = json!({"revision":2,"command":{"type":"duplicate_role","key":"http-copy","roleId":source["id"],"sourceVersion":1,"name":"New role"}});
    let request = |credential: &str, origin: &str, body: &Value| {
        Request::builder()
            .uri("/api/configure")
            .method("POST")
            .header("authorization", format!("Bearer {credential}"))
            .header("origin", origin)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let head = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    for (credential, origin) in [
        ("wrong-fixture", "http://127.0.0.1:1420"),
        (token, "https://foreign.invalid"),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(credential, origin, &command))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .last_sequence()
            .unwrap(),
        head
    );
    let first = app
        .clone()
        .oneshot(request(token, "http://127.0.0.1:1420", &command))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first: Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first["revision"], 3);
    assert_ne!(first["result"]["role"]["id"], source["id"]);
    assert_eq!(first["result"]["role"]["name"], "New role");
    assert_eq!(
        first["result"]["role"]["instruction"],
        source["instruction"]
    );
    assert_eq!(first["result"]["sourceRoleId"], source["id"]);
    assert_eq!(first["result"]["sourceVersion"], 1);
    let retry = app
        .clone()
        .oneshot(request(token, "http://127.0.0.1:1420", &command))
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    let retry: Value =
        serde_json::from_slice(&retry.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(retry, first);
    let mut reused = command.clone();
    reused["revision"] = json!(3);
    reused["command"]["name"] = json!("Changed request");
    assert_eq!(
        app.clone()
            .oneshot(request(token, "http://127.0.0.1:1420", &reused))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let mut stale = command.clone();
    stale["command"]["key"] = json!("another-copy");
    assert_eq!(
        app.clone()
            .oneshot(request(token, "http://127.0.0.1:1420", &stale))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let mut injected = command;
    injected["command"]["draft"] = json!({"instruction":"Injected"});
    assert_eq!(
        app.oneshot(request(token, "http://127.0.0.1:1420", &injected))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let engine = state.engine.lock().await;
    assert_eq!(engine.workspace.config().revision, 3);
    assert_eq!(engine.workspace.config().value.registry.roles.len(), 2);
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head + 1);
}
