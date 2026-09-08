use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    scheduler::TaskInput,
    topology::{CommunicationMode, Edge, Target},
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, gateway, router};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn setup(mode: CommunicationMode) -> (tempfile::TempDir, AppState, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Writer".into(),
                    instruction: "Publish evidence".into(),
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
                    name_pattern: "Writer {n}".into(),
                },
            },
        )
        .unwrap();
    let agents = engine.workspace.config().value.registry.agents.clone();
    let mut architecture = engine.workspace.config().value.architecture.clone();
    architecture.communication_mode = mode;
    architecture.max_concurrency = 2;
    architecture.edges = vec![Edge {
        from: agents[0].id.clone(),
        to: agents[1].id.clone(),
    }];
    engine
        .configure(3, ConfigCommand::SetArchitecture { architecture })
        .unwrap();
    let run = engine
        .submit(
            "run",
            TaskInput {
                prompt: "Publish and read".into(),
                target: Target::System { broadcast: true },
                ..Default::default()
            },
            2,
        )
        .unwrap();
    engine
        .claim(2, "fixture", chrono::Utc::now().timestamp())
        .unwrap();
    (
        dir,
        AppState::new(
            engine,
            "owner-publication-fixture-token-00000".into(),
            vec![],
        )
        .unwrap(),
        run.task_ids,
    )
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
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn payload(reply: &Value) -> Value {
    assert_eq!(reply["isError"], false, "{reply}");
    serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn all_publication_modes_use_attempt_identity_and_live_intersection_permissions() {
    for mode in [
        CommunicationMode::Blackboard,
        CommunicationMode::Wiki,
        CommunicationMode::Graph,
    ] {
        let (_dir, state, tasks) = setup(mode).await;
        let a = gateway::issue(&state, &tasks[0]).await.unwrap();
        let b = gateway::issue(&state, &tasks[1]).await.unwrap();
        let mut input = json!({"key":"one","title":"Evidence","body":"PUBLICATION-MARKER"});
        if mode == CommunicationMode::Wiki {
            input["slug"] = json!("plan");
        }
        let first =
            payload(&call(&state, a.token.expose(), "polymas_publish", input.clone()).await);
        assert_eq!(
            payload(&call(&state, a.token.expose(), "polymas_publish", input.clone()).await)["id"],
            first["id"]
        );
        let read = payload(
            &call(
                &state,
                b.token.expose(),
                "polymas_read_publications",
                json!({"limit":1}),
            )
            .await,
        );
        assert_eq!(read["items"][0]["body"], "PUBLICATION-MARKER");
        assert_eq!(
            read["items"][0]["author"],
            state
                .engine
                .lock()
                .await
                .scheduler
                .task(&tasks[0])
                .unwrap()
                .assignee
        );
        let mut injected = input;
        injected["author"] = json!("owner");
        assert_eq!(
            call(&state, b.token.expose(), "polymas_publish", injected).await["isError"],
            true
        );
        assert_eq!(
            call(
                &state,
                b.token.expose(),
                "polymas_read_publications",
                json!({"runId":"other"})
            )
            .await["isError"],
            true
        );
        assert_eq!(
            call(&state, b.token.expose(), "polymas_list_inbox", json!({})).await["isError"],
            true
        );
        {
            let mut engine = state.engine.lock().await;
            let rev = engine.workspace.config().revision;
            let mut architecture = engine.workspace.config().value.architecture.clone();
            architecture.edges.clear();
            engine
                .configure(rev, ConfigCommand::SetArchitecture { architecture })
                .unwrap();
        }
        assert_eq!(
            payload(
                &call(
                    &state,
                    b.token.expose(),
                    "polymas_read_publications",
                    json!({})
                )
                .await
            )["items"],
            json!([])
        );
        {
            let mut engine = state.engine.lock().await;
            let rev = engine.workspace.config().revision;
            let mut architecture = engine.workspace.config().value.architecture.clone();
            architecture.communication_mode = CommunicationMode::Direct;
            engine
                .configure(rev, ConfigCommand::SetArchitecture { architecture })
                .unwrap();
        }
        assert_eq!(
            call(
                &state,
                a.token.expose(),
                "polymas_publish",
                json!({"key":"two","title":"Denied","body":"No"})
            )
            .await["isError"],
            true
        );
        let old_token = a.token.expose().to_string();
        drop(a);
        assert_eq!(
            call(&state, &old_token, "polymas_read_publications", json!({})).await["code"],
            "unauthorized"
        );
    }
}

#[tokio::test]
async fn late_permissions_do_not_enlarge_a_pinned_run_and_direct_mode_cannot_publish() {
    let (_dir, state, tasks) = setup(CommunicationMode::Blackboard).await;
    let a = gateway::issue(&state, &tasks[0]).await.unwrap();
    let b = gateway::issue(&state, &tasks[1]).await.unwrap();
    payload(
        &call(
            &state,
            b.token.expose(),
            "polymas_publish",
            json!({"key":"b","title":"Private","body":"B-ONLY"}),
        )
        .await,
    );
    {
        let mut engine = state.engine.lock().await;
        let rev = engine.workspace.config().revision;
        let mut architecture = engine.workspace.config().value.architecture.clone();
        architecture.edges.push(Edge {
            from: engine.scheduler.task(&tasks[1]).unwrap().assignee.clone(),
            to: engine.scheduler.task(&tasks[0]).unwrap().assignee.clone(),
        });
        engine
            .configure(rev, ConfigCommand::SetArchitecture { architecture })
            .unwrap();
    }
    assert_eq!(
        payload(
            &call(
                &state,
                a.token.expose(),
                "polymas_read_publications",
                json!({})
            )
            .await
        )["items"],
        json!([])
    );
    let (_dir2, direct, tasks2) = setup(CommunicationMode::Direct).await;
    let grant = gateway::issue(&direct, &tasks2[0]).await.unwrap();
    assert_eq!(
        call(
            &direct,
            grant.token.expose(),
            "polymas_publish",
            json!({"key":"x","title":"x","body":"x"})
        )
        .await["isError"],
        true
    );
}
