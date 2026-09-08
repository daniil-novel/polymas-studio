use polymas_core::{
    connections::{Connection, Health, ToolDescription},
    engine::Engine,
    policy::{Binding, Subject},
    registry::{BatchRequest, RoleDraft, RoleVersion, Sandbox},
    scheduler::TaskInput,
    topology::Target,
    workspace::ConfigCommand,
};
use serde_json::{Value, json};

fn request(key: &str, id: &str, version: u32, name: &str) -> ConfigCommand {
    serde_json::from_value(
        json!({"type":"duplicate_role","key":key,"roleId":id,"sourceVersion":version,"name":name}),
    )
    .unwrap()
}

fn seeded(path: &std::path::Path) -> (Engine, String) {
    let mut engine = Engine::open(path).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Исходная роль".into(),
                    description: "Первая версия".into(),
                    instruction: "Проверь источники 🧠".into(),
                    output_contract: "JSON: sources".into(),
                    acceptance_criteria: "Две ссылки".into(),
                    escalation_rule: "Спроси владельца".into(),
                    model: Some("fixture-model".into()),
                    sandbox: Sandbox::WorkspaceWrite,
                    timeout_seconds: 123,
                    max_turns: 7,
                    max_output_tokens: Some(2048),
                },
            },
        )
        .unwrap();
    let id = role["id"].as_str().unwrap().to_owned();
    engine
        .configure(
            2,
            ConfigCommand::ReviseRole {
                role_id: id.clone(),
                expected_version: 1,
                draft: RoleDraft {
                    name: "Latest".into(),
                    instruction: "Version two".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::CreateAgents {
                key: "duplicate-fixture".into(),
                request: BatchRequest {
                    role_id: id.clone(),
                    role_version: 2,
                    count: 2,
                    name_pattern: "Agent {n}".into(),
                },
            },
        )
        .unwrap();
    // Authored capability fixture: no probe or external call is performed.
    let mut connection = Connection::files("Fixture knowledge", vec![path.to_path_buf()]);
    connection.enabled = true;
    connection.health = Health::Ready;
    connection.tool_allowlist = vec!["read_text".into()];
    connection.tools.push(ToolDescription {
        name: "read_text".into(),
        description: "Read".into(),
        input_schema: json!({"type":"object"}),
        read_only: true,
    });
    engine
        .workspace
        .db
        .transaction(|tx| tx.put("connections", &connection.id, 0, &connection))
        .unwrap();
    engine
        .configure(
            4,
            ConfigCommand::SetBindings {
                bindings: vec![Binding {
                    id: "source-binding".into(),
                    connection_id: connection.id,
                    subject: Subject::Role { id: id.clone() },
                    tools: ["read_text".into()].into(),
                    allow_write: false,
                    required: true,
                    namespace: "fixture".into(),
                }],
            },
        )
        .unwrap();
    (engine, id)
}

#[test]
fn a_duplicate_has_a_new_identity_and_preserves_source_bindings_instances_and_pinned_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, id) = seeded(dir.path());
    let before = engine.workspace.config().value.clone();
    let ids = before
        .registry
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    let run = engine
        .submit(
            "old-tasks",
            TaskInput {
                prompt: "Fixture".into(),
                target: Target::Agents { ids },
                ..Default::default()
            },
            1,
        )
        .unwrap();
    engine.claim(1, "fixture-worker", 100).unwrap();
    let tasks = run
        .task_ids
        .iter()
        .map(|id| engine.scheduler.task(id).unwrap().clone())
        .collect::<Vec<_>>();
    let head = engine.workspace.db.last_sequence().unwrap();
    let copied = engine
        .configure(5, request("copy-a", &id, 1, "Новая роль 🧠"))
        .unwrap();
    let role: RoleVersion = serde_json::from_value(copied["role"].clone()).unwrap();
    assert_ne!(role.id, id);
    assert_eq!(role.version, 1);
    let mut expected_draft = before.registry.roles[0].draft.clone();
    expected_draft.name = "Новая роль 🧠".into();
    assert_eq!(role.draft, expected_draft);
    assert_eq!(copied["sourceRoleId"], id);
    assert_eq!(copied["sourceVersion"], 1);
    let mut expected = serde_json::to_value(&before).unwrap();
    expected["registry"]["roles"]
        .as_array_mut()
        .unwrap()
        .push(copied["role"].clone());
    assert_eq!(
        serde_json::to_value(&engine.workspace.config().value).unwrap(),
        expected
    );
    for task in &tasks {
        assert_eq!(engine.scheduler.task(&task.id).unwrap(), task);
    }
    assert_eq!(engine.workspace.config().revision, 6);
    let events = engine.workspace.db.events(head, 10, None).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].input.kind, "role.duplicated");
    assert_eq!(events[0].input.body["result"], copied);
    assert!(
        engine
            .workspace
            .history()
            .unwrap()
            .iter()
            .any(|h| h.value.revision == 6 && h.value.reason == "role.duplicated")
    );
    assert_eq!(
        engine
            .configure(5, request("copy-a", &id, 1, "Новая роль 🧠"))
            .unwrap(),
        copied
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head + 1);
    drop(engine);
    let mut reopened = Engine::open(dir.path()).unwrap();
    let reopened_head = reopened.workspace.db.last_sequence().unwrap();
    assert_eq!(
        reopened
            .configure(5, request("copy-a", &id, 1, "Новая роль 🧠"))
            .unwrap(),
        copied
    );
    assert_eq!(reopened.workspace.config().revision, 6);
    assert_eq!(
        reopened.workspace.db.last_sequence().unwrap(),
        reopened_head
    );
    assert_eq!(
        reopened
            .workspace
            .config()
            .value
            .registry
            .role(&role.id, 1)
            .unwrap(),
        &role
    );
    assert_eq!(
        serde_json::to_value(&reopened.workspace.config().value).unwrap(),
        expected
    );
}

#[test]
fn stale_unknown_invalid_or_reused_copy_requests_do_not_mutate_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, id) = seeded(dir.path());
    let copied = engine
        .configure(5, request("copy-a", &id, 1, "Copy"))
        .unwrap();
    let before = serde_json::to_value(engine.workspace.config()).unwrap();
    let head = engine.workspace.db.last_sequence().unwrap();
    for (rev, key, source, version, name) in [
        (5, "new", id.as_str(), 1, "Copy"),
        (6, "new", "missing", 1, "Copy"),
        (6, "new", id.as_str(), 0, "Copy"),
        (6, "new", id.as_str(), 3, "Copy"),
        (6, "new", id.as_str(), 1, "  "),
        (6, "", id.as_str(), 1, "Copy"),
        (6, "copy-a", id.as_str(), 2, "Copy"),
        (6, "copy-a", id.as_str(), 1, "Different"),
        (
            6,
            "copy-a",
            copied["role"]["id"].as_str().unwrap(),
            1,
            "Copy",
        ),
    ] {
        assert!(
            engine
                .configure(rev, request(key, source, version, name))
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(engine.workspace.config()).unwrap(),
            before
        );
        assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    }
    assert!(
        engine
            .configure(6, request(&"x".repeat(129), &id, 1, "Copy"))
            .is_err()
    );
    assert!(
        engine
            .configure(6, request("long-name", &id, 1, &"я".repeat(101)))
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(engine.workspace.config()).unwrap(),
        before
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    let injected = json!({"type":"duplicate_role","key":"new","roleId":id,"sourceVersion":1,"name":"Copy","draft":{"instruction":"Injected"}});
    assert!(serde_json::from_value::<ConfigCommand>(injected).is_err());
    // A new key deliberately permits a second independent copy with the same display name.
    let another = engine
        .configure(6, request("copy-b", &id, 1, "Copy"))
        .unwrap();
    assert_ne!(another["role"]["id"], copied["role"]["id"]);
}

#[test]
fn history_failure_rolls_back_the_new_role_and_durable_receipt_together() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, id) = seeded(dir.path());
    let before = serde_json::to_value(engine.workspace.config()).unwrap();
    let head = engine.workspace.db.last_sequence().unwrap();
    // Force the history CAS to fail after the configuration write inside the transaction.
    engine
        .workspace
        .db
        .transaction(|tx| {
            tx.put(
                "configuration_history",
                "6",
                0,
                &json!({"fixture":"conflict"}),
            )
        })
        .unwrap();
    assert!(
        engine
            .configure(5, request("rolled-back", &id, 1, "Copy"))
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(engine.workspace.config()).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(
            engine
                .workspace
                .db
                .get::<Value>("workspace", "configuration")
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        before
    );
    assert!(
        engine
            .workspace
            .db
            .get::<Value>("role_duplication_receipts", "rolled-back")
            .unwrap()
            .is_none()
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
}
