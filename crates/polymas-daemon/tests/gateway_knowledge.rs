use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{
    connections::{Connection, Health, files_tools},
    engine::Engine,
    knowledge::{ClaimDraft, Knowledge, SourceDraft},
    policy::{Binding, Subject},
    registry::{BatchRequest, RoleDraft},
    scheduler::TaskInput,
    workspace::ConfigCommand,
};
use polymas_daemon::{AppState, gateway::issue, router};
use serde_json::{Value, json};
use tower::ServiceExt;
async fn setup(memory: bool) -> (tempfile::TempDir, AppState, String, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Memory reader".into(),
                    instruction: "Read permitted claims".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let role_id = role["id"].as_str().unwrap().to_string();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "one".into(),
                request: BatchRequest {
                    role_id: role_id.clone(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "Reader {n}".into(),
                },
            },
        )
        .unwrap();
    let mut connection = Connection::files("Memory fixture", vec![dir.path().to_owned()]);
    connection.tools = files_tools();
    connection.tool_allowlist = vec!["read_text".into()];
    connection.health = Health::Ready;
    connection.enabled = true;
    let connection_id = connection.id.clone();
    polymas_daemon::connection_api::persist_connection(
        &mut engine,
        0,
        connection,
        "fixture.connection",
    )
    .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::SetBindings {
                bindings: vec![Binding {
                    id: "read".into(),
                    connection_id: connection_id.clone(),
                    subject: Subject::Role { id: role_id },
                    tools: ["read_text".into()].into(),
                    allow_write: false,
                    required: false,
                    namespace: "shared".into(),
                }],
            },
        )
        .unwrap();
    let mut architecture = engine.workspace.config().value.architecture.clone();
    architecture.memory_enabled = memory;
    architecture.memory_connection_id = Some(connection_id.clone());
    engine
        .configure(4, ConfigCommand::SetArchitecture { architecture })
        .unwrap();
    let mut ids = vec![];
    for (index, connection) in [Some(connection_id), Some("private-connection".into()), None]
        .into_iter()
        .enumerate()
    {
        let source = Knowledge::add_source(
            &mut engine.workspace.db,
            SourceDraft {
                name: format!("Source {index}"),
                uri: "https://example.org/fixture".into(),
                version: "fixture-v1".into(),
                namespace: "shared".into(),
                connection_id: connection,
                retention_days: 30,
            },
        )
        .unwrap();
        let claim = Knowledge::publish(
            &mut engine.workspace.db,
            "human:owner",
            ClaimDraft {
                assertion: format!("FIXTURE-MEMORY-{index}"),
                source_id: source.id,
                namespace: "shared".into(),
            },
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
        ids.push(claim.id);
    }
    let run = engine
        .submit(
            "run",
            TaskInput {
                prompt: "Read fixture memory".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap();
    (
        dir,
        AppState::new(
            engine,
            "gateway-knowledge-control-token-32-chars".into(),
            vec![],
        )
        .unwrap(),
        run.task_ids[0].clone(),
        ids[0].clone(),
        ids[1].clone(),
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
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn data(response: &Value) -> Value {
    assert_eq!(response["isError"], false, "{response}");
    serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[tokio::test]
async fn memory_search_and_detail_apply_both_source_scope_and_namespace_without_logging_content() {
    let (_dir, state, task, visible, hidden) = setup(true).await;
    let grant = issue(&state, &task).await.unwrap();
    let search = call(
        &state,
        grant.token.expose(),
        "polymas_search_knowledge",
        json!({"namespace":"shared","limit":1}),
    )
    .await;
    let page = data(&search);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["value"]["id"], visible);
    assert_eq!(page["items"][0]["value"]["status"], "unverified");
    assert!(page["nextCursor"].is_null());
    let detail = data(
        &call(
            &state,
            grant.token.expose(),
            "polymas_get_knowledge_claim",
            json!({"namespace":"shared","claimId":visible}),
        )
        .await,
    );
    assert_eq!(detail["claim"]["value"]["sourceVersion"], "fixture-v1");
    for arguments in [
        json!({"namespace":"shared","claimId":hidden}),
        json!({"namespace":"private","claimId":visible}),
        json!({"namespace":"shared","claimId":visible,"agentId":"forged"}),
    ] {
        let denied = call(
            &state,
            grant.token.expose(),
            "polymas_get_knowledge_claim",
            arguments,
        )
        .await;
        assert_eq!(denied["isError"], true);
        assert!(!denied.to_string().contains("FIXTURE-MEMORY"));
    }
    let engine = state.engine.lock().await;
    let events = engine.workspace.db.events(0, 1000, None).unwrap();
    let reads: Vec<_> = events
        .iter()
        .filter(|e| e.input.kind == "knowledge.read_completed")
        .collect();
    assert_eq!(reads.len(), 2);
    assert!(reads.iter().all(|e| e.input.agent_id.as_deref()
        == Some(engine.scheduler.task(&task).unwrap().assignee.as_str())));
    assert!(!json!(events).to_string().contains("FIXTURE-MEMORY"));
}
#[tokio::test]
async fn memory_access_shrinks_immediately_and_cannot_expand_during_a_run() {
    for initially_enabled in [true, false] {
        let (_dir, state, task, _, _) = setup(initially_enabled).await;
        let grant = issue(&state, &task).await.unwrap();
        {
            let mut engine = state.engine.lock().await;
            let mut architecture = engine.workspace.config().value.architecture.clone();
            architecture.memory_enabled = !initially_enabled;
            let revision = engine.workspace.config().revision;
            engine
                .configure(revision, ConfigCommand::SetArchitecture { architecture })
                .unwrap();
        }
        assert_eq!(
            call(
                &state,
                grant.token.expose(),
                "polymas_search_knowledge",
                json!({"namespace":"shared"})
            )
            .await["isError"],
            true
        );
    }
    let (_dir, state, task, _, _) = setup(true).await;
    let grant = issue(&state, &task).await.unwrap();
    {
        let mut engine = state.engine.lock().await;
        let revision = engine.workspace.config().revision;
        engine
            .configure(revision, ConfigCommand::SetBindings { bindings: vec![] })
            .unwrap();
    }
    assert_eq!(
        call(
            &state,
            grant.token.expose(),
            "polymas_search_knowledge",
            json!({"namespace":"shared"})
        )
        .await["isError"],
        true
    );
}
