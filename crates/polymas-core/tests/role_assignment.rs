use polymas_core::{
    engine::Engine,
    registry::{AgentStatus, BatchRequest, RoleDraft},
    scheduler::TaskInput,
    topology::Target,
    workspace::ConfigCommand,
};

fn engine(path: &std::path::Path, count: usize) -> Engine {
    let mut engine = Engine::open(path).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Research".into(),
                    instruction: "Version one".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "assignment-fixture".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count,
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::ReviseRole {
                role_id: role["id"].as_str().unwrap().into(),
                expected_version: 1,
                draft: RoleDraft {
                    name: "Research".into(),
                    instruction: "Version two".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
}

#[test]
fn a_role_assignment_requires_a_bounded_unique_non_archived_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = engine(dir.path(), 3);
    let role = engine.workspace.config().value.registry.roles[0].id.clone();
    let ids: Vec<_> = engine
        .workspace
        .config()
        .value
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect();
    engine
        .configure(
            4,
            ConfigCommand::ArchiveAgent {
                agent_id: ids[2].clone(),
            },
        )
        .unwrap();
    let revision = engine.workspace.config().revision;
    let before = serde_json::to_value(engine.workspace.config()).unwrap();
    let head = engine.workspace.db.last_sequence().unwrap();
    for selection in [
        vec![],
        vec![ids[0].clone(), ids[0].clone()],
        vec![ids[0].clone(), "missing".into()],
        vec![ids[0].clone(), ids[2].clone()],
        vec![ids[0].clone(); 5001],
    ] {
        assert!(
            engine
                .configure(
                    revision,
                    ConfigCommand::PinRole {
                        agent_ids: selection,
                        role_id: role.clone(),
                        version: 2
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(engine.workspace.config()).unwrap(),
            before
        );
        assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    }
    for (rev, role_id, version) in [
        (revision - 1, role.clone(), 2),
        (revision, role.clone(), 99),
        (revision, "missing-role".into(), 1),
    ] {
        assert!(
            engine
                .configure(
                    rev,
                    ConfigCommand::PinRole {
                        agent_ids: vec![ids[0].clone()],
                        role_id,
                        version
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(engine.workspace.config()).unwrap(),
            before
        );
        assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    }
}

#[test]
fn assignment_changes_future_tasks_without_rewriting_running_queued_or_saved_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = engine(dir.path(), 2);
    let config = engine.workspace.config().value.clone();
    let ids: Vec<_> = config
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect();
    let role = config.registry.roles[0].id.clone();
    let input = TaskInput {
        prompt: "Assignment fixture".into(),
        target: Target::Agents { ids: ids.clone() },
        ..Default::default()
    };
    let old = engine.submit("before", input.clone(), 1).unwrap();
    let claimed = engine.claim(1, "fixture", 100).unwrap().remove(0);
    let old_tasks = old
        .task_ids
        .iter()
        .map(|id| engine.scheduler.task(id).unwrap().clone())
        .collect::<Vec<_>>();
    let head = engine.workspace.db.last_sequence().unwrap();
    engine
        .configure(
            4,
            ConfigCommand::PinRole {
                agent_ids: ids.clone(),
                role_id: role.clone(),
                version: 2,
            },
        )
        .unwrap();
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head + 1);
    for task in &old_tasks {
        assert_eq!(engine.scheduler.task(&task.id).unwrap(), task);
    }
    for (old, new) in config
        .registry
        .agents
        .iter()
        .zip(&engine.workspace.config().value.registry.agents)
    {
        let mut expected = serde_json::to_value(old).unwrap();
        expected["roleVersion"] = serde_json::json!(2);
        assert_eq!(serde_json::to_value(new).unwrap(), expected);
    }
    let fresh = engine.submit("after", input, 1).unwrap();
    for id in fresh.task_ids {
        assert_eq!(
            engine.scheduler.task(&id).unwrap().role.draft.instruction,
            "Version two"
        );
    }
    engine
        .finish(
            &claimed.id,
            claimed.lease.unwrap().epoch,
            Ok("Preserved result".into()),
        )
        .unwrap();
    drop(engine);
    let reopened = Engine::open(dir.path()).unwrap();
    for id in old.task_ids {
        assert_eq!(
            reopened.scheduler.task(&id).unwrap().role.draft.instruction,
            "Version one"
        );
    }
    assert_eq!(
        reopened
            .scheduler
            .task(&claimed.id)
            .unwrap()
            .result
            .as_deref(),
        Some("Preserved result")
    );
    assert!(
        reopened
            .workspace
            .config()
            .value
            .registry
            .agents
            .iter()
            .all(|a| a.role_version == 2)
    );
}

#[test]
fn five_thousand_assignments_preserve_identity_and_direct_registry_failure_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = engine(dir.path(), 5000);
    let config = engine.workspace.config().value.clone();
    let ids: Vec<_> = config
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect();
    let role = config.registry.roles[0].id.clone();
    engine
        .configure(
            4,
            ConfigCommand::PinRole {
                agent_ids: ids.clone(),
                role_id: role.clone(),
                version: 2,
            },
        )
        .unwrap();
    assert_eq!(engine.workspace.config().value.registry.agents.len(), 5000);
    for (before, after) in config
        .registry
        .agents
        .iter()
        .zip(&engine.workspace.config().value.registry.agents)
    {
        assert_eq!(before.id, after.id);
        assert_eq!(before.workspace_key, after.workspace_key);
        assert_eq!(before.session_id, after.session_id);
        assert_eq!(after.role_version, 2);
    }
    let mut registry = config.registry;
    registry.agents[1].status = AgentStatus::Archived;
    let before = serde_json::to_value(&registry).unwrap();
    assert!(registry.pin_role(&ids, &role, 2).is_err());
    assert_eq!(serde_json::to_value(registry).unwrap(), before);
}
