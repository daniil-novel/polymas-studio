use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::engine::Engine;
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "run-history-owner-fixture-token-32-characters";

fn setup() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    engine
        .workspace
        .db
        .transaction(|tx| {
            for i in 0..137 {
                let id = format!("run-{i:04}");
                tx.put(
                    "runs",
                    &id,
                    0,
                    &json!({
                            "id":id,"title":format!("История {i:04} %_"),
                            "createdAt":"2026-09-08T12:00:00Z","accepted":i < 5,"stopped":i == 5,
                    "configRevision":7,"taskIds": if i == 0 {
                        (0..1017).map(|n| format!("task-{n:04}")).collect::<Vec<_>>()
                    } else { vec!["fixture".to_owned()] },
                            "input":{"prompt":"PRIVATE-RUN-PROMPT","completion":{"type":"manual"}},
                            "architecture":{"private":"PRIVATE-ARCHITECTURE"}
                        }),
                )?;
            }
            for i in 0..1017 {
                let id = format!("task-{i:04}");
                tx.put(
                    "tasks",
                    &id,
                    0,
                    &json!({
                        "id":id,"runId":"run-0000","assignee":format!("agent-{i:04}"),
                        "state":if i < 7 {"failed"} else {"accepted"},"attempt":1,
                        "createdAt":"2026-09-08T12:00:00Z","completedAt":null,
                        "prompt":"PRIVATE-TASK-PROMPT","result":"PRIVATE-TASK-RESULT".repeat(1000),
                        "error":"PRIVATE-ERROR","role":{"instruction":"PRIVATE-ROLE"}
                    }),
                )?;
            }
            tx.put("tasks", "other-run-task", 0, &json!({
            "id":"other-run-task","runId":"run-0001","assignee":"other-agent",
            "state":"failed","attempt":1,"createdAt":"2026-09-08T12:00:00Z","completedAt":null
        }))?;
            Ok(())
        })
        .unwrap();
    (dir, AppState::new(engine, TOKEN.into(), vec![]).unwrap())
}

async fn get(state: &AppState, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
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

fn encoded(text: &str) -> String {
    text.bytes().map(|b| format!("%{b:02X}")).collect()
}

#[tokio::test]
async fn whole_run_history_has_stable_pages_literal_unicode_search_and_private_free_summaries() {
    let (_dir, state) = setup();
    let mut cursor = None;
    let mut ids = vec![];
    loop {
        let uri = format!(
            "/api/runs?limit=30{}",
            cursor
                .as_ref()
                .map(|c: &String| format!("&cursor={}", encoded(c)))
                .unwrap_or_default()
        );
        let (status, page) = get(&state, &uri, TOKEN).await;
        assert_eq!(status, StatusCode::OK);
        let rows = page["items"].as_array().unwrap();
        assert!(rows.len() <= 30);
        assert!(!page.to_string().contains("PRIVATE-"));
        assert!(rows.iter().all(|r| r.get("taskIds").is_none()
            && r["taskCount"] == if r["id"] == "run-0000" { 1017 } else { 1 }));
        ids.extend(rows.iter().map(|r| r["id"].as_str().unwrap().to_owned()));
        cursor = page["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
        if ids.len() == 30 {
            state
                .engine
                .lock()
                .await
                .workspace
                .db
                .transaction(|tx| {
                    tx.put(
                        "runs",
                        "new-run",
                        0,
                        &json!({"id":"new-run","title":"New arrival",
                            "createdAt":"2026-09-08T13:00:00Z","accepted":false,"stopped":false,
                            "configRevision":8,"taskIds":[],"input":{"completion":{"type":"manual"}}
                        }),
                    )?;
                    Ok(())
                })
                .unwrap();
        }
    }
    assert_eq!(
        ids,
        (0..137)
            .rev()
            .map(|i| format!("run-{i:04}"))
            .collect::<Vec<_>>()
    );
    let (_, filtered) = get(
        &state,
        &format!(
            "/api/runs?query={}&status=accepted&limit=2",
            encoded("ИСТОРИЯ")
        ),
        TOKEN,
    )
    .await;
    assert_eq!(filtered["items"][0]["id"], "run-0004");
    let (_, literal) = get(
        &state,
        &format!("/api/runs?query={}", encoded("0000 %_")),
        TOKEN,
    )
    .await;
    assert_eq!(literal["items"].as_array().unwrap().len(), 1);
    assert_eq!(literal["items"][0]["id"], "run-0000");
    let (_, detail) = get(&state, "/api/runs/run-0000", TOKEN).await;
    assert_eq!(detail["completion"]["type"], "manual");
    assert!(!detail.to_string().contains("PRIVATE-"));
    assert_eq!(
        get(&state, "/api/runs/missing", TOKEN).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn run_task_pages_cover_old_tasks_and_filter_before_limit() {
    let (_dir, state) = setup();
    let mut cursor = None;
    let mut ids = vec![];
    loop {
        let path = format!(
            "/api/runs/run-0000/tasks?limit=100{}",
            cursor
                .as_ref()
                .map(|c: &String| format!("&cursor={}", encoded(c)))
                .unwrap_or_default()
        );
        let (status, page) = get(&state, &path, TOKEN).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!page.to_string().contains("PRIVATE-"));
        let rows = page["items"].as_array().unwrap();
        assert!(rows.len() <= 100 && rows.iter().all(|t| t["runId"] == "run-0000"));
        ids.extend(rows.iter().map(|t| t["id"].as_str().unwrap().to_owned()));
        cursor = page["nextCursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        ids,
        (0..1017)
            .rev()
            .map(|i| format!("task-{i:04}"))
            .collect::<Vec<_>>()
    );
    let (_, page) = get(
        &state,
        "/api/runs/run-0000/tasks?state=failed&limit=3",
        TOKEN,
    )
    .await;
    assert_eq!(page["items"][0]["id"], "task-0006");
    let cursor = encoded(page["nextCursor"].as_str().unwrap());
    assert_eq!(
        get(
            &state,
            &format!("/api/runs/run-0001/tasks?state=failed&cursor={cursor}"),
            TOKEN
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(
            &state,
            &format!("/api/runs/run-0000/tasks?cursor={cursor}"),
            TOKEN
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn history_enforces_owner_auth_query_bounds_and_cursor_scope_without_writes() {
    let (_dir, state) = setup();
    let before = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    for path in [
        "/api/runs",
        "/api/runs/run-0000",
        "/api/runs/run-0000/tasks",
    ] {
        assert_eq!(
            get(&state, path, "wrong-owner-token").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    for path in [
        "/api/runs?limit=0",
        "/api/runs?limit=101",
        "/api/runs?status=bogus",
        "/api/runs?cursor=garbage",
        "/api/runs?unexpected=1",
        "/api/runs/run-0000/tasks?state=bogus",
        "/api/runs/run-0000/tasks?limit=0",
    ] {
        assert_eq!(
            get(&state, path, TOKEN).await.0,
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    let (_, page) = get(&state, "/api/runs?limit=1", TOKEN).await;
    let cursor = encoded(page["nextCursor"].as_str().unwrap());
    for suffix in ["&query=changed", "&status=accepted"] {
        assert_eq!(
            get(&state, &format!("/api/runs?cursor={cursor}{suffix}"), TOKEN)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        get(
            &state,
            &format!("/api/runs?query={}", "x".repeat(201)),
            TOKEN
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
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
        before
    );
}
