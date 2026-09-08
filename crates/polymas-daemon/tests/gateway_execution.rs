#[path = "support/approval_fixture.rs"]
mod fixture;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    connections::ConnectionTransport,
    engine::OperationRequest,
    policy::{Approval, ApprovalState},
    workspace::ConfigCommand,
};
use polymas_daemon::{
    AppState,
    gateway::{Grant, issue},
    router,
};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use tower::ServiceExt;

async fn call(state: &AppState, grant: &Grant, tool: &str, arguments: Value) -> Value {
    call_token(state, grant.token.expose(), tool, arguments).await
}
async fn call_token(state: &AppState, token: &str, tool: &str, arguments: Value) -> Value {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/gateway/call")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":tool,"arguments":arguments}).to_string(),
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
async fn setup(
    mode: &str,
) -> (
    tempfile::TempDir,
    AppState,
    OperationRequest,
    Grant,
    Approval,
    PathBuf,
) {
    let (dir, mut engine, request) = fixture::setup();
    let target = dir.path().join("external-records.jsonl");
    let output = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let python = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    let mut connection = engine.workspace.connection(&request.connection_id).unwrap();
    connection.value.transport = ConnectionTransport::Stdio {
        command: python,
        args: vec![
            "-u".into(),
            format!("{}/tests/support/write_mcp.py", env!("CARGO_MANIFEST_DIR")),
            target.to_str().unwrap().into(),
            mode.into(),
        ],
    };
    connection.value.timeout_seconds = 5;
    connection.value.validate().unwrap();
    engine
        .workspace
        .db
        .transaction(|tx| {
            tx.put(
                "connections",
                &request.connection_id,
                connection.revision,
                &connection.value,
            )
        })
        .unwrap();
    let state = AppState::new(
        engine,
        "execution-fixture-owner-token-32-characters".into(),
        vec![],
    )
    .unwrap();
    let origin = issue(&state, &request.task_id).await.unwrap();
    let receipt = content(&call(&state,&origin,"polymas_request_operation",json!({"idempotencyKey":"write-once","connectionId":request.connection_id,"namespace":request.namespace,"tool":request.tool,"arguments":request.arguments})).await);
    let id = receipt["approvalId"].as_str().unwrap();
    let mut engine = state.engine.lock().await;
    let approval = engine
        .workspace
        .db
        .get::<Approval>("approvals", id)
        .unwrap()
        .unwrap()
        .value;
    engine
        .decide_approval(
            id,
            1,
            &approval.fingerprint,
            true,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    drop(engine);
    assert_eq!(
        call(
            &state,
            &origin,
            "polymas_execute_operation",
            json!({"approvalId":id})
        )
        .await["isError"],
        true,
        "Even approved original turn cannot execute"
    );
    let mut engine = state.engine.lock().await;
    engine
        .park_approval(
            &request.task_id,
            request.origin_epoch,
            "Ready for decision".into(),
        )
        .unwrap();
    engine
        .wake_approvals(chrono::Utc::now().timestamp())
        .unwrap();
    let next = engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap()
        .remove(0);
    assert_eq!(next.lease.unwrap().epoch, approval.operation.epoch);
    drop(engine);
    drop(origin);
    assert!(!target.exists());
    let grant = issue(&state, &request.task_id).await.unwrap();
    (dir, state, request, grant, approval, target)
}
#[tokio::test]
async fn an_unresolved_execution_cannot_be_replaced_by_another_request_or_wait() {
    let (_dir, state, request, grant, approval, _) = setup("disconnect").await;
    let request_input = json!({"idempotencyKey":"second-write","connectionId":request.connection_id,"namespace":request.namespace,"tool":request.tool,"arguments":request.arguments});
    assert_eq!(
        call(
            &state,
            &grant,
            "polymas_request_operation",
            request_input.clone()
        )
        .await["isError"],
        true
    );
    assert_eq!(
        call(
            &state,
            &grant,
            "polymas_wait_for_message",
            json!({"timeoutSeconds":60,"summary":"Wait"})
        )
        .await["isError"],
        true
    );
    let outcome = content(
        &call(
            &state,
            &grant,
            "polymas_execute_operation",
            json!({"approvalId":approval.id}),
        )
        .await,
    );
    assert_eq!(outcome["state"], "unknown");
    assert_eq!(
        call(&state, &grant, "polymas_request_operation", request_input).await["isError"],
        true
    );
    assert!(
        state
            .engine
            .lock()
            .await
            .scheduler
            .task(&request.task_id)
            .unwrap()
            .continuation
            .is_none()
    );
}
#[tokio::test]
async fn revoking_a_grant_interrupts_its_in_flight_call_even_while_the_task_lease_is_running() {
    let (_dir, state, _, grant, approval, target) = setup("slow").await;
    let token = grant.token.expose().to_owned();
    let copy = state.clone();
    let id = approval.id.clone();
    let handler = tokio::spawn(async move {
        call_token(
            &copy,
            &token,
            "polymas_execute_operation",
            json!({"approvalId":id}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !target.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    drop(grant);
    let result = tokio::time::timeout(Duration::from_secs(2), handler)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result["isError"], true);
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .get::<Approval>("approvals", &approval.id)
            .unwrap()
            .unwrap()
            .value
            .state,
        ApprovalState::Unknown
    );
    assert_eq!(std::fs::read_to_string(target).unwrap().lines().count(), 1);
}
#[tokio::test]
async fn approved_future_turn_writes_frozen_arguments_once_and_reads_the_durable_receipt() {
    let (_dir, state, request, grant, approval, target) = setup("success").await;
    let args = json!({"approvalId":approval.id});
    let mut forged = args.clone();
    forged["arguments"] = json!({"text":"REPLACED"});
    assert_eq!(
        call(&state, &grant, "polymas_execute_operation", forged).await["isError"],
        true
    );
    assert!(!target.exists());
    let result = content(&call(&state, &grant, "polymas_execute_operation", args.clone()).await);
    assert_eq!(result["state"], "succeeded");
    assert_eq!(
        result["result"]["response"]["content"][0]["text"],
        "WROTE:APPROVAL-FIXTURE-73"
    );
    let repeated = content(&call(&state, &grant, "polymas_execute_operation", args.clone()).await);
    assert_eq!(repeated, result);
    let status = content(&call(&state, &grant, "polymas_operation_status", args).await);
    assert_eq!(status["result"], result["result"]);
    assert_eq!(std::fs::read_to_string(&target).unwrap().lines().count(), 1);
    let written: Value =
        serde_json::from_str(std::fs::read_to_string(&target).unwrap().trim()).unwrap();
    assert_eq!(written, request.arguments);
    let engine = state.engine.lock().await;
    assert_eq!(
        engine
            .workspace
            .db
            .get::<Approval>("approvals", &approval.id)
            .unwrap()
            .unwrap()
            .value
            .state,
        ApprovalState::Succeeded
    );
    assert!(
        engine
            .workspace
            .db
            .get::<Value>("approval_results", &approval.id)
            .unwrap()
            .is_some()
    );
    let events = engine.workspace.db.events(0, 200, None).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.input.kind == "approval.execution_started")
            .count(),
        1
    );
    assert!(!json!(events).to_string().contains("APPROVAL-FIXTURE-73"));
}
#[tokio::test]
async fn lost_response_or_partial_tool_error_is_unknown_and_never_resent() {
    for mode in ["disconnect", "error"] {
        let (_dir, state, _, grant, approval, target) = setup(mode).await;
        let args = json!({"approvalId":approval.id});
        let outcome =
            content(&call(&state, &grant, "polymas_execute_operation", args.clone()).await);
        assert_eq!(outcome["state"], "unknown");
        let again = content(&call(&state, &grant, "polymas_execute_operation", args).await);
        assert_eq!(again["state"], "unknown");
        assert_eq!(std::fs::read_to_string(target).unwrap().lines().count(), 1);
    }
}
#[tokio::test]
async fn revoked_target_is_rejected_before_consuming_or_spawning() {
    let (_dir, state, _, grant, approval, target) = setup("success").await;
    let mut engine = state.engine.lock().await;
    let revision = engine.workspace.config().revision;
    engine
        .configure(revision, ConfigCommand::SetBindings { bindings: vec![] })
        .unwrap();
    drop(engine);
    assert_eq!(
        call(
            &state,
            &grant,
            "polymas_execute_operation",
            json!({"approvalId":approval.id})
        )
        .await["isError"],
        true
    );
    assert!(!target.exists());
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .get::<Approval>("approvals", &approval.id)
            .unwrap()
            .unwrap()
            .value
            .state,
        ApprovalState::Approved
    );
}
#[tokio::test]
async fn cancelled_http_handler_leaves_unknown_instead_of_an_execution_that_can_be_retried() {
    let (_dir, state, _, grant, approval, target) = setup("slow").await;
    let state_copy = state.clone();
    let id = approval.id.clone();
    let handler = tokio::spawn(async move {
        call(
            &state_copy,
            &grant,
            "polymas_execute_operation",
            json!({"approvalId":id}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !target.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    handler.abort();
    let _ = handler.await;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if state
                .engine
                .lock()
                .await
                .workspace
                .db
                .get::<Approval>("approvals", &approval.id)
                .unwrap()
                .unwrap()
                .value
                .state
                == ApprovalState::Unknown
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(target).unwrap().lines().count(), 1);
}
