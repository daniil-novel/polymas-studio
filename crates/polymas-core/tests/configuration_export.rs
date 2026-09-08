use polymas_core::{
    configuration_export::{MAX_EXPORT_BYTES, configuration_export},
    connections::{Connection, ConnectionTransport},
    registry::{AgentStatus, BatchRequest, RoleDraft},
    storage::Stored,
    workspace::{ConfigCommand, Snapshot, Workspace},
};
use serde_json::{Value, json};

fn snapshot() -> (tempfile::TempDir, Workspace, Snapshot) {
    let dir = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(dir.path()).unwrap();
    let role = workspace
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Исследователь".into(),
                    instruction: "Сравни источники и сохрани ссылки.".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    workspace
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "PRIVATE-BATCH-RECEIPT".into(),
                request: BatchRequest {
                    role_id: role["id"].as_str().unwrap().into(),
                    role_version: 1,
                    count: 2,
                    name_pattern: "Агент {n}".into(),
                },
            },
        )
        .unwrap();
    workspace
        .configure(
            3,
            ConfigCommand::ReviseRole {
                role_id: role["id"].as_str().unwrap().into(),
                expected_version: 1,
                draft: RoleDraft {
                    name: "Исследователь".into(),
                    instruction: "Новая инструкция".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let mut source = workspace.snapshot().unwrap();
    source.config.value.registry.agents[0].session_id = Some("PRIVATE-SESSION".into());
    source.config.value.registry.agents[0].workspace_key = "PRIVATE-FOLDER".into();
    source.config.value.registry.agents[0].forked_from = Some("PRIVATE-LINEAGE".into());
    source.config.value.registry.agents[0].status = AgentStatus::Running;
    source.config.value.registry.agents[1].status = AgentStatus::Archived;
    let mut files = Connection::files("Общая память", vec![dir.path().join("PRIVATE-ROOT")]);
    files.enabled = true;
    files.auth_secret_ref = Some("secret_PRIVATE-AUTH".into());
    files
        .environment
        .insert("CONFIG".into(), "PRIVATE-ENVIRONMENT".into());
    files
        .secret_environment
        .insert("TOKEN".into(), "secret_PRIVATE-ENVIRONMENT".into());
    files.last_error = Some("PRIVATE-DIAGNOSTIC".into());
    files.tool_allowlist = vec!["read_text".into()];
    files.tools = vec![polymas_core::connections::ToolDescription {
        name: "read_text".into(),
        description: "PRIVATE-DESCRIPTOR".into(),
        input_schema: json!({"PRIVATE-SCHEMA":true}),
        read_only: true,
    }];
    source.config.value.architecture.memory_enabled = true;
    source.config.value.architecture.memory_connection_id = Some(files.id.clone());
    let first = source.config.value.registry.agents[0].id.clone();
    source
        .config
        .value
        .architecture
        .positions
        .insert(first.clone(), [1.25, -20.0]);
    source
        .config
        .value
        .architecture
        .holons
        .push(polymas_core::topology::Holon {
            id: "holon_export".into(),
            name: "Исследование".into(),
            goal: "Проверить источники".into(),
            members: vec![first.clone()],
            coordinator_agent_id: Some(first.clone()),
            ..Default::default()
        });
    source
        .config
        .value
        .bindings
        .push(polymas_core::policy::Binding {
            id: "binding_export".into(),
            connection_id: files.id.clone(),
            subject: polymas_core::policy::Subject::Agent { id: first },
            tools: ["read_text".into()].into(),
            allow_write: false,
            required: true,
            namespace: "research/sources".into(),
        });
    let mut http = files.clone();
    http.id = "connection_http".into();
    http.transport = ConnectionTransport::Http {
        url: "https://example.invalid/PRIVATE-ENDPOINT".into(),
    };
    let mut stdio = files.clone();
    stdio.id = "connection_stdio".into();
    stdio.transport = ConnectionTransport::Stdio {
        command: dir.path().join("PRIVATE-EXECUTABLE"),
        args: vec!["PRIVATE-ARGUMENT".into()],
    };
    source.connections = vec![files, http, stdio]
        .into_iter()
        .map(|value| Stored { revision: 7, value })
        .collect();
    (dir, workspace, source)
}

#[test]
fn explicit_export_preserves_definitions_but_excludes_runtime_and_local_connection_data() {
    let (_dir, workspace, source) = snapshot();
    let before = serde_json::to_value(&source).unwrap();
    let head = workspace.db.last_sequence().unwrap();
    let bytes = configuration_export(&source).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("PRIVATE-"));
    assert!(!text.contains(&source.config.value.id));
    let document: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(document["format"], "polymas-configuration");
    assert_eq!(document["schemaVersion"], "0.1.0");
    assert_eq!(document["migrationVersion"], 1);
    assert_eq!(document["sourceRevision"], 4);
    assert_eq!(
        document["runtime"],
        json!({"adapter":"codex-exec", "setupRequired":true})
    );
    assert_eq!(document["roleVersions"].as_array().unwrap().len(), 2);
    assert_eq!(
        document["roleVersions"][0]["instruction"],
        "Сравни источники и сохрани ссылки."
    );
    for (index, agent) in source.config.value.registry.agents.iter().enumerate() {
        assert_eq!(
            document["instances"][index],
            json!({"id":agent.id,"name":agent.name,"roleId":agent.role_id,"roleVersion":1,"archived":index == 1})
        );
    }
    let mut architecture = serde_json::to_value(&source.config.value.architecture).unwrap();
    architecture["paused"] = json!(true);
    assert_eq!(document["architecture"], architecture);
    assert_eq!(
        document["connections"][0]["setupFields"],
        json!([
            "roots",
            "authentication",
            "environment",
            "secret_environment"
        ])
    );
    assert_eq!(document["connections"][0]["rootCount"], 1);
    assert_eq!(document["connections"][1]["setupFields"][0], "url");
    assert_eq!(document["connections"][2]["setupFields"][0], "command");
    assert_eq!(document["connections"][2]["setupFields"][1], "args");
    for connection in document["connections"].as_array().unwrap() {
        assert_eq!(connection["setupRequired"], true);
        assert_eq!(connection["desiredEnabled"], true);
    }
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(workspace.db.last_sequence().unwrap(), head);
    assert_eq!(workspace.config().revision, 4);
    assert_eq!(configuration_export(&source).unwrap(), text.as_bytes());
    let mut normalized = text.clone();
    for (old, new) in [
        (
            source.config.value.registry.roles[0].id.as_str(),
            "role_export",
        ),
        (
            source.config.value.registry.agents[0].id.as_str(),
            "agent_export_1",
        ),
        (
            source.config.value.registry.agents[1].id.as_str(),
            "agent_export_2",
        ),
        (source.connections[0].value.id.as_str(), "connection_files"),
    ] {
        normalized = normalized.replace(old, new);
    }
    let actual: Value = serde_json::from_str(&normalized).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../contracts/export-examples/materialized.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn schema_fixture_is_the_exact_empty_workspace_projection() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(dir.path()).unwrap();
    let bytes = configuration_export(&workspace.snapshot().unwrap()).unwrap();
    let actual: Value = serde_json::from_slice(&bytes).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../contracts/export-examples/empty.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn oversized_exports_fail_instead_of_producing_a_truncated_file() {
    let (_dir, _workspace, mut source) = snapshot();
    source.config.value.registry.roles[0].draft.output_contract = "x".repeat(MAX_EXPORT_BYTES);
    let error = configuration_export(&source).unwrap_err();
    assert!(error.to_string().contains("16 МиБ"));
}

#[test]
fn five_thousand_instance_definitions_keep_distinct_references_in_one_file() {
    let (_dir, _workspace, mut source) = snapshot();
    let original = source.config.value.registry.agents[0].clone();
    source.config.value.registry.agents = (0..5000)
        .map(|index| {
            let mut agent = original.clone();
            agent.id = format!("agent_export_{index:04}");
            agent.name = format!("Исследователь {index:04}");
            agent
        })
        .collect();
    source.config.value.architecture = polymas_core::topology::Architecture {
        max_concurrency: 5000,
        ..Default::default()
    };
    source.config.value.bindings.clear();
    let bytes = configuration_export(&source).unwrap();
    assert!(bytes.len() < MAX_EXPORT_BYTES);
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    let agents = document["instances"].as_array().unwrap();
    assert_eq!(agents.len(), 5000);
    assert_eq!(
        agents
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        5000
    );
    assert_eq!(agents[4999]["id"], "agent_export_4999");
    assert_eq!(document["architecture"]["maxConcurrency"], 5000);
}
