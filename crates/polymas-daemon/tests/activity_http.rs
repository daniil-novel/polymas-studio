use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    scheduler::{TaskInput, TaskState},
    topology::Target,
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, router};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn old_active_agents_remain_observable_beyond_the_recent_task_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Activity fixture".into(),
                    instruction: "Test".into(),
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
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap();
    engine
        .submit(
            "active",
            TaskInput {
                prompt: "ACTIVITY-PRIVATE-FIXTURE".into(),
                target: Target::System { broadcast: true },
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let running = engine.claim(1, "fixture-worker", 100).unwrap().remove(0);
    let queued = engine
        .scheduler
        .tasks
        .iter()
        .find(|task| task.state == TaskState::Queued)
        .unwrap()
        .clone();
    engine
        .submit(
            "later",
            TaskInput {
                prompt: "Wait behind active task".into(),
                target: Target::Agent {
                    id: running.assignee.clone(),
                },
                ..Default::default()
            },
            1,
        )
        .unwrap();
    // Author a large synthetic terminal history; no model processes are started.
    for i in 0..1001 {
        let mut task = queued.clone();
        task.id = format!("history-{i}");
        task.state = TaskState::Accepted;
        engine.scheduler.tasks.push(task);
    }
    let state = AppState::new(
        engine,
        "activity-test-token-at-least-32-characters".into(),
        vec![],
    )
    .unwrap();
    let response = router(state)
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .header(
                    "authorization",
                    "Bearer activity-test-token-at-least-32-characters",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["tasks"].as_array().unwrap().len(), 1000);
    assert!(
        value["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["id"] != running.id)
    );
    let activity = value["agentActivity"]
        .as_array()
        .expect("separate activity projection");
    assert_eq!(activity.len(), 2);
    let active = activity
        .iter()
        .find(|item| item["assignee"] == running.assignee)
        .unwrap();
    assert_eq!(active["id"], running.id);
    assert_eq!(active["state"], "running");
    assert_eq!(active["lease"]["heartbeatAt"], 100);
    assert!(
        activity
            .iter()
            .any(|item| item["id"] == queued.id && item["state"] == "queued")
    );
    assert!(
        activity
            .iter()
            .all(|item| item.as_object().unwrap().len() == 4)
    );
    assert!(
        !serde_json::to_string(activity)
            .unwrap()
            .contains("ACTIVITY-PRIVATE-FIXTURE")
    );
}
