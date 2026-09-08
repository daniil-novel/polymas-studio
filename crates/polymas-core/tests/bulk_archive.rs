use polymas_core::engine::Engine;
use polymas_core::registry::{AgentStatus, BatchRequest, RoleDraft};
use polymas_core::scheduler::{TaskInput, TaskState};
use polymas_core::topology::{Edge, Holon, Preset, Target};
use polymas_core::workspace::{ConfigCommand, Workspace};
use serde_json::json;

#[test]
fn archiving_removes_only_selected_agent_bindings_and_preserves_shared_memory() {
    use polymas_core::connections::Connection;
    use polymas_core::policy::{Binding, Subject};
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, ids) = setup(dir.path(), 3);
    let mut connection = Connection::files("Authored source", vec![dir.path().to_owned()]);
    // Authored discovery record: this test does not perform an MCP probe.
    connection.tools = polymas_core::connections::files_tools();
    connection.health = polymas_core::connections::Health::Ready;
    engine
        .workspace
        .db
        .transaction(|tx| tx.put("connections", &connection.id, 0, &connection))
        .unwrap();
    let role = engine.workspace.config().value.registry.agents[0]
        .role_id
        .clone();
    let bindings = [
        Subject::Agent { id: ids[0].clone() },
        Subject::Agent { id: ids[2].clone() },
        Subject::Role { id: role },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, subject)| Binding {
        id: format!("binding-{i}"),
        subject,
        connection_id: connection.id.clone(),
        tools: std::collections::BTreeSet::from(["read_text".into()]),
        allow_write: false,
        required: false,
        namespace: "fixture".into(),
    })
    .collect();
    engine
        .configure(3, ConfigCommand::SetBindings { bindings })
        .unwrap();
    let mut architecture = engine.workspace.config().value.architecture.clone();
    architecture.memory_enabled = true;
    architecture.memory_connection_id = Some(connection.id.clone());
    engine
        .configure(4, ConfigCommand::SetArchitecture { architecture })
        .unwrap();
    engine
        .configure(
            5,
            ConfigCommand::ArchiveAgents {
                agent_ids: ids[..2].to_vec(),
            },
        )
        .unwrap();
    let config = &engine.workspace.config().value;
    assert_eq!(
        config
            .bindings
            .iter()
            .map(|binding| binding.id.as_str())
            .collect::<Vec<_>>(),
        vec!["binding-1", "binding-2"]
    );
    assert!(config.architecture.memory_enabled);
    assert_eq!(
        config.architecture.memory_connection_id,
        Some(connection.id.clone())
    );
    let stored = engine
        .workspace
        .db
        .get::<Connection>("connections", &connection.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.revision, 1);
    assert_eq!(
        serde_json::to_value(stored.value).unwrap(),
        serde_json::to_value(connection).unwrap()
    );
}

fn setup(path: &std::path::Path, count: usize) -> (Engine, Vec<String>) {
    let mut engine = Engine::open(path).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Role".into(),
                    instruction: "Work".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let result = engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "fixture".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count,
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap();
    (
        engine,
        serde_json::from_value(result["ids"].clone()).unwrap(),
    )
}

#[test]
fn a_batch_is_one_revision_and_preserves_every_identity_and_prior_architecture() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, ids) = setup(dir.path(), 4);
    let mut architecture = engine.workspace.config().value.architecture.clone();
    architecture.preset = Preset::Star;
    architecture.max_concurrency = 4;
    architecture.coordinator_agent_id = Some(ids[0].clone());
    architecture.edges = vec![
        Edge {
            from: ids[0].clone(),
            to: ids[2].clone(),
        },
        Edge {
            from: ids[2].clone(),
            to: ids[3].clone(),
        },
    ];
    architecture.positions.insert(ids[0].clone(), [10., 20.]);
    architecture.positions.insert(ids[3].clone(), [30., 40.]);
    architecture.holons = vec![Holon {
        id: "group".into(),
        name: "Group".into(),
        goal: "Goal".into(),
        parent_id: None,
        members: ids.clone(),
        coordinator_agent_id: Some(ids[1].clone()),
        input_contract: "".into(),
        output_contract: "".into(),
        max_tasks: 10,
    }];
    engine
        .configure(
            3,
            ConfigCommand::SetArchitecture {
                architecture: architecture.clone(),
            },
        )
        .unwrap();
    let before = engine.workspace.config().value.registry.agents.clone();
    let directory = engine.workspace.agent_workspace(&ids[0]).unwrap();
    std::fs::write(directory.join("result.txt"), "authored result").unwrap();
    let sequence = engine.workspace.db.last_sequence().unwrap();
    let result = engine
        .configure(
            4,
            ConfigCommand::ArchiveAgents {
                agent_ids: ids[..2].to_vec(),
            },
        )
        .unwrap();
    assert_eq!(result["ids"], json!(&ids[..2]));
    assert_eq!(engine.workspace.config().revision, 5);
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), sequence + 1);
    drop(engine);
    let workspace = Workspace::open(dir.path()).unwrap();
    let config = &workspace.config().value;
    for (i, agent) in config.registry.agents.iter().enumerate() {
        let mut expected = serde_json::to_value(&before[i]).unwrap();
        if i < 2 {
            expected["status"] = json!("archived");
        }
        assert_eq!(serde_json::to_value(agent).unwrap(), expected);
    }
    assert_eq!(config.architecture.preset, Preset::Custom);
    assert_eq!(config.architecture.max_concurrency, 2);
    assert_eq!(config.architecture.coordinator_agent_id, None);
    assert_eq!(config.architecture.holons[0].members, ids[2..]);
    assert_eq!(config.architecture.holons[0].coordinator_agent_id, None);
    assert_eq!(
        config.architecture.edges,
        vec![architecture.edges[1].clone()]
    );
    assert!(!config.architecture.positions.contains_key(&ids[0]));
    assert_eq!(config.architecture.positions[&ids[3]], [30., 40.]);
    assert_eq!(
        std::fs::read_to_string(directory.join("result.txt")).unwrap(),
        "authored result"
    );
    let history = workspace.history().unwrap();
    assert_eq!(
        history
            .iter()
            .find(|entry| entry.value.revision == 4)
            .unwrap()
            .value
            .configuration
            .architecture,
        architecture
    );
    assert_eq!(
        history
            .iter()
            .find(|entry| entry.value.revision == 5)
            .unwrap()
            .value
            .reason,
        "agents.archived"
    );
}

#[test]
fn invalid_batch_or_one_unfinished_attempt_never_archives_a_partial_selection() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, ids) = setup(dir.path(), 3);
    engine
        .submit(
            "one-task",
            TaskInput {
                prompt: "Work".into(),
                target: Target::Agent { id: ids[1].clone() },
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let before = serde_json::to_value(engine.workspace.config()).unwrap();
    let sequence = engine.workspace.db.last_sequence().unwrap();
    for state in [
        TaskState::Queued,
        TaskState::Waiting,
        TaskState::WaitingUser,
        TaskState::Running,
        TaskState::Stopping,
        TaskState::Unknown,
    ] {
        engine.scheduler.tasks[0].state = state;
        assert!(
            engine
                .configure(
                    3,
                    ConfigCommand::ArchiveAgents {
                        agent_ids: vec![ids[0].clone(), ids[1].clone()]
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(engine.workspace.config()).unwrap(),
            before
        );
    }
    for selected in [
        vec![],
        vec![ids[0].clone(), ids[0].clone()],
        vec![ids[0].clone(), "missing".into()],
        vec![ids[0].clone(); 5001],
    ] {
        assert!(
            engine
                .configure(
                    3,
                    ConfigCommand::ArchiveAgents {
                        agent_ids: selected
                    }
                )
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(engine.workspace.config()).unwrap(),
            before
        );
    }
    assert!(
        engine
            .configure(
                2,
                ConfigCommand::ArchiveAgents {
                    agent_ids: vec![ids[0].clone()]
                }
            )
            .is_err()
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), sequence);
}

#[test]
fn archiving_all_definitions_keeps_an_empty_draft_and_stale_registry_status_is_not_idle() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, ids) = setup(dir.path(), 5000);
    let mut registry = engine.workspace.config().value.registry.clone();
    for state in [AgentStatus::Waiting, AgentStatus::Stale] {
        registry.agents[1].status = state;
        assert!(registry.archive_many(&ids[..2]).is_err());
        assert_eq!(registry.agents[0].status, AgentStatus::Idle);
    }
    engine
        .configure(3, ConfigCommand::ArchiveAgents { agent_ids: ids })
        .unwrap();
    let config = &engine.workspace.config().value;
    assert_eq!(config.registry.agents.len(), 5000);
    assert!(
        config
            .registry
            .agents
            .iter()
            .all(|agent| agent.status == AgentStatus::Archived)
    );
    assert_eq!(config.architecture.preset, Preset::Network);
    assert_eq!(config.architecture.max_concurrency, 1);
    config.architecture.validate(&config.registry).unwrap();
}
