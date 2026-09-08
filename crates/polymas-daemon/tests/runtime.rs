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
impl Executor for FixtureExecutor {
    fn execute(
        &self,
        request: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        cancel: watch::Receiver<bool>,
    ) -> Execution {
        Box::pin(async move {
            let script = "import sys,json,uuid; sys.stdin.read(); print(json.dumps({'type':'thread.started','thread_id':str(uuid.uuid4())})); print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':'Actual child result'}})); print(json.dumps({'type':'turn.completed','usage':{'input_tokens':1,'output_tokens':2}}))";
            let (tx, mut rx) = mpsc::channel(32);
            let child = tokio::spawn(run_process(
                ProcessSpec {
                    executable: if cfg!(windows) { "python" } else { "python3" }.into(),
                    args: vec!["-u".into(), "-c".into(), script.into()],
                    workspace: request.workspace,
                    stdin: request.prompt,
                    timeout_seconds: 10,
                    environment: Default::default(),
                },
                tx,
                cancel,
            ));
            let mut stream = CodexStream::new(None);
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
    let handles = drive_once(&state, &options).await.unwrap();
    assert_eq!(handles.len(), 1);
    for h in handles {
        h.await.unwrap().unwrap();
    }
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
