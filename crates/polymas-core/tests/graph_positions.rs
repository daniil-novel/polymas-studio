use polymas_core::registry::{BatchRequest, RoleDraft};
use polymas_core::workspace::{ConfigCommand, Workspace};
use serde_json::{Value, json};

fn workspace(path: &std::path::Path) -> Workspace {
    let mut ws = Workspace::open(path).unwrap();
    let role = ws
        .configure(
            ws.config().revision,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Graph fixture".into(),
                    instruction: "Test layout".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    ws.configure(
        ws.config().revision,
        ConfigCommand::CreateAgents {
            key: "nodes".into(),
            request: BatchRequest {
                role_id: role["id"].as_str().unwrap().into(),
                role_version: 1,
                count: 2,
                name_pattern: "Node {n}".into(),
            },
        },
    )
    .unwrap();
    ws
}
fn movement(node: &str, position: Value) -> ConfigCommand {
    serde_json::from_value(json!({"type":"set_node_position","nodeId":node,"position":position}))
        .unwrap()
}

#[test]
fn moving_one_node_persists_only_its_position_with_history_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = workspace(dir.path());
    let id = ws.config().value.registry.agents[0].id.clone();
    let before = serde_json::to_value(&ws.config().value).unwrap();
    let revision = ws.config().revision;
    ws.configure(revision, movement(&id, json!([123.5, -45.25])))
        .unwrap();
    let mut expected = before;
    expected["architecture"]["positions"][&id] = json!([123.5, -45.25]);
    assert_eq!(serde_json::to_value(&ws.config().value).unwrap(), expected);
    assert_eq!(ws.config().revision, revision + 1);
    let history = ws.history().unwrap();
    assert!(
        history
            .iter()
            .any(|entry| entry.value.revision == revision + 1
                && entry.value.reason == "graph.position_changed")
    );
    let cursor = ws.db.last_sequence().unwrap();
    assert!(
        ws.configure(revision, movement(&id, json!([9, 9])))
            .is_err()
    );
    assert_eq!(ws.db.last_sequence().unwrap(), cursor);
    drop(ws);
    let mut ws = Workspace::open(dir.path()).unwrap();
    assert_eq!(
        ws.config().value.architecture.positions[&id],
        [123.5, -45.25]
    );
    ws.configure(ws.config().revision, movement(&id, Value::Null))
        .unwrap();
    assert!(!ws.config().value.architecture.positions.contains_key(&id));
}

#[test]
fn invalid_or_archived_nodes_cannot_change_layout_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = workspace(dir.path());
    let id = ws.config().value.registry.agents[0].id.clone();
    for (node, position) in [
        ("missing", json!([1, 2])),
        ("memory:missing", json!([1, 2])),
        (&id, json!([1_000_000, 2])),
        (&id, json!([1, -1_000_000])),
    ] {
        let before = (
            ws.config().revision,
            ws.db.last_sequence().unwrap(),
            ws.history().unwrap().len(),
        );
        assert!(ws.configure(before.0, movement(node, position)).is_err());
        assert_eq!(
            (
                ws.config().revision,
                ws.db.last_sequence().unwrap(),
                ws.history().unwrap().len()
            ),
            before
        );
        assert!(ws.config().value.architecture.positions.is_empty());
    }
    ws.configure(
        ws.config().revision,
        ConfigCommand::ArchiveAgent {
            agent_id: id.clone(),
        },
    )
    .unwrap();
    let revision = ws.config().revision;
    assert!(
        ws.configure(revision, movement(&id, json!([1, 2])))
            .is_err()
    );
    assert_eq!(ws.config().revision, revision);
}

#[test]
fn only_the_enabled_memory_node_can_be_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = workspace(dir.path());
    let connection =
        polymas_core::connections::Connection::files("Layout memory", vec![dir.path().to_owned()]);
    let node = format!("memory:{}", connection.id);
    let connection_id = connection.id.clone();
    ws.save_connection(0, connection).unwrap();
    let mut architecture = ws.config().value.architecture.clone();
    architecture.memory_connection_id = Some(connection_id);
    ws.configure(
        ws.config().revision,
        ConfigCommand::SetArchitecture {
            architecture: architecture.clone(),
        },
    )
    .unwrap();
    assert!(
        ws.configure(ws.config().revision, movement(&node, json!([10, 20])))
            .is_err()
    );
    architecture.memory_enabled = true;
    ws.configure(
        ws.config().revision,
        ConfigCommand::SetArchitecture { architecture },
    )
    .unwrap();
    ws.configure(ws.config().revision, movement(&node, json!([10, 20])))
        .unwrap();
    assert_eq!(
        ws.config().value.architecture.positions[&node],
        [10.0, 20.0]
    );
}
