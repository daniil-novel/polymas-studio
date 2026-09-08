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
async fn owner_reads_saved_result_pages_after_attempt_closes_without_leaking_them_in_lists() {
    use polymas_core::policy::{ApprovalLedger, ApprovalState};
    let (_dir, mut engine, request) = fixture::setup();
    let now = chrono::Utc::now().timestamp();
    let approval = engine
        .request_approval("result-fixture", request.clone(), now, 60)
        .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Wait".into())
        .unwrap();
    engine
        .decide_approval(&approval.id, 1, &approval.fingerprint, true, now)
        .unwrap();
    engine.wake_approvals(now).unwrap();
    let task = engine.claim(1, "local", now).unwrap().remove(0);
    let epoch = task.lease.as_ref().unwrap().epoch;
    engine
        .consume_approval(&task.id, epoch, &approval.id, now)
        .unwrap();
    let result = json!({"response":{"text":"PRIVATE-RESULT-\"\\🧠".repeat(3000)}});
    ApprovalLedger::complete_with_result(
        &mut engine.workspace.db,
        &approval.id,
        ApprovalState::Succeeded,
        Some(result.clone()),
    )
    .unwrap();
    let state = AppState::new(engine, TOKEN.into(), vec![]).unwrap();
    let grant = polymas_daemon::gateway::issue(&state, &task.id)
        .await
        .unwrap();
    let path = format!("/api/approvals/{}/result", approval.id);
    assert_eq!(
        call(&state, &path, None, grant.token.expose()).await.0,
        StatusCode::UNAUTHORIZED
    );
    state
        .engine
        .lock()
        .await
        .finish(&task.id, epoch, Ok("Model finished".into()))
        .unwrap();
    let head = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    let (status, first) = call(&state, &path, None, TOKEN).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["chunk"].as_str().unwrap().len() <= 32768);
    let (_, detail) = call(
        &state,
        &format!("/api/approvals/{}", approval.id),
        None,
        TOKEN,
    )
    .await;
    assert_eq!(detail["resultMetadata"], first["metadata"]);
    assert!(!detail.to_string().contains("PRIVATE-RESULT"));
    let mut text = first["chunk"].as_str().unwrap().to_owned();
    let mut next = first["nextOffset"].as_u64();
    while let Some(offset) = next {
        let (status, page) = call(&state, &format!("{path}?offset={offset}"), None, TOKEN).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["metadata"], first["metadata"]);
        text.push_str(page["chunk"].as_str().unwrap());
        next = page["nextOffset"].as_u64();
    }
    assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), result);
    assert_eq!(
        call(&state, &format!("{path}?limit=32769"), None, TOKEN)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&state, "/api/approvals/missing/result", None, TOKEN)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = call(&state, "/api/approvals", None, TOKEN).await;
    assert!(!list.to_string().contains("PRIVATE-RESULT"));
    let engine = state.engine.lock().await;
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
}

#[tokio::test]
async fn only_the_owner_can_record_an_unknown_outcome_without_sending_a_new_call() {
    use polymas_core::policy::{ApprovalLedger, ApprovalState};
    let (_dir, mut engine, request) = fixture::setup();
    let approval = engine
        .request_approval(
            "request",
            request.clone(),
            chrono::Utc::now().timestamp(),
            60,
        )
        .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Wait".into())
        .unwrap();
    engine
        .decide_approval(
            &approval.id,
            1,
            &approval.fingerprint,
            true,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    engine
        .wake_approvals(chrono::Utc::now().timestamp())
        .unwrap();
    let task = engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap()
        .remove(0);
    engine
        .consume_approval(
            &task.id,
            task.lease.as_ref().unwrap().epoch,
            &approval.id,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    ApprovalLedger::complete(
        &mut engine.workspace.db,
        &approval.id,
        ApprovalState::Unknown,
    )
    .unwrap();
    let state = AppState::new(engine, TOKEN.into(), vec![]).unwrap();
    let grant = polymas_daemon::gateway::issue(&state, &task.id)
        .await
        .unwrap();
    let path = format!("/api/approvals/{}/reconcile", approval.id);
    let input = json!({"expectedRevision":4,"outcome":"succeeded","note":"Fixture server shows the expected document hash"});
    assert_eq!(
        call(&state, &path, Some(input.clone()), grant.token.expose())
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let mut invalid = input.clone();
    invalid["outcome"] = json!("approved");
    assert_eq!(
        call(&state, &path, Some(invalid), TOKEN).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(&state, &path, Some(input.clone()), TOKEN).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&state, &path, Some(input), TOKEN).await.0,
        StatusCode::OK
    );
    let (_, reviewed) = call(
        &state,
        &format!("/api/approvals/{}", approval.id),
        None,
        TOKEN,
    )
    .await;
    assert_eq!(reviewed["reconciliation"]["author"], "local:owner");
    assert_eq!(reviewed["reconciliation"]["outcome"], "succeeded");
    let engine = state.engine.lock().await;
    assert_eq!(
        engine.scheduler.task(&task.id).unwrap().attempt,
        task.attempt
    );
    assert_eq!(engine.scheduler.task(&task.id).unwrap().state, task.state);
    let events = engine
        .workspace
        .db
        .events(0, 100, Some(&task.run_id))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.input.kind == "approval.execution_started")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| e.input.kind == "approval.reconciled")
            .count(),
        1
    );
}

/// Manual browser QA uses an explicitly separate, new workspace with no real target.
#[test]
#[ignore = "requires POLYMAS_VISUAL_FIXTURE_DIR naming a new local directory"]
fn create_local_approval_review_fixture() {
    let target = std::path::PathBuf::from(std::env::var("POLYMAS_VISUAL_FIXTURE_DIR").unwrap());
    assert!(target.is_absolute() && !target.exists());
    let (dir, mut engine, mut request) = fixture::setup();
    engine
        .configure(
            4,
            polymas_core::workspace::ConfigCommand::RenameWorkspace {
                name: "Проверка подтверждений · тестовые данные".into(),
            },
        )
        .unwrap();
    request.arguments = json!({"document":"reports/architecture.md","title":"Архитектура агентов","text":"Каждый экземпляр сохраняет собственную сессию. Запись требует решения владельца."});
    let approval = engine
        .request_approval(
            "visual-fixture",
            request.clone(),
            chrono::Utc::now().timestamp(),
            3600,
        )
        .unwrap();
    engine
        .park_approval(
            &request.task_id,
            request.origin_epoch,
            "Подготовлен отчёт; ожидается решение владельца.".into(),
        )
        .unwrap();
    if std::env::var("POLYMAS_VISUAL_FIXTURE_STATE").as_deref() == Ok("unknown") {
        let now = chrono::Utc::now().timestamp();
        engine
            .decide_approval(&approval.id, 1, &approval.fingerprint, true, now)
            .unwrap();
        engine.wake_approvals(now).unwrap();
        let task = engine.claim(1, "local", now).unwrap().remove(0);
        engine
            .consume_approval(
                &task.id,
                task.lease.as_ref().unwrap().epoch,
                &approval.id,
                now,
            )
            .unwrap();
        // Deliberate test data: no external call occurs. Reopen recovers the abandoned lease.
    }
    engine.pause(true).unwrap();
    drop(engine);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        dir.path().join("state.sqlite3"),
        target.join("state.sqlite3"),
    )
    .unwrap();
}

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
