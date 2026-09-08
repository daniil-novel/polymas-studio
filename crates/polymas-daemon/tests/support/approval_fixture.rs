use polymas_core::{
    connections::{Connection, ConnectionTransport, Health, ToolDescription},
    engine::{Engine, OperationRequest},
    policy::{Binding, Subject},
    registry::{BatchRequest, RoleDraft},
    scheduler::TaskInput,
    workspace::ConfigCommand,
};
use serde_json::json;
pub fn setup() -> (tempfile::TempDir, Engine, OperationRequest) {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let role = engine
        .configure(
            1,
            ConfigCommand::CreateRole {
                draft: RoleDraft {
                    name: "Writer".into(),
                    instruction: "Write only approved fixture data".into(),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let role_id = role["id"].as_str().unwrap().to_owned();
    engine
        .configure(
            2,
            ConfigCommand::CreateAgents {
                key: "agent".into(),
                request: BatchRequest {
                    role_id: role_id.clone(),
                    role_version: 1,
                    count: 1,
                    name_pattern: "Writer {n}".into(),
                },
            },
        )
        .unwrap();
    let mut connection = Connection::files("Fixture", vec![]);
    connection.transport = ConnectionTransport::Http {
        url: "https://fixture.invalid/mcp".into(),
    };
    connection.read_only = false;
    connection.enabled = true;
    connection.health = Health::Ready;
    connection.tools = vec![ToolDescription {
        name: "write".into(),
        description: "Write fixture".into(),
        input_schema: json!({"type":"object"}),
        read_only: false,
    }];
    connection.tool_allowlist = vec!["write".into()];
    connection.validate().unwrap();
    let connection_id = connection.id.clone();
    engine
        .workspace
        .db
        .transaction(|tx| tx.put("connections", &connection.id, 0, &connection))
        .unwrap();
    engine
        .configure(
            3,
            ConfigCommand::SetBindings {
                bindings: vec![Binding {
                    id: "write-binding".into(),
                    connection_id: connection_id.clone(),
                    subject: Subject::Role { id: role_id },
                    tools: ["write".into()].into(),
                    allow_write: true,
                    required: false,
                    namespace: "fixture".into(),
                }],
            },
        )
        .unwrap();
    engine
        .submit(
            "task",
            TaskInput {
                prompt: "Update fixture".into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
    let task = engine.claim(1, "local", 100).unwrap().remove(0);
    let request = OperationRequest {
        task_id: task.id,
        origin_epoch: task.lease.unwrap().epoch,
        connection_id,
        namespace: "fixture".into(),
        tool: "write".into(),
        arguments: json!({"text":"APPROVAL-FIXTURE-73"}),
    };
    (dir, engine, request)
}
