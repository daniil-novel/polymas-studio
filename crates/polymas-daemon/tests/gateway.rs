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
    let roots = dir.path().join("knowledge");
    std::fs::create_dir(&roots).unwrap();
    std::fs::write(roots.join("guide.txt"), "GATEWAY-READ-93").unwrap();
    let mut connection = polymas_core::connections::Connection::files("Knowledge", vec![roots]);
    connection.tools = polymas_core::connections::files_tools();
    connection.tool_allowlist = vec!["read_text".into(), "list_files".into()];
    connection.health = polymas_core::connections::Health::Ready;
    connection.enabled = true;
    let connection_id = connection.id.clone();
    polymas_daemon::connection_api::persist_connection(
        &mut engine,
        0,
        connection,
        "fixture.connection",
    )
    .unwrap();
    let agent_id = engine.workspace.config().value.registry.agents[0]
        .id
        .clone();
    engine
        .configure(
            3,
            ConfigCommand::SetBindings {
                bindings: vec![polymas_core::policy::Binding {
                    id: "reader-binding".into(),
                    connection_id,
                    subject: polymas_core::policy::Subject::Agent { id: agent_id },
                    tools: ["read_text".into(), "list_files".into()].into(),
                    allow_write: false,
                    required: false,
                    namespace: "shared".into(),
                }],
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

async fn call(state: &AppState, token: &str, name: &str, arguments: Value) -> (StatusCode, Value) {
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
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn source_discovery_and_calls_apply_live_and_pinned_bindings_and_record_real_completion() {
    let (_dir, state, task_id) = setup().await;
    let grant = issue(&state, &task_id).await.unwrap();
    let (status, catalog) = call(
        &state,
        grant.token.expose(),
        "polymas_list_sources",
        json!({"query":"read_text","limit":1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["isError"], false);
    let catalog: Value =
        serde_json::from_str(catalog["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(catalog["items"].as_array().unwrap().len(), 1);
    let connection = catalog["items"][0]["connectionId"].as_str().unwrap();
    let input = json!({"connectionId":connection,"namespace":"shared","tool":"read_text","arguments":{"root":0,"path":"guide.txt"}});
    let (status, result) = call(
        &state,
        grant.token.expose(),
        "polymas_read_source",
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["content"][0]["text"], "GATEWAY-READ-93");
    let mut wrong_namespace = input.clone();
    wrong_namespace["namespace"] = json!("private");
    assert_eq!(
        call(
            &state,
            grant.token.expose(),
            "polymas_read_source",
            wrong_namespace
        )
        .await
        .1["isError"],
        true
    );
    let mut escape = input.clone();
    escape["arguments"]["path"] = json!("../outside.txt");
    assert_eq!(
        call(&state, grant.token.expose(), "polymas_read_source", escape)
            .await
            .1["isError"],
        true
    );
    let mut impersonate = input.clone();
    impersonate["agentId"] = json!("someone-else");
    assert_eq!(
        call(
            &state,
            grant.token.expose(),
            "polymas_read_source",
            impersonate
        )
        .await
        .1["isError"],
        true
    );
    let mut engine = state.engine.lock().await;
    let events = engine.workspace.db.events(0, 5000, None).unwrap();
    assert!(events.iter().any(|e| e.input.kind == "mcp.call_completed"));
    assert!(
        !json!(events).to_string().contains("GATEWAY-READ-93"),
        "Source content is not copied into the activity log"
    );
    let revision = engine.workspace.config().revision;
    engine
        .configure(revision, ConfigCommand::SetBindings { bindings: vec![] })
        .unwrap();
    drop(engine);
    assert_eq!(
        call(&state, grant.token.expose(), "polymas_read_source", input)
            .await
            .1["isError"],
        true
    );
}

#[tokio::test]
async fn a_real_stdio_bridge_initializes_lists_tools_and_reads_through_scoped_http() {
    use rmcp::{ServiceExt, model::CallToolRequestParams, transport::TokioChildProcess};
    let (_dir, state, task_id) = setup().await;
    let grant = issue(&state, &task_id).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let service_state = state.clone();
    let service = tokio::spawn(async move {
        axum::serve(listener, router(service_state)).await.unwrap();
    });
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_polymas-daemon"));
    command
        .args(["--gateway-port", &port.to_string()])
        .env("POLYMAS_GATEWAY_TOKEN", grant.token.expose())
        .env_remove("POLYMAS_CONTROL_TOKEN");
    let transport = TokioChildProcess::builder(command)
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap()
        .0;
    let mut client = tokio::time::timeout(std::time::Duration::from_secs(8), ().serve(transport))
        .await
        .unwrap()
        .expect("the trusted bridge must initialize as an MCP server");
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|t| t.name == "polymas_list_sources"));
    let catalog = client
        .call_tool(
            CallToolRequestParams::new("polymas_list_sources")
                .with_arguments(json!({"query":"read_text"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(catalog).unwrap();
    let catalog: Value =
        serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
    let connection = catalog["items"][0]["connectionId"].as_str().unwrap();
    let input = json!({"connectionId":connection,"namespace":"shared","tool":"read_text","arguments":{"root":0,"path":"guide.txt"}});
    let result = client
        .call_tool(
            CallToolRequestParams::new("polymas_read_source")
                .with_arguments(input.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap()["content"][0]["text"],
        "GATEWAY-READ-93"
    );
    drop(grant);
    let denied = client
        .call_tool(
            CallToolRequestParams::new("polymas_list_sources").with_arguments(Default::default()),
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(denied).unwrap()["isError"], true);
    client
        .close_with_timeout(std::time::Duration::from_secs(2))
        .await
        .unwrap();
    service.abort();
}

#[tokio::test]
async fn the_read_only_gateway_contract_cannot_execute_an_allowed_write_tool() {
    let definitions = polymas_daemon::gateway_tools::definitions();
    let read = definitions["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "polymas_read_source")
        .expect("read interface is explicit and separately annotated");
    assert_eq!(read["annotations"]["readOnlyHint"], true);
    assert_eq!(read["annotations"]["destructiveHint"], false);
    let (_dir, state, task_id) = setup().await;
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
    let mut stored = engine
        .workspace
        .db
        .list::<polymas_core::connections::Connection>("connections", 10)
        .unwrap()
        .remove(0);
    stored.value.transport = polymas_core::connections::ConnectionTransport::Stdio {
        command: env!("CARGO_BIN_EXE_polymas-daemon").into(),
        args: vec!["--help".into()],
    };
    stored.value.recipe = "custom".into();
    stored.value.read_only = false;
    stored
        .value
        .tools
        .push(polymas_core::connections::ToolDescription {
            name: "write_record".into(),
            description: "Write a record".into(),
            input_schema: json!({"type":"object"}),
            read_only: false,
        });
    stored.value.tool_allowlist.push("write_record".into());
    stored.value.validate().unwrap();
    let connection_id = stored.value.id.clone();
    polymas_daemon::connection_api::persist_connection(
        &mut engine,
        stored.revision,
        stored.value,
        "fixture.write_tool",
    )
    .unwrap();
    let mut bindings = engine.workspace.config().value.bindings.clone();
    bindings[0].allow_write = true;
    bindings[0].tools.insert("write_record".into());
    let revision = engine.workspace.config().revision;
    engine
        .configure(revision, ConfigCommand::SetBindings { bindings })
        .unwrap();
    let run = engine
        .submit(
            "write-test",
            TaskInput {
                prompt: "Verify write denial".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    engine
        .claim(1, "local", chrono::Utc::now().timestamp())
        .unwrap();
    let cursor = engine.workspace.db.last_sequence().unwrap();
    drop(engine);
    let grant = issue(&state, &run.task_ids[0]).await.unwrap();
    let (_, denied) = call(&state, grant.token.expose(), "polymas_read_source", json!({"connectionId":connection_id,"namespace":"shared","tool":"write_record","arguments":{}})).await;
    assert_eq!(denied["isError"], true);
    assert!(
        !state
            .engine
            .lock()
            .await
            .workspace
            .db
            .events(cursor, 100, None)
            .unwrap()
            .iter()
            .any(|e| e.input.kind == "mcp.call_started")
    );
}
