use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    scheduler::TaskInput,
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, gateway::issue, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const CONTROL: &str = "test-gateway-control-token-32-characters";
async fn setup() -> (tempfile::TempDir, AppState, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Reader".into(),
                    instruction: "Read assigned sources".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "agents".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 2,
                    name_pattern: "Reader {n}".into(),
                },
            },
        )
        .unwrap();
    let run = engine
        .submit(
            "run",
            TaskInput {
                prompt: "Read a source".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap();
    let task = run.task_ids[0].clone();
    (
        dir,
        AppState::new(engine, CONTROL.into(), vec![]).unwrap(),
        task,
    )
}
async fn request(
    state: &AppState,
    token: &str,
    path: &str,
    origin: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .uri(path)
        .header("authorization", format!("Bearer {token}"));
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    let response = router(state.clone())
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn gateway_credentials_are_attempt_scoped_and_never_control_credentials() {
    let (_dir, state, task_id) = setup().await;
    let grant = issue(&state, &task_id).await.unwrap();
    let token = grant.token.expose().to_owned();
    let (status, context) = request(&state, &token, "/gateway/context", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(context["taskId"], task_id);
    assert_eq!(
        request(&state, CONTROL, "/gateway/context", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&state, &token, "/api/state", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &state,
            &token,
            "/gateway/context",
            Some("http://127.0.0.1:1420")
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let engine = state.engine.lock().await;
    let events = engine.workspace.db.events(0, 5000, None).unwrap();
    assert!(!json!(events).to_string().contains(&token));
    drop(engine);
    drop(grant);
    assert_eq!(
        request(&state, &token, "/gateway/context", None).await.0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn finishing_or_cancelling_a_task_revokes_access_before_guard_cleanup() {
    let (_dir, state, task_id) = setup().await;
    let grant = issue(&state, &task_id).await.unwrap();
    let mut engine = state.engine.lock().await;
    let epoch = engine
        .scheduler
        .task(&task_id)
        .unwrap()
        .lease
        .as_ref()
        .unwrap()
        .epoch;
    engine.finish(&task_id, epoch, Ok("done".into())).unwrap();
    drop(engine);
    assert_eq!(
        request(&state, grant.token.expose(), "/gateway/context", None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert!(issue(&state, &task_id).await.is_err());
}
