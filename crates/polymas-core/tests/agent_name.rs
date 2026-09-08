use polymas_core::registry::{BatchRequest, RoleDraft};
use polymas_core::workspace::{ConfigCommand, Workspace};
use serde_json::json;

#[test]
fn renaming_keeps_identity_directories_and_history_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let role = ws
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Role".into(),
                    instruction: "Read sources".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    ws.configure(
        2,
        ConfigCommand::CreateAgents {
            key: "one".into(),
            request: BatchRequest {
                role_id: role["id"].as_str().unwrap().into(),
                role_version: 1,
                count: 2,
                name_pattern: "Agent {n}".into(),
            },
        },
    )
    .unwrap();
    let before = ws.config().value.registry.agents[0].clone();
    let other = serde_json::to_value(&ws.config().value.registry.agents[1]).unwrap();
    let path = ws.agent_workspace(&before.id).unwrap();
    std::fs::write(path.join("authored-artifact.txt"), "keep these bytes").unwrap();
    let sequence = ws.db.last_sequence().unwrap();
    let rename = |name: &str| ConfigCommand::RenameAgent {
        agent_id: before.id.clone(),
        name: name.into(),
    };
    assert!(ws.configure(2, rename("stale")).is_err());
    assert!(ws.configure(3, rename(&"Я".repeat(101))).is_err());
    assert!(ws.configure(3, rename("  ")).is_err());
    assert_eq!(ws.db.last_sequence().unwrap(), sequence);
    ws.configure(3, rename("Проверяющий источники")).unwrap();
    assert_eq!(ws.db.last_sequence().unwrap(), sequence + 1);
    drop(ws);
    let ws = Workspace::open(dir.path()).unwrap();
    let mut expected = serde_json::to_value(&before).unwrap();
    expected["name"] = json!("Проверяющий источники");
    assert_eq!(
        serde_json::to_value(&ws.config().value.registry.agents[0]).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_value(&ws.config().value.registry.agents[1]).unwrap(),
        other
    );
    assert_eq!(ws.agent_workspace(&before.id).unwrap(), path);
    assert_eq!(
        std::fs::read_to_string(path.join("authored-artifact.txt")).unwrap(),
        "keep these bytes"
    );
    let history = ws.history().unwrap();
    let prior = history
        .iter()
        .find(|entry| entry.value.revision == 3)
        .unwrap();
    assert_eq!(
        prior.value.configuration.registry.agents[0].name,
        before.name
    );
    assert_eq!(
        history
            .iter()
            .find(|entry| entry.value.revision == 4)
            .unwrap()
            .value
            .reason,
        "agent.renamed"
    );
}
