use polymas_core::{
    engine::Engine,
    registry::{BatchRequest, Registry, RoleDraft, Sandbox},
    scheduler::TaskInput,
    topology::Target,
    workspace::ConfigCommand,
};

fn seeded(path: &std::path::Path) -> (Engine, String) {
    let mut engine = Engine::open(path).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Исходная роль".into(),
                    description: "Первая инструкция".into(),
                    instruction: "Проверь источники 🧠".into(),
                    output_contract: "Markdown: источники".into(),
                    acceptance_criteria: "Две ссылки".into(),
                    escalation_rule: "Обратись к оператору".into(),
                    model: Some("fixture-model".into()),
                    sandbox: Sandbox::ReadOnly,
                    timeout_seconds: 123,
                    max_turns: 7,
                    max_output_tokens: Some(2048),
                },
            },
        )
        .unwrap();
    let id = role["id"].as_str().unwrap().to_string();
    engine
        .configure(
            2,
            ConfigCommand::ReviseRole {
                role_id: id.clone(),
                expected_version: 1,
                draft: RoleDraft {
                    name: "Изменённая роль".into(),
                    instruction: "Версия два".into(),
                    sandbox: Sandbox::WorkspaceWrite,
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::CreateAgents {
                key: "restore-fixture".into(),
                request: BatchRequest {
                    role_id: id.clone(),
                    role_version: 2,
                    count: 2,
                    name_pattern: "Экземпляр {n}".into(),
                },
            },
        )
        .unwrap();
    (engine, id)
}

#[test]
fn restoring_creates_a_new_version_and_preserves_assignments_tasks_history_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, role_id) = seeded(dir.path());
    let before = engine.workspace.config().value.clone();
    let ids = before
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    let path = dir.path().join("restore-sentinel.txt");
    std::fs::write(&path, "Preserved owner file").unwrap();
    let input = TaskInput {
        prompt: "Restore fixture".into(),
        target: Target::Agents { ids: ids.clone() },
        ..Default::default()
    };
    let old = engine.submit("old", input.clone(), 1).unwrap();
    let claimed = engine.claim(1, "fixture", 100).unwrap().remove(0);
    let tasks = old
        .task_ids
        .iter()
        .map(|id| engine.scheduler.task(id).unwrap().clone())
        .collect::<Vec<_>>();
    let head = engine.workspace.db.last_sequence().unwrap();
    let restored = engine
        .configure(
            4,
            ConfigCommand::RestoreRole {
                role_id: role_id.clone(),
                source_version: 1,
                expected_version: 2,
            },
        )
        .unwrap();
    assert_eq!(restored["sourceVersion"], 1);
    let role: polymas_core::registry::RoleVersion =
        serde_json::from_value(restored["role"].clone()).unwrap();
    assert_eq!(role.id, role_id);
    assert_eq!(role.version, 3);
    assert_eq!(role.draft, before.registry.roles[0].draft);
    assert_eq!(engine.workspace.config().revision, 5);
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head + 1);
    let events = engine.workspace.db.events(head, 10, None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].input.kind, "role.restored");
    assert_eq!(events[0].input.body["result"]["sourceVersion"], 1);
    let mut expected = serde_json::to_value(&before).unwrap();
    expected["registry"]["roles"]
        .as_array_mut()
        .unwrap()
        .push(restored["role"].clone());
    assert_eq!(
        serde_json::to_value(&engine.workspace.config().value).unwrap(),
        expected
    );
    for task in &tasks {
        assert_eq!(engine.scheduler.task(&task.id).unwrap(), task);
    }
    let after = engine.submit("still-v2", input.clone(), 1).unwrap();
    for task in &after.task_ids {
        assert_eq!(engine.scheduler.task(task).unwrap().role.version, 2);
    }
    engine
        .configure(
            5,
            ConfigCommand::PinRole {
                agent_ids: ids,
                role_id: role_id.clone(),
                version: 3,
            },
        )
        .unwrap();
    let new = engine.submit("restored-v3", input, 1).unwrap();
    for task in &new.task_ids {
        let pinned = &engine.scheduler.task(task).unwrap().role;
        assert_eq!(pinned.version, 3);
        assert_eq!(pinned.draft, before.registry.roles[0].draft);
    }
    engine
        .finish(
            &claimed.id,
            claimed.lease.unwrap().epoch,
            Ok("Old result".into()),
        )
        .unwrap();
    drop(engine);
    let reopened = Engine::open(dir.path()).unwrap();
    assert_eq!(
        reopened.workspace.config().value.registry.roles[0..2],
        before.registry.roles
    );
    assert_eq!(
        reopened
            .workspace
            .config()
            .value
            .registry
            .role(&role_id, 3)
            .unwrap(),
        &role
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "Preserved owner file"
    );
    assert_eq!(
        reopened
            .scheduler
            .task(&claimed.id)
            .unwrap()
            .result
            .as_deref(),
        Some("Old result")
    );
    assert!(
        reopened
            .workspace
            .history()
            .unwrap()
            .iter()
            .any(|h| h.value.revision == 5 && h.value.reason == "role.restored")
    );
}

#[test]
fn missing_current_future_stale_or_injected_restore_sources_never_mutate_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, role_id) = seeded(dir.path());
    let before = serde_json::to_value(engine.workspace.config()).unwrap();
    let head = engine.workspace.db.last_sequence().unwrap();
    for (revision, id, source, expected) in [
        (3, role_id.clone(), 1, 2),
        (4, role_id.clone(), 1, 1),
        (4, role_id.clone(), 0, 2),
        (4, role_id.clone(), 2, 2),
        (4, role_id.clone(), 3, 2),
        (4, "missing".into(), 1, 2),
    ] {
        assert!(
            engine
                .configure(
                    revision,
                    ConfigCommand::RestoreRole {
                        role_id: id,
                        source_version: source,
                        expected_version: expected
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
    let wire = serde_json::json!({"type":"restore_role","roleId":role_id,"sourceVersion":1,"expectedVersion":2});
    assert!(serde_json::from_value::<ConfigCommand>(wire.clone()).is_ok());
    let mut injected = wire;
    injected["draft"] = serde_json::json!({"instruction":"Unreviewed replacement"});
    assert!(serde_json::from_value::<ConfigCommand>(injected).is_err());
}

#[test]
fn exhausted_version_numbers_fail_without_panicking_or_reusing_an_identity() {
    let mut registry = Registry::default();
    let first = registry
        .create_role(RoleDraft {
            name: "Version guard".into(),
            instruction: "Original".into(),
            ..Default::default()
        })
        .unwrap();
    let mut latest = first.clone();
    latest.version = u32::MAX;
    registry.roles.push(latest);
    let before = serde_json::to_value(&registry).unwrap();
    assert!(registry.restore_role(&first.id, 1, u32::MAX).is_err());
    assert!(
        registry
            .revise_role(&first.id, u32::MAX, first.draft)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&registry).unwrap(), before);
}
