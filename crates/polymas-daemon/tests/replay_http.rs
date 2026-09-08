use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    scheduler::TaskInput,
    storage::EventInput,
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "replay-fixture-token-at-least-32-characters";
async fn get(app: &Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("authorization", format!("Bearer {TOKEN}"))
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
fn fixture() -> (tempfile::TempDir, AppState, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Recorded role".into(),
                    instruction: "Private instruction omitted from graph manifest".into(),
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
                    count: 1,
                    name_pattern: "Original {n}".into(),
                },
            },
        )
        .unwrap();
    let run = engine
        .submit(
            "recording",
            TaskInput {
                title: "Recorded run".into(),
                prompt: "Private prompt omitted from manifest".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let agent_id = engine.workspace.config().value.registry.agents[0]
        .id
        .clone();
    engine
        .configure(
            3,
            ConfigCommand::RenameAgent {
                agent_id,
                name: "Renamed now".into(),
            },
        )
        .unwrap();
    (
        dir,
        AppState::new(engine, TOKEN.into(), vec![]).unwrap(),
        run.id,
    )
}

#[tokio::test]
async fn recording_uses_pinned_configuration_and_a_frozen_run_only_cursor_without_mutating_state() {
    let (_dir, state, run) = fixture();
    let app = router(state.clone());
    let path = format!("/api/runs/{run}/recording");
    let (status, manifest) = get(&app, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(manifest["configRevision"], 3);
    assert_eq!(manifest["agents"][0]["name"], "Original 1");
    assert!(manifest["tasks"][0].get("state").is_none());
    assert!(!manifest.to_string().contains("Private"));
    let head = manifest["head"].as_i64().unwrap();
    let initial = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    state
        .engine
        .lock()
        .await
        .workspace
        .db
        .transaction(|tx| {
            tx.emit(EventInput::new("fixture.other", json!({})).run("other-run"))?;
            tx.emit(EventInput::new("fixture.later", json!({})).run(&run))?;
            Ok(())
        })
        .unwrap();
    let mut after = 0;
    let mut ids = vec![];
    loop {
        let (status, page) = get(
            &app,
            &format!("{path}/events?after={after}&through={head}&limit=1"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        for event in page["events"].as_array().unwrap() {
            assert_eq!(event["runId"], run);
            assert_ne!(event["kind"], "fixture.later");
            ids.push(event["sequence"].as_i64().unwrap());
        }
        let next = page["cursor"].as_i64().unwrap();
        assert!(next > after);
        after = next;
        if page["complete"] == true {
            break;
        }
    }
    assert_eq!(after, head);
    assert_eq!(ids.len(), 2);
    assert!(ids.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .last_sequence()
            .unwrap(),
        initial + 2
    );
    assert!(state.active.lock().await.is_empty());
    assert_eq!(state.engine.lock().await.scheduler.tasks[0].attempt, 0);
}

#[tokio::test]
async fn recording_pages_are_size_bounded_and_reject_invalid_cursors_and_unauthorized_reads() {
    let (_dir, state, run) = fixture();
    state
        .engine
        .lock()
        .await
        .workspace
        .db
        .transaction(|tx| {
            for _ in 0..4 {
                tx.emit(
                    EventInput::new("fixture.large", json!({"data":"x".repeat(800_000)})).run(&run),
                )?;
            }
            Ok(())
        })
        .unwrap();
    let app = router(state.clone());
    let path = format!("/api/runs/{run}/recording");
    let (_, manifest) = get(&app, &path).await;
    let head = manifest["head"].as_i64().expect("recording manifest");
    let mut cursor = 0;
    let mut count = 0;
    loop {
        let (status, page) = get(
            &app,
            &format!("{path}/events?after={cursor}&through={head}&limit=500"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(serde_json::to_vec(&page).unwrap().len() <= 2 * 1024 * 1024);
        count += page["events"].as_array().unwrap().len();
        cursor = page["cursor"].as_i64().unwrap();
        if page["complete"] == true {
            break;
        }
    }
    assert_eq!(count, 6);
    for query in [
        format!("after=-1&through={head}"),
        format!("after={}&through={head}", head + 1),
        format!("through={}&limit=1", head + 1),
        format!("through={head}&limit=501"),
    ] {
        assert_eq!(
            get(&app, &format!("{path}/events?{query}")).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        get(&app, "/api/runs/missing/recording").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.clone()
            .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri(&path)
                .header("authorization", format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}
