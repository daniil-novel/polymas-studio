use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, router};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn batch_archive_is_owner_only_and_a_stale_repeat_emits_no_second_event() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Role".into(),
                    instruction: "Work".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let ids = engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "two".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 2,
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap()["ids"]
        .clone();
    let state = AppState::new(
        engine,
        "bulk-archive-owner-fixture-32-characters".into(),
        vec![],
    )
    .unwrap();
    let app = router(state.clone());
    let body = json!({"revision":3,"command":{"type":"archive_agents","agentIds":ids}}).to_string();
    let request = |token: &str| {
        Request::builder()
            .uri("/api/configure")
            .method("POST")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap()
    };
    let before = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    let denied = app
        .clone()
        .oneshot(request("untrusted-fixture-token"))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .last_sequence()
            .unwrap(),
        before
    );
    let response = app
        .clone()
        .oneshot(request("bulk-archive-owner-fixture-32-characters"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value, json!({"revision":4,"result":{"ids":ids}}));
    let conflict = app
        .oneshot(request("bulk-archive-owner-fixture-32-characters"))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let engine = state.engine.lock().await;
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), before + 1);
    assert!(
        engine
            .workspace
            .config()
            .value
            .registry
            .agents
            .iter()
            .all(|agent| agent.status == polymas_core::registry::AgentStatus::Archived)
    );
}
