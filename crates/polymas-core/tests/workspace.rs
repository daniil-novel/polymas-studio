use polymas_core::registry::{BatchRequest, RoleDraft};
use polymas_core::workspace::{ConfigCommand, Workspace};
use serde_json::json;
#[test]
fn configuration_commands_persist_with_revision_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let first = ws.config().revision;
    let result = ws
        .configure(
            first,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Research".into(),
                    instruction: "Investigate with evidence".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let role_id = result["id"].as_str().unwrap().to_owned();
    ws.configure(
        first + 1,
        ConfigCommand::CreateAgents {
            key: "batch".into(),
            request: BatchRequest {
                role_id,
                role_version: 1,
                count: 10,
                name_pattern: "Agent {n}".into(),
            },
        },
    )
    .unwrap();
    assert!(
        ws.configure(
            first,
            ConfigCommand::RenameWorkspace {
                name: "stale".into()
            }
        )
        .is_err()
    );
    drop(ws);
    let ws = Workspace::open(dir.path()).unwrap();
    assert_eq!(ws.config().value.registry.agents.len(), 10);
    assert_eq!(ws.history().unwrap().len(), 3);
}
#[test]
fn invalid_configuration_does_not_mutate_live_state_or_emit_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let before = ws.db.last_sequence().unwrap();
    assert!(
        ws.configure(
            ws.config().revision,
            ConfigCommand::RenameWorkspace { name: " ".into() }
        )
        .is_err()
    );
    assert_eq!(ws.db.last_sequence().unwrap(), before);
    assert_ne!(ws.config().value.name, " ");
}
#[test]
fn connection_drafts_cannot_forge_probe_results_or_enable_themselves() {
    let dir = tempfile::tempdir().unwrap();
    let mut ws = Workspace::open(dir.path()).unwrap();
    let mut c = polymas_core::connections::Connection::files("files", vec![dir.path().to_owned()]);
    c.enabled = true;
    c.health = polymas_core::connections::Health::Ready;
    c.tools = polymas_core::connections::files_tools();
    let saved = ws.save_connection(0, c).unwrap();
    assert!(!saved.value.enabled);
    assert!(saved.value.tools.is_empty());
    let data = serde_json::to_value(ws.snapshot().unwrap()).unwrap();
    assert_eq!(
        data["connections"][0]["value"]["health"],
        json!("unconfigured")
    );
}
