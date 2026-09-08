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
    let e = state.engine.lock().await;
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
}
