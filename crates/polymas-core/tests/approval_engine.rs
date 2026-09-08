use polymas_core::{
    connections::{Connection, ConnectionTransport, Health, ToolDescription},
    engine::{Engine, OperationRequest},
    policy::{Approval, ApprovalLedger, Binding, Subject},
    registry::{BatchRequest, RoleDraft},
    scheduler::{Scheduler, TaskInput},
    workspace::ConfigCommand,
};
use serde_json::json;

fn setup() -> (tempfile::TempDir, Engine, OperationRequest) {
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

#[test]
fn reservation_request_and_receipt_commit_together_or_leave_nothing() {
    let (_dir, mut engine, request) = setup();
    let before = serde_json::to_value(&engine.scheduler).unwrap();
    let head = engine.workspace.db.last_sequence().unwrap();
    assert!(
        engine
            .request_approval("request", request.clone(), 100, 0)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&engine.scheduler).unwrap(), before);
    assert!(
        Scheduler::load(&engine.workspace.db)
            .unwrap()
            .task(&request.task_id)
            .unwrap()
            .continuation
            .is_none()
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    assert!(
        engine
            .workspace
            .db
            .list::<Approval>("approvals", 10)
            .unwrap()
            .is_empty()
    );
    assert!(
        ApprovalLedger::for_attempt(&engine.workspace.db, &request.task_id, request.origin_epoch)
            .unwrap()
            .is_none()
    );
    let approval = engine
        .request_approval("request", request.clone(), 100, 30)
        .unwrap();
    let saved = Scheduler::load(&engine.workspace.db).unwrap();
    let task = saved.task(&request.task_id).unwrap();
    assert_eq!(
        task.continuation.as_ref().unwrap().epoch,
        approval.operation.epoch
    );
    assert_eq!(approval.operation.origin_epoch, request.origin_epoch);
    assert_eq!(approval.operation.agent_id, task.assignee);
    assert_eq!(approval.operation.run_id, task.run_id);
    assert_eq!(approval.operation.connection_revision, 1);
    assert_eq!(approval.operation.resource_scope_hash.len(), 64);
    let head = engine.workspace.db.last_sequence().unwrap();
    assert_eq!(
        engine
            .request_approval("request", request.clone(), 110, 60)
            .unwrap()
            .id,
        approval.id
    );
    assert_eq!(engine.workspace.db.last_sequence().unwrap(), head);
    let mut changed = request.clone();
    changed.arguments = json!({"text":"DIFFERENT"});
    assert!(
        engine
            .request_approval("request", changed, 110, 30)
            .is_err()
    );
    assert_eq!(
        engine
            .workspace
            .db
            .list::<Approval>("approvals", 10)
            .unwrap()
            .len(),
        1
    );
    let event_json =
        serde_json::to_string(&engine.workspace.db.events(0, 500, None).unwrap()).unwrap();
    assert!(!event_json.contains("APPROVAL-FIXTURE-73"));
}

#[test]
fn request_checks_live_and_pinned_access_and_never_accepts_stale_attempts() {
    let (_dir, mut engine, request) = setup();
    let mut invalid = request.clone();
    invalid.origin_epoch += 1;
    assert!(engine.request_approval("stale", invalid, 100, 30).is_err());
    let mut invalid = request.clone();
    invalid.namespace = "other".into();
    assert!(engine.request_approval("scope", invalid, 100, 30).is_err());
    let mut newly_granted = engine.workspace.config().value.bindings.clone();
    newly_granted[0].namespace = "new-scope".into();
    engine
        .configure(
            4,
            ConfigCommand::SetBindings {
                bindings: newly_granted,
            },
        )
        .unwrap();
    let mut invalid = request.clone();
    invalid.namespace = "new-scope".into();
    assert!(
        engine
            .request_approval("new-scope", invalid, 100, 30)
            .is_err()
    );
    engine
        .configure(5, ConfigCommand::SetBindings { bindings: vec![] })
        .unwrap();
    assert!(
        engine
            .request_approval("revoked", request.clone(), 100, 30)
            .is_err()
    );
    assert!(
        engine
            .scheduler
            .task(&request.task_id)
            .unwrap()
            .continuation
            .is_none()
    );
    assert!(
        engine
            .workspace
            .db
            .list::<Approval>("approvals", 10)
            .unwrap()
            .is_empty()
    );
}
