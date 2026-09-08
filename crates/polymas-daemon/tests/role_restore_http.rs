use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{engine::Engine, registry::RoleDraft, workspace::ConfigCommand};
use polymas_daemon::{AppState, router};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn restoration_uses_owner_and_origin_guards_and_stale_retry_cannot_add_another_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let original = RoleDraft {
        name: "Restore source".into(),
        instruction: "Saved original".into(),
        ..Default::default()
    };
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: original.clone(),
            },
        )
        .unwrap();
    let role_id = role["id"].as_str().unwrap().to_string();
    engine
        .configure(
            2,
            ConfigCommand::ReviseRole {
                role_id: role_id.clone(),
                expected_version: 1,
                draft: RoleDraft {
                    instruction: "Second instruction".into(),
                    ..original.clone()
                },
            },
        )
        .unwrap();
    let token = "role-restore-owner-fixture-32-characters";
    let state = AppState::new(engine, token.into(), vec!["http://127.0.0.1:1420".into()]).unwrap();
    let app = router(state.clone());
    let command = json!({"revision":3,"command":{"type":"restore_role","roleId":role_id,"sourceVersion":1,"expectedVersion":2}});
    let request = |credential: &str, origin: &str| {
        Request::builder()
            .uri("/api/configure")
            .method("POST")
            .header("authorization", format!("Bearer {credential}"))
            .header("origin", origin)
            .header("content-type", "application/json")
            .body(Body::from(command.to_string()))
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
    assert_eq!(
        app.clone()
            .oneshot(request("wrong-fixture", "http://127.0.0.1:1420"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(request(token, "https://foreign.invalid"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
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
    let response = app
        .clone()
        .oneshot(request(token, "http://127.0.0.1:1420"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["revision"], 4);
    assert_eq!(value["result"]["sourceVersion"], 1);
    assert_eq!(value["result"]["role"]["version"], 3);
    assert_eq!(value["result"]["role"]["instruction"], "Saved original");
    assert_eq!(
        app.oneshot(request(token, "http://127.0.0.1:1420"))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let engine = state.engine.lock().await;
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head + 1);
    assert_eq!(engine.workspace.config().value.registry.roles.len(), 3);
    assert_eq!(
        engine
            .workspace
            .config()
            .value
            .registry
            .role(&role_id, 3)
            .unwrap()
            .draft,
        original
    );
}
