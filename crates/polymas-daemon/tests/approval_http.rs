#[path = "support/approval_fixture.rs"]
mod fixture;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;
const TOKEN: &str = "approval-owner-fixture-token-32-characters";

#[tokio::test]
async fn pages_are_stable_and_resource_review_uses_the_original_target() {
    use polymas_core::policy::ApprovalLedger;
    let (_dir, mut engine, request) = fixture::setup();
    let approval = engine
        .request_approval(
            "request",
            request.clone(),
            chrono::Utc::now().timestamp(),
            60,
        )
        .unwrap();
    for n in 0..3 {
        let mut operation = approval.operation.clone();
        operation.task_id = format!("historical-{n}");
        ApprovalLedger::request(&mut engine.workspace.db, operation, 100, 30).unwrap();
    }
    let mut connection = engine.workspace.connection(&request.connection_id).unwrap();
    connection.value.transport = polymas_core::connections::ConnectionTransport::Http {
        url: "https://changed.invalid/mcp".into(),
    };
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
    let state = AppState::new(engine, TOKEN.into(), vec![]).unwrap();
    let (_, detail) = call(
        &state,
        &format!("/api/approvals/{}", approval.id),
        None,
        TOKEN,
    )
    .await;
    assert_eq!(
        detail["connection"]["transport"]["url"],
        "https://fixture.invalid/mcp"
    );
    assert_eq!(detail["canApprove"], false);
    assert_eq!(detail["canDeny"], true);
    let (_, first) = call(&state, "/api/approvals?limit=2", None, TOKEN).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    let (_, second) = call(
        &state,
        &format!(
            "/api/approvals?limit=2&after={}",
            first["nextCursor"].as_str().unwrap()
        ),
        None,
        TOKEN,
    )
    .await;
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert!(second["nextCursor"].is_null());
    let ids: std::collections::BTreeSet<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 4);
}

async fn call(
    state: &AppState,
    path: &str,
    body: Option<Value>,
    token: &str,
) -> (StatusCode, Value) {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .uri(path)
                .method(if body.is_some() { "POST" } else { "GET" })
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
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

#[tokio::test]
async fn owner_reviews_exact_arguments_and_decides_a_revision_without_agent_authority() {
    let (_dir, mut engine, request) = fixture::setup();
    let approval = engine
        .request_approval(
            "request",
            request.clone(),
            chrono::Utc::now().timestamp(),
            60,
        )
        .unwrap();
    let state = AppState::new(engine, TOKEN.into(), vec![]).unwrap();
    let grant = polymas_daemon::gateway::issue(&state, &request.task_id)
        .await
        .unwrap();
    assert_eq!(
        call(&state, "/api/approvals", None, grant.token.expose())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, page) = call(&state, "/api/approvals?limit=1&state=pending", None, TOKEN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"][0]["id"], approval.id);
    assert!(!page.to_string().contains("APPROVAL-FIXTURE-73"));
    let detail_path = format!("/api/approvals/{}", approval.id);
    let (status, detail) = call(&state, &detail_path, None, TOKEN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        detail["approval"]["value"]["operation"]["arguments"],
        request.arguments
    );
    assert_eq!(detail["connection"]["name"], "Fixture");
    let decision = json!({"expectedRevision":1,"fingerprint":approval.fingerprint,"allow":true});
    let path = format!("{detail_path}/decision");
    assert_eq!(
        call(&state, &path, Some(decision.clone()), grant.token.expose())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut tampered = decision.clone();
    tampered["arguments"] = json!({"text":"REPLACED"});
    assert_eq!(
        call(&state, &path, Some(tampered), TOKEN).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(&state, &path, Some(decision.clone()), TOKEN).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&state, &path, Some(decision), TOKEN).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&state, "/api/approvals?limit=0", None, TOKEN).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&state, "/api/approvals?state=other", None, TOKEN)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, pending) = call(&state, "/api/approvals?state=pending", None, TOKEN).await;
    assert!(pending["items"].as_array().unwrap().is_empty());
    let (_, wrong_run) = call(&state, "/api/approvals?runId=other", None, TOKEN).await;
    assert!(wrong_run["items"].as_array().unwrap().is_empty());
}
