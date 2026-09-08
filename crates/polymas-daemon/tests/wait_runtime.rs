use polymas_core::{
    codex::{CodexRequest, CodexResult},
    engine::Engine,
    process::ProcessEvent,
    registry::{BatchRequest, RoleDraft},
    scheduler::{TaskInput, TaskState},
    topology::{Edge, Target},
    workspace::ConfigCommand,
};
use polymas_daemon::{
    AppState,
    runtime::{Execution, Executor, RuntimeOptions, drive_once},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};

struct WaitExecutor {
    port: u16,
    agents: Vec<String>,
    sessions: Arc<Mutex<Vec<String>>>,
}
impl Executor for WaitExecutor {
    fn execute(
        &self,
        request: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        _cancel: watch::Receiver<bool>,
    ) -> Execution {
        let port = self.port;
        let agents = self.agents.clone();
        let sessions = self.sessions.clone();
        Box::pin(async move {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let token = &request.environment["POLYMAS_GATEWAY_TOKEN"];
            let context: Value = client
                .get(format!("http://127.0.0.1:{port}/gateway/context"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            let count = sessions.lock().unwrap().len();
            let mut first_session = None;
            let call = if count == 0 {
                assert_eq!(context["agentId"], agents[0]);
                assert!(request.session_id.is_none());
                Some(
                    json!({"name":"polymas_wait_for_message","arguments":{"sender":agents[1],"timeoutSeconds":60,"summary":"Need fixture evidence"}}),
                )
            } else if count == 1 {
                assert_eq!(context["agentId"], agents[1]);
                assert!(request.session_id.is_none());
                Some(
                    json!({"name":"polymas_send_message","arguments":{"idempotencyKey":"wake-once","message":{"recipient":agents[0],"body":"WAIT-RESUME-73"}}}),
                )
            } else {
                assert_eq!(context["agentId"], agents[0]);
                let expected = sessions.lock().unwrap()[0].clone();
                assert_eq!(
                    request.session_id.as_deref(),
                    Some(expected.as_str()),
                    "newSession applies only to the initial turn; continuation must resume the exact same session"
                );
                assert!(request.prompt.contains("WAIT-RESUME-73"));
                assert!(request.prompt.contains("ПРИЧИНА ПРОДОЛЖЕНИЯ"));
                first_session = Some(expected);
                None
            };
            if let Some(body) = call {
                let result: Value = client
                    .post(format!("http://127.0.0.1:{port}/gateway/call"))
                    .bearer_auth(token)
                    .json(&body)
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                assert_eq!(result["isError"], false, "{result}");
            }
            // This executor models a successfully reaped turn, not a provider or OS process.
            let session_id = first_session.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            sender
                .send(ProcessEvent {
                    kind: "runtime.input_delivered".into(),
                    body: json!({"pid":1000+count}),
                })
                .await
                .unwrap();
            sessions.lock().unwrap().push(session_id.clone());
            Ok(CodexResult {
                session_id,
                pid: 1000 + count as u32,
                output: "Fixture turn complete".into(),
                usage: None,
            })
        })
    }
}
#[tokio::test]
async fn yielding_releases_capacity_and_message_wakes_the_exact_same_agent_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Wait role".into(),
                    instruction: "Use peer evidence".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "pair".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 2,
                    name_pattern: "Peer {n}".into(),
                },
            },
        )
        .unwrap();
    let agents: Vec<_> = engine
        .workspace
        .config()
        .value
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect();
    let mut architecture = engine.workspace.config().value.architecture.clone();
    architecture.edges = vec![Edge {
        from: agents[1].clone(),
        to: agents[0].clone(),
    }];
    engine
        .configure(3, ConfigCommand::SetArchitecture { architecture })
        .unwrap();
    let run = engine
        .submit(
            "wait",
            TaskInput {
                prompt: "Exchange evidence".into(),
                target: Target::Agents {
                    ids: agents.clone(),
                },
                new_session: true,
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let state = AppState::new(
        engine,
        "test-wait-control-32-characters-token".into(),
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
    let sessions = Arc::new(Mutex::new(vec![]));
    let options = RuntimeOptions::new(
        Arc::new(WaitExecutor {
            port,
            agents: agents.clone(),
            sessions: sessions.clone(),
        }),
        1,
    )
    .unwrap();
    for n in 0..3 {
        let handles = drive_once(&state, &options).await.unwrap();
        assert_eq!(handles.len(), 1, "turn {n}");
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        let mut engine = state.engine.lock().await;
        let task = engine.scheduler.task(&run.task_ids[0]).unwrap();
        if n < 2 {
            assert_eq!(task.state, TaskState::Waiting);
            assert!(task.lease.is_none());
            let artifact = engine
                .workspace
                .db
                .get::<polymas_core::artifacts::Artifact>("artifacts", &task.artifact_ids[0])
                .unwrap()
                .unwrap()
                .value;
            assert!(artifact.partial && !artifact.accepted);
        } else {
            assert_eq!(task.state, TaskState::AwaitingAcceptance);
            assert_eq!(task.attempt, 2);
            assert_eq!(task.assignee, agents[0]);
        }
        if n == 1 {
            // Another task may have used a new conversation for this same agent while it waited.
            let stored = engine
                .workspace
                .db
                .get::<polymas_daemon::runtime::AgentRuntime>("agent_runtime", &agents[0])
                .unwrap()
                .unwrap();
            let mut runtime = stored.value;
            runtime.session_id = Some(uuid::Uuid::new_v4().to_string());
            runtime.task_id = Some("other-task".into());
            engine
                .workspace
                .db
                .transaction(|tx| {
                    tx.put("agent_runtime", &agents[0], stored.revision, &runtime)?;
                    Ok(())
                })
                .unwrap();
        }
        drop(engine);
        assert!(state.active.lock().await.is_empty());
    }
    let sessions = sessions.lock().unwrap();
    assert_ne!(sessions[0], sessions[1]);
    assert_eq!(sessions[0], sessions[2]);
    service.abort();
}
