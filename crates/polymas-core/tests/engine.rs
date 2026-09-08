use polymas_core::engine::Engine;
use polymas_core::registry::{BatchRequest, RoleDraft};
use polymas_core::scheduler::{TaskInput, TaskState};
use polymas_core::workspace::ConfigCommand;
fn engine(path: &std::path::Path) -> Engine {
    let mut e = Engine::open(path).unwrap();
    let role = e
        .configure(
            e.workspace.config().revision,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "A".into(),
                    instruction: "Work".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    e.configure(
        e.workspace.config().revision,
        ConfigCommand::CreateAgents {
            key: "agents".into(),
            request: BatchRequest {
                role_id: role["id"].as_str().unwrap().into(),
                role_version: 1,
                count: 2,
                name_pattern: "A{n}".into(),
            },
        },
    )
    .unwrap();
    e
}
#[test]
fn durable_claim_becomes_unknown_after_restart_and_cannot_automatically_repeat() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = engine(dir.path());
    let run = e
        .submit(
            "r",
            TaskInput {
                prompt: "Make a result".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let claimed = e.claim(1, "local", 100).unwrap();
    assert_eq!(claimed.len(), 1);
    drop(e);
    let mut e = Engine::open(dir.path()).unwrap();
    assert_eq!(
        e.scheduler.task(&run.task_ids[0]).unwrap().state,
        TaskState::Unknown
    );
    assert!(e.claim(1, "local", 101).unwrap().is_empty());
    assert!(
        e.reconcile(&run.task_ids[0], false, "I checked external effects")
            .is_ok()
    );
    assert!(e.claim(1, "local", 102).unwrap().is_empty());
}
#[test]
fn pinned_run_bindings_and_roles_are_immutable_and_completion_requires_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = engine(dir.path());
    let run = e
        .submit(
            "r",
            TaskInput {
                prompt: "Work".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let task = e.claim(1, "local", 100).unwrap().remove(0);
    let role = task.role.clone();
    e.configure(
        e.workspace.config().revision,
        ConfigCommand::ReviseRole {
            role_id: role.id.clone(),
            expected_version: 1,
            draft: RoleDraft {
                instruction: "Changed".into(),
                ..role.draft
            },
        },
    )
    .unwrap();
    assert_eq!(
        e.scheduler.task(&task.id).unwrap().role.draft.instruction,
        "Work"
    );
    let epoch = task.lease.unwrap().epoch;
    e.finish(&task.id, epoch, Ok("Result".into())).unwrap();
    assert!(
        !e.scheduler
            .runs
            .iter()
            .find(|r| r.id == run.id)
            .unwrap()
            .accepted
    );
    e.accept(&task.id).unwrap();
    drop(e);
    let e = Engine::open(dir.path()).unwrap();
    assert!(
        e.scheduler
            .runs
            .iter()
            .find(|r| r.id == run.id)
            .unwrap()
            .accepted
    );
}
#[test]
fn active_agent_cannot_be_archived_and_a_failed_transition_does_not_mutate_queue() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = engine(dir.path());
    e.submit(
        "r",
        TaskInput {
            prompt: "Work".into(),
            ..Default::default()
        },
        1,
    )
    .unwrap();
    let task = e.claim(1, "local", 100).unwrap().remove(0);
    assert!(
        e.configure(
            e.workspace.config().revision,
            ConfigCommand::ArchiveAgent {
                agent_id: task.assignee.clone()
            }
        )
        .is_err()
    );
    let epoch = task.lease.unwrap().epoch;
    assert!(e.finish(&task.id, epoch + 1, Ok("old".into())).is_err());
    assert_eq!(
        e.scheduler.task(&task.id).unwrap().state,
        TaskState::Running
    );
}
