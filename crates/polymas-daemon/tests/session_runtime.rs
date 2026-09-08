use polymas_core::{
    codex::{CodexRequest, CodexResult},
    engine::Engine,
    error::Error,
    process::ProcessEvent,
    registry::{BatchRequest, RoleDraft},
    scheduler::{TaskInput, TaskState},
    topology::Target,
    workspace::ConfigCommand,
};
use polymas_daemon::{
    AppState,
    runtime::{AgentRuntime, Execution, Executor, RuntimeOptions, drive_once},
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::{mpsc, watch};
const OLD: &str = "0199a213-81c0-7800-8aa1-bbab2a035a53";
const NEW: &str = "0199a213-81c0-7800-8aa1-bbab2a035a54";
struct SessionExecutor {
    calls: Arc<AtomicUsize>,
    fail: bool,
}
impl Executor for SessionExecutor {
    fn execute(
        &self,
        _: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        _: watch::Receiver<bool>,
    ) -> Execution {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let fail = self.fail;
        Box::pin(async move {
            if fail {
                sender
                    .send(ProcessEvent {
                        kind: "runtime.session".into(),
                        body: json!({"thread_id":OLD}),
                    })
                    .await
                    .unwrap();
                return Err(Error::Runtime(
                    "Fixture turn failed after session started".into(),
                ));
            }
            Ok(CodexResult {
                session_id: OLD.into(),
                output: "Fixture output".into(),
                usage: None,
                pid: 917,
            })
        })
    }
}
fn setup() -> (tempfile::TempDir, Engine, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Session fixture".into(),
                    instruction: "Return a fixture result".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "two".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 2,
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap();
    let ids = engine
        .workspace
        .config()
        .value
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect();
    (dir, engine, ids)
}

#[tokio::test]
async fn historical_foreign_session_is_denied_before_resume_and_before_result_acceptance() {
    for forged_resume in [true, false] {
        let (_dir, mut engine, ids) = setup();
        engine
            .workspace
            .db
            .transaction(|tx| {
                tx.put(
                    "agent_runtime",
                    &ids[0],
                    0,
                    &AgentRuntime {
                        agent_id: ids[0].clone(),
                        session_id: Some(NEW.into()),
                        ..Default::default()
                    },
                )?;
                tx.put(
                    "task_runtime",
                    "old-task",
                    0,
                    &AgentRuntime {
                        agent_id: ids[0].clone(),
                        session_id: Some(OLD.into()),
                        task_id: Some("old-task".into()),
                        ..Default::default()
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let state = AppState::new(
            engine,
            "session-fixture-control-token-32-chars".into(),
            vec![],
        )
        .unwrap();
        let task_id = {
            let mut engine = state.engine.lock().await;
            if forged_resume {
                engine
                    .workspace
                    .db
                    .transaction(|tx| {
                        tx.put(
                            "agent_runtime",
                            &ids[1],
                            0,
                            &AgentRuntime {
                                agent_id: ids[1].clone(),
                                session_id: Some(OLD.into()),
                                ..Default::default()
                            },
                        )?;
                        Ok(())
                    })
                    .unwrap();
            }
            engine
                .submit(
                    "target",
                    TaskInput {
                        prompt: "Fixture task".into(),
                        target: Target::Agent { id: ids[1].clone() },
                        ..Default::default()
                    },
                    1,
                )
                .unwrap()
                .task_ids[0]
                .clone()
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let options = RuntimeOptions::new(
            Arc::new(SessionExecutor {
                calls: calls.clone(),
                fail: false,
            }),
            1,
        )
        .unwrap();
        let handles = drive_once(&state, &options).await.unwrap();
        assert_eq!(handles.len(), 1);
        for handle in handles {
            assert!(handle.await.unwrap().is_err());
        }
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(!forged_resume));
        assert_eq!(
            state
                .engine
                .lock()
                .await
                .scheduler
                .task(&task_id)
                .unwrap()
                .state,
            TaskState::Failed
        );
        assert!(state.active.lock().await.is_empty());
    }
}

#[tokio::test]
async fn a_session_announced_before_a_failed_turn_is_still_saved_for_that_task() {
    let (_dir, mut engine, ids) = setup();
    let task_id = engine
        .submit(
            "first",
            TaskInput {
                prompt: "Fixture task".into(),
                target: Target::Agent { id: ids[0].clone() },
                ..Default::default()
            },
            1,
        )
        .unwrap()
        .task_ids[0]
        .clone();
    let state = AppState::new(
        engine,
        "session-fixture-control-token-32-chars".into(),
        vec![],
    )
    .unwrap();
    let options = RuntimeOptions::new(
        Arc::new(SessionExecutor {
            calls: Default::default(),
            fail: true,
        }),
        1,
    )
    .unwrap();
    for handle in drive_once(&state, &options).await.unwrap() {
        handle.await.unwrap().unwrap();
    }
    let engine = state.engine.lock().await;
    let saved = engine
        .workspace
        .db
        .get::<AgentRuntime>("task_runtime", &task_id)
        .unwrap()
        .expect("session must survive a failed turn");
    assert_eq!(saved.value.session_id.as_deref(), Some(OLD));
    assert_eq!(saved.value.agent_id, ids[0]);
    assert_eq!(
        engine.scheduler.task(&task_id).unwrap().state,
        TaskState::Failed
    );
}

struct ReapingExecutor(Arc<AtomicBool>);
impl Executor for ReapingExecutor {
    fn execute(
        &self,
        _: CodexRequest,
        sender: mpsc::Sender<ProcessEvent>,
        mut cancel: watch::Receiver<bool>,
    ) -> Execution {
        let reaped = self.0.clone();
        // A detached worker mirrors the process adapter: dropping its join future
        // requests cancellation but does not itself prove that the child exited.
        let worker = tokio::spawn(async move {
            sender
                .send(ProcessEvent {
                    kind: "runtime.session".into(),
                    body: json!({"thread_id":OLD}),
                })
                .await
                .unwrap();
            while !*cancel.borrow_and_update() {
                if cancel.changed().await.is_err() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            reaped.store(true, Ordering::SeqCst);
            Err(Error::Runtime("Fixture child reaped".into()))
        });
        Box::pin(async move { worker.await.unwrap() })
    }
}

#[tokio::test]
async fn rejected_session_event_cancels_and_reaps_before_releasing_the_agent() {
    let (_dir, mut engine, ids) = setup();
    engine
        .workspace
        .db
        .transaction(|tx| polymas_core::sessions::SessionRegistry::register(tx, OLD, &ids[0]))
        .unwrap();
    let task_id = engine
        .submit(
            "second",
            TaskInput {
                prompt: "Fixture task".into(),
                target: Target::Agent { id: ids[1].clone() },
                ..Default::default()
            },
            1,
        )
        .unwrap()
        .task_ids[0]
        .clone();
    let state = AppState::new(
        engine,
        "session-fixture-control-token-32-chars".into(),
        vec![],
    )
    .unwrap();
    let reaped = Arc::new(AtomicBool::new(false));
    let options = RuntimeOptions::new(Arc::new(ReapingExecutor(reaped.clone())), 1).unwrap();
    for handle in drive_once(&state, &options).await.unwrap() {
        let error = tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("event rejection must cancel the worker")
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("другому"), "{error}");
    }
    assert!(reaped.load(Ordering::SeqCst), "agent released before reap");
    assert!(state.active.lock().await.is_empty());
    let engine = state.engine.lock().await;
    assert_eq!(
        engine.scheduler.task(&task_id).unwrap().state,
        TaskState::Failed
    );
    assert!(
        engine
            .workspace
            .db
            .get::<AgentRuntime>("task_runtime", &task_id)
            .unwrap()
            .is_none()
    );
}
