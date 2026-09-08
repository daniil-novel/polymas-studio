use polymas_core::{
    codex::{CodexRequest, CodexStream},
    process::{ProcessEvent, ProcessSpec, run_process},
};
use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, RoleDraft},
    scheduler::{TaskInput, TaskState},
    workspace::ConfigCommand,
};
use polymas_daemon::{
    AppState,
    runtime::{Execution, Executor, RuntimeOptions, drive_once},
};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
struct FixtureExecutor;

struct ApprovalExecutor {
    state: AppState,
    connection_id: String,
    sessions: Arc<std::sync::Mutex<Vec<Option<String>>>>,
}
impl Executor for ApprovalExecutor {
    fn execute(
        &self,
        request: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Execution {
        let state = self.state.clone();
        let connection_id = self.connection_id.clone();
        let sessions = self.sessions.clone();
        Box::pin(async move {
            let first = {
                let mut seen = sessions.lock().unwrap();
                seen.push(request.session_id.clone());
                seen.len() == 1
            };
            if first {
                let mut engine = state.engine.lock().await;
                let task = engine
                    .scheduler
                    .tasks
                    .iter()
                    .find(|t| t.state == TaskState::Running)
                    .unwrap()
                    .clone();
                engine.request_approval(
                    "fixture",
                    polymas_core::engine::OperationRequest {
                        task_id: task.id,
                        origin_epoch: task.lease.unwrap().epoch,
                        connection_id,
                        namespace: "fixture".into(),
                        tool: "write".into(),
                        arguments: serde_json::json!({"text":"runtime-fixture"}),
                    },
                    chrono::Utc::now().timestamp(),
                    60,
                )?;
            } else {
                assert!(request.prompt.contains("approvalId"));
                assert!(request.prompt.contains("polymas_operation_status"));
            }
            FixtureExecutor.execute(request, sender, cancel).await
        })
    }
}

#[tokio::test]
async fn owner_wait_reaps_process_publishes_partial_and_resumes_the_same_session() {
    use polymas_core::{
        connections::{Connection, ConnectionTransport, Health, ToolDescription},
        policy::{Approval, Binding, Subject},
    };
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Approval fixture".into(),
                    instruction: "Request a fixture operation".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let role_id = role["id"].as_str().unwrap().to_owned();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "one".into(),
                request: BatchRequest {
                    role_id: role_id.clone(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "Writer".into(),
                },
            },
        )
        .unwrap();
    let mut connection = Connection::files("Fixture", vec![]);
    connection.transport = ConnectionTransport::Http {
        url: "https://fixture.invalid/mcp".into(),
    };
    connection.enabled = true;
    connection.health = Health::Ready;
    connection.read_only = false;
    connection.tools = vec![ToolDescription {
        name: "write".into(),
        description: "Fixture write".into(),
        input_schema: serde_json::json!({"type":"object"}),
        read_only: false,
    }];
    connection.tool_allowlist = vec!["write".into()];
    let connection_id = connection.id.clone();
    engine
        .workspace
        .db
        .transaction(|tx| tx.put("connections", &connection.id, 0, &connection))
        .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::SetBindings {
                bindings: vec![Binding {
                    id: "writer".into(),
                    connection_id: connection_id.clone(),
                    subject: Subject::Role { id: role_id },
                    namespace: "fixture".into(),
                    tools: ["write".into()].into(),
                    allow_write: true,
                    required: false,
                }],
            },
        )
        .unwrap();
    let run = engine
        .submit(
            "run",
            TaskInput {
                prompt: "Request fixture operation".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let task_id = run.task_ids[0].clone();
    let state = AppState::new(
        engine,
        "test-runtime-approval-control-token-32".into(),
        vec![],
    )
    .unwrap();
    let sessions = Arc::new(std::sync::Mutex::new(vec![]));
    let options = RuntimeOptions::new(
        Arc::new(ApprovalExecutor {
            state: state.clone(),
            connection_id,
            sessions: sessions.clone(),
        }),
        1,
    )
    .unwrap();
    for handle in drive_once(&state, &options).await.unwrap() {
        handle.await.unwrap().unwrap();
    }
    assert!(state.active.lock().await.is_empty());
    let (session_id, reserved_epoch, approval) = {
        let engine = state.engine.lock().await;
        let task = engine.scheduler.task(&task_id).unwrap();
        assert_eq!(task.state, TaskState::WaitingUser);
        assert!(task.lease.is_none());
        let artifact = engine
            .workspace
            .db
            .get::<polymas_core::artifacts::Artifact>("artifacts", &task.artifact_ids[0])
            .unwrap()
            .unwrap();
        assert!(artifact.value.partial);
        let runtime = engine
            .workspace
            .db
            .get::<polymas_daemon::runtime::AgentRuntime>("task_runtime", &task_id)
            .unwrap()
            .unwrap()
            .value;
        let approval = engine
            .workspace
            .db
            .get::<Approval>(
                "approvals",
                &task.approval_wait.as_ref().unwrap().approval_id,
            )
            .unwrap()
            .unwrap()
            .value;
        (
            runtime.session_id.unwrap(),
            task.continuation.as_ref().unwrap().epoch,
            approval,
        )
    };
    assert!(drive_once(&state, &options).await.unwrap().is_empty());
    state
        .engine
        .lock()
        .await
        .decide_approval(
            &approval.id,
            1,
            &approval.fingerprint,
            false,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
    for handle in drive_once(&state, &options).await.unwrap() {
        handle.await.unwrap().unwrap();
    }
    assert_eq!(*sessions.lock().unwrap(), vec![None, Some(session_id)]);
    let engine = state.engine.lock().await;
    let task = engine.scheduler.task(&task_id).unwrap();
    assert_eq!(task.state, TaskState::AwaitingAcceptance);
    assert_eq!(task.lease.as_ref().unwrap().epoch, reserved_epoch);
    assert_eq!(task.attempt, 2);
}
impl Executor for FixtureExecutor {
    fn execute(
        &self,
        request: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Execution {
        Box::pin(async move {
            let script = "import sys,json,uuid; sys.stdin.read(); print(json.dumps({'type':'thread.started','thread_id':sys.argv[1] or str(uuid.uuid4())})); print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':'Actual child result'}})); print(json.dumps({'type':'turn.completed','usage':{'input_tokens':1,'output_tokens':2}}))";
            let (tx, mut rx) = mpsc::channel(32);
            let child = tokio::spawn(run_process(
                ProcessSpec {
                    executable: if cfg!(windows) { "python" } else { "python3" }.into(),
                    args: vec![
                        "-u".into(),
                        "-c".into(),
                        script.into(),
                        request.session_id.clone().unwrap_or_default(),
                    ],
                    workspace: request.workspace,
                    stdin: request.prompt,
                    timeout_seconds: 10,
                    environment: Default::default(),
                },
                tx,
                cancel,
            ));
            let mut stream = CodexStream::new(request.session_id);
            while let Some(event) = rx.recv().await {
                if event.kind == "runtime.event" {
                    stream.observe(&event.body)?;
                }
                sender.send(event).await.unwrap();
            }
            let outcome = child.await.unwrap()?;
            assert!(outcome.success);
            let mut result = stream.finish()?;
            result.pid = outcome.pid;
            Ok(result)
        })
    }
}

struct GatewayExecutor {
    token: Arc<std::sync::Mutex<Option<String>>>,
    port: u16,
}
impl Executor for GatewayExecutor {
    fn execute(
        &self,
        request: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Execution {
        let token_out = self.token.clone();
        let port = self.port;
        Box::pin(async move {
            let launch = request
                .gateway
                .as_ref()
                .expect("each configured attempt needs its trusted gateway");
            assert_eq!(
                launch.args,
                vec!["--gateway-port".to_owned(), port.to_string()]
            );
            assert_eq!(launch.env_vars, vec!["POLYMAS_GATEWAY_TOKEN"]);
            assert!(!request.environment.contains_key("POLYMAS_CONTROL_TOKEN"));
            let token = request
                .environment
                .get("POLYMAS_GATEWAY_TOKEN")
                .expect("scoped credential")
                .clone();
            let response = reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(format!("http://127.0.0.1:{port}/gateway/context"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::OK);
            let context: serde_json::Value = response.json().await.unwrap();
            for field in ["agentId", "taskId", "runId"] {
                assert!(
                    request.prompt.contains(context[field].as_str().unwrap()),
                    "the trusted attempt context must identify {field}"
                );
            }
            assert!(!request.prompt.contains(&token));
            *token_out.lock().unwrap() = Some(token);
            FixtureExecutor.execute(request, sender, cancel).await
        })
    }
}
#[tokio::test]
async fn execution_injects_only_attempt_authority_and_revokes_it_after_process_completion() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Bridge test".into(),
                    instruction: "Return a result".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "one".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "Agent".into(),
                },
            },
        )
        .unwrap();
    engine
        .submit(
            "run",
            TaskInput {
                prompt: "Run fixture".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let state = AppState::new(
        engine,
        "test-runtime-control-token-32-characters".into(),
        vec![],
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    state
        .configure_gateway(port, env!("CARGO_BIN_EXE_polymas-daemon").into())
        .await
        .unwrap();
    let server_state = state.clone();
    let service = tokio::spawn(async move {
        axum::serve(listener, polymas_daemon::router(server_state))
            .await
            .unwrap();
    });
    let token = Arc::new(std::sync::Mutex::new(None));
    let options = RuntimeOptions::new(
        Arc::new(GatewayExecutor {
            token: token.clone(),
            port,
        }),
        1,
    )
    .unwrap();
    for handle in drive_once(&state, &options).await.unwrap() {
        handle.await.unwrap().unwrap();
    }
    let token = token.lock().unwrap().clone().unwrap();
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://127.0.0.1:{port}/gateway/context"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    service.abort();
}

#[test]
fn inbox_budget_accounts_for_json_escaping_without_starving_a_valid_full_size_message() {
    use polymas_core::{
        messages::{MessageInput, Router},
        storage::Store,
        topology::{Architecture, Edge},
    };
    let mut db = Store::memory().unwrap();
    let architecture = Architecture {
        edges: vec![Edge {
            from: "a".into(),
            to: "b".into(),
        }],
        ..Default::default()
    };
    for n in 0..3 {
        Router::send(
            &mut db,
            &architecture,
            "run",
            "a",
            &n.to_string(),
            MessageInput {
                recipient: "b".into(),
                body: "\u{0001}".repeat(64 * 1024),
                ..Default::default()
            },
            100,
        )
        .unwrap();
    }
    let pending = Router::pending_for_run(&db, "run", "b", 101, 100).unwrap();
    let expected_id = pending[0].id.clone();
    let batch = polymas_daemon::runtime::bounded_inbox(pending).unwrap();
    assert_eq!(
        batch.len(),
        1,
        "one valid 64 KiB message must fit even after JSON escaping"
    );
    assert_eq!(batch[0].id, expected_id);
    assert!(serde_json::to_vec(&batch).unwrap().len() <= 512 * 1024);
}
#[tokio::test]
async fn real_child_execution_updates_durable_pid_session_result_and_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Fixture".into(),
                    instruction: "Return a result".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "one".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "A".into(),
                },
            },
        )
        .unwrap();
    let run = engine
        .submit(
            "task",
            TaskInput {
                prompt: "Execute fixture".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let state = AppState::new(
        engine,
        "test-runtime-control-token-32-characters".into(),
        vec![],
    )
    .unwrap();
    // This is a protocol fixture with a real OS process, not an OpenAI test.
    let options = RuntimeOptions::new(Arc::new(FixtureExecutor), 1).unwrap();
    state.require_adoption();
    assert!(
        drive_once(&state, &options).await.unwrap().is_empty(),
        "unadopted service must not launch user work"
    );
    state.adopt().unwrap();
    let handles = drive_once(&state, &options).await.unwrap();
    assert_eq!(handles.len(), 1);
    for h in handles {
        h.await.unwrap().unwrap();
    }
    state.require_adoption();
    assert!(state.expire_adoption());
    assert!(
        state.adopt().is_err(),
        "expired startup cannot win a late adoption race"
    );
    assert!(!state.is_adopted());
    let mut e = state.engine.lock().await;
    let t = e.scheduler.task(&run.task_ids[0]).unwrap();
    assert_eq!(t.state, TaskState::AwaitingAcceptance);
    assert_eq!(t.result.as_deref(), Some("Actual child result"));
    assert_eq!(t.artifact_ids.len(), 1);
    assert!(t.lease.as_ref().unwrap().pid.unwrap() > 0);
    let runtime = e
        .workspace
        .db
        .get::<serde_json::Value>("agent_runtime", &t.assignee)
        .unwrap()
        .unwrap()
        .value;
    assert!(runtime["sessionId"].as_str().unwrap().len() > 20);
    let events = e.workspace.db.events(0, 100, None).unwrap();
    assert!(
        events
            .iter()
            .any(|e| e.input.kind == "runtime.input_delivered")
    );
    let artifact_id = t.artifact_ids[0].clone();
    let before = e
        .workspace
        .db
        .get::<polymas_core::artifacts::Artifact>("artifacts", &artifact_id)
        .unwrap()
        .unwrap();
    assert!(!before.value.accepted);
    let cursor = e.workspace.db.last_sequence().unwrap();
    e.scheduler
        .task_mut(&run.task_ids[0])
        .unwrap()
        .artifact_ids
        .push("missing-artifact".into());
    assert!(e.accept(&run.task_ids[0]).is_err());
    assert_eq!(
        e.workspace.db.last_sequence().unwrap(),
        cursor,
        "failed acceptance must roll back artifact events too"
    );
    assert!(
        !e.workspace
            .db
            .get::<polymas_core::artifacts::Artifact>("artifacts", &artifact_id)
            .unwrap()
            .unwrap()
            .value
            .accepted
    );
    assert_eq!(
        e.scheduler.task(&run.task_ids[0]).unwrap().state,
        TaskState::AwaitingAcceptance
    );
    e.scheduler
        .task_mut(&run.task_ids[0])
        .unwrap()
        .artifact_ids
        .pop();
    e.accept(&run.task_ids[0]).unwrap();
    let after = e
        .workspace
        .db
        .get::<polymas_core::artifacts::Artifact>("artifacts", &artifact_id)
        .unwrap()
        .unwrap();
    assert!(
        after.value.accepted,
        "accepted task and its result artifact must agree"
    );
}
