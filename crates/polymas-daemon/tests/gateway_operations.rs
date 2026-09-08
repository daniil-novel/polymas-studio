#[path = "support/approval_fixture.rs"]
mod fixture;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    engine::OperationRequest,
    policy::{Approval, ApprovalLedger},
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, gateway::issue, router};
use serde_json::{Value, json};
use tower::ServiceExt;
const OWNER: &str = "operation-fixture-owner-token-32-characters";
fn input(request: &OperationRequest) -> Value {
    json!({"idempotencyKey":"fixture-write", "connectionId":request.connection_id,"namespace":request.namespace,"tool":request.tool,"arguments":request.arguments})
}
async fn call(state: &AppState, token: &str, name: &str, arguments: Value) -> Value {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/gateway/call")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":name,"arguments":arguments}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn content(value: &Value) -> Value {
    assert_eq!(value["isError"], false, "{value}");
    serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[tokio::test]
async fn request_is_scoped_retryable_and_status_follows_only_the_reserved_continuation() {
    let (_dir, engine, request) = fixture::setup();
    let state = AppState::new(engine, OWNER.into(), vec![]).unwrap();
    let grant = issue(&state, &request.task_id).await.unwrap();
    let first = content(
        &call(
            &state,
            grant.token.expose(),
            "polymas_request_operation",
            input(&request),
        )
        .await,
    );
    assert_eq!(first["state"], "operation_pending");
    let id = first["approvalId"].as_str().unwrap();
    let repeated = content(
        &call(
            &state,
            grant.token.expose(),
            "polymas_request_operation",
            input(&request),
        )
        .await,
    );
    assert_eq!(first, repeated);
    let mut changed = input(&request);
    changed["arguments"]["text"] = json!("OTHER");
    assert_eq!(
        call(
            &state,
            grant.token.expose(),
            "polymas_request_operation",
            changed
        )
        .await["isError"],
        true
    );
    let status = content(
        &call(
            &state,
            grant.token.expose(),
            "polymas_operation_status",
            json!({"approvalId":id}),
        )
        .await,
    );
    assert_eq!(status["state"], "pending");
    assert_eq!(status["canExecute"], false);
    let mut engine = state.engine.lock().await;
    let approval = engine
        .workspace
        .db
        .get::<Approval>("approvals", id)
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(approval.operation.origin_epoch, request.origin_epoch);
    assert!(approval.operation.epoch > request.origin_epoch);
    assert_eq!(approval.operation.arguments, request.arguments);
    let mut other = approval.operation.clone();
    other.task_id = "other-task".into();
    let foreign = ApprovalLedger::request(
        &mut engine.workspace.db,
        other,
        chrono::Utc::now().timestamp(),
        600,
    )
    .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Waiting".into())
        .unwrap();
    engine
        .decide_approval(
            id,
            1,
            &approval.fingerprint,
            false,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    engine
        .wake_approvals(chrono::Utc::now().timestamp())
        .unwrap();
    let resumed = engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap()
        .remove(0);
    assert_eq!(resumed.lease.unwrap().epoch, approval.operation.epoch);
    drop(engine);
    drop(grant);
    let next = issue(&state, &request.task_id).await.unwrap();
    let status = content(
        &call(
            &state,
            next.token.expose(),
            "polymas_operation_status",
            json!({"approvalId":id}),
        )
        .await,
    );
    assert_eq!(status["state"], "denied");
    assert_eq!(status["canExecute"], false);
    assert_eq!(
        call(
            &state,
            next.token.expose(),
            "polymas_operation_status",
            json!({"approvalId":foreign.id})
        )
        .await["isError"],
        true
    );
    let engine = state.engine.lock().await;
    let events = engine
        .workspace
        .db
        .events(0, 200, Some(&resumed.run_id))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.input.kind == "approval.requested")
            .count(),
        2
    );
    assert!(!json!(events).to_string().contains("APPROVAL-FIXTURE-73"));
}
#[tokio::test]
async fn wait_and_operation_requests_are_mutually_exclusive_in_either_order() {
    for wait_first in [true, false] {
        let (_dir, engine, request) = fixture::setup();
        let state = AppState::new(engine, OWNER.into(), vec![]).unwrap();
        let grant = issue(&state, &request.task_id).await.unwrap();
        let wait = (
            "polymas_wait_for_message",
            json!({"timeoutSeconds":60,"summary":"Waiting for input"}),
        );
        let operation = ("polymas_request_operation", input(&request));
        let (first, second) = if wait_first {
            (wait, operation)
        } else {
            (operation, wait)
        };
        assert_eq!(
            call(&state, grant.token.expose(), first.0, first.1).await["isError"],
            false
        );
        assert_eq!(
            call(&state, grant.token.expose(), second.0, second.1).await["isError"],
            true
        );
        let engine = state.engine.lock().await;
        assert_eq!(
            ApprovalLedger::for_attempt(
                &engine.workspace.db,
                &request.task_id,
                request.origin_epoch
            )
            .unwrap()
            .is_some(),
            !wait_first
        );
        assert_eq!(
            engine
                .scheduler
                .task(&request.task_id)
                .unwrap()
                .continuation
                .is_some(),
            !wait_first
        );
    }
}
#[tokio::test]
async fn forged_identity_or_revoked_binding_never_creates_a_reservation() {
    let (_dir, engine, request) = fixture::setup();
    let state = AppState::new(engine, OWNER.into(), vec![]).unwrap();
    let grant = issue(&state, &request.task_id).await.unwrap();
    for (field, value) in [
        ("taskId", json!("other")),
        ("epoch", json!(999)),
        ("resourceScopeHash", json!("fake")),
        ("allow", json!(true)),
    ] {
        let mut forged = input(&request);
        forged[field] = value;
        assert_eq!(
            call(
                &state,
                grant.token.expose(),
                "polymas_request_operation",
                forged
            )
            .await["isError"],
            true
        );
    }
    let mut engine = state.engine.lock().await;
    let revision = engine.workspace.config().revision;
    engine
        .configure(revision, ConfigCommand::SetBindings { bindings: vec![] })
        .unwrap();
    drop(engine);
    assert_eq!(
        call(
            &state,
            grant.token.expose(),
            "polymas_request_operation",
            input(&request)
        )
        .await["isError"],
        true
    );
    let engine = state.engine.lock().await;
    assert!(
        engine
            .scheduler
            .task(&request.task_id)
            .unwrap()
            .continuation
            .is_none()
    );
    assert!(
        engine
            .workspace
            .db
            .list::<Approval>("approvals", 10)
            .unwrap()
            .is_empty()
    );
}
