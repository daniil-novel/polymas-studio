use polymas_core::{
    connections::{Connection, ConnectionTransport, Health, ToolDescription},
    engine::{Engine, OperationRequest},
    policy::{Approval, ApprovalLedger, Binding, Subject},
    registry::{BatchRequest, RoleDraft},
    scheduler::{Scheduler, TaskInput},
    workspace::ConfigCommand,
};
use serde_json::json;

#[test]
fn an_abandoned_external_execution_requires_owner_reconciliation_before_retry_or_acceptance() {
    use polymas_core::policy::ApprovalState;
    let (dir, mut engine, request) = setup();
    let approval = engine
        .request_approval("request", request.clone(), 100, 30)
        .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Wait".into())
        .unwrap();
    engine
        .decide_approval(&approval.id, 1, &approval.fingerprint, true, 101)
        .unwrap();
    engine.wake_approvals(101).unwrap();
    let epoch = engine.claim(1, "local", 102).unwrap()[0]
        .lease
        .as_ref()
        .unwrap()
        .epoch;
    engine
        .consume_approval(&request.task_id, epoch, &approval.id, 102)
        .unwrap();
    drop(engine);
    let mut engine = Engine::open(dir.path()).unwrap();
    let unknown = engine
        .workspace
        .db
        .get::<Approval>("approvals", &approval.id)
        .unwrap()
        .unwrap();
    assert_eq!(unknown.value.state, ApprovalState::Unknown);
    assert!(
        engine
            .consume_approval(&request.task_id, epoch, &approval.id, 103)
            .is_err()
    );
    assert!(
        engine
            .reconcile(&request.task_id, true, "Please retry the task")
            .is_err()
    );
    assert!(
        ApprovalLedger::reconcile_unknown(
            &mut engine.workspace.db,
            &approval.id,
            unknown.revision,
            ApprovalState::Approved,
            "Not a valid outcome"
        )
        .is_err()
    );
    assert!(
        ApprovalLedger::reconcile_unknown(
            &mut engine.workspace.db,
            &approval.id,
            unknown.revision,
            ApprovalState::Failed,
            ""
        )
        .is_err()
    );
    ApprovalLedger::reconcile_unknown(
        &mut engine.workspace.db,
        &approval.id,
        unknown.revision,
        ApprovalState::Failed,
        "Fixture server confirms no write occurred",
    )
    .unwrap();
    engine
        .reconcile(&request.task_id, true, "External outcome verified")
        .unwrap();
    let next = engine.claim(1, "local", 104).unwrap().remove(0);
    assert!(next.lease.as_ref().unwrap().epoch > epoch);
    engine
        .finish(
            &request.task_id,
            next.lease.unwrap().epoch,
            Ok("Verified result".into()),
        )
        .unwrap();
    engine.accept(&request.task_id).unwrap();
}

#[test]
fn an_unresolved_call_blocks_acceptance_even_after_a_model_reports_success() {
    use polymas_core::policy::ApprovalState;
    let (_dir, mut engine, request) = setup();
    let approval = engine
        .request_approval("request", request.clone(), 100, 30)
        .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Wait".into())
        .unwrap();
    engine
        .decide_approval(&approval.id, 1, &approval.fingerprint, true, 101)
        .unwrap();
    engine.wake_approvals(101).unwrap();
    let epoch = engine.claim(1, "local", 102).unwrap()[0]
        .lease
        .as_ref()
        .unwrap()
        .epoch;
    engine
        .consume_approval(&request.task_id, epoch, &approval.id, 102)
        .unwrap();
    assert!(engine.recover_inactive_operations().unwrap().is_empty());
    engine
        .finish(&request.task_id, epoch, Ok("Model claims success".into()))
        .unwrap();
    assert!(engine.accept(&request.task_id).is_err());
    assert_eq!(
        engine.recover_inactive_operations().unwrap(),
        vec![approval.id.clone()]
    );
    assert!(engine.accept(&request.task_id).is_err());
    let stored = engine
        .workspace
        .db
        .get::<Approval>("approvals", &approval.id)
        .unwrap()
        .unwrap();
    ApprovalLedger::reconcile_unknown(
        &mut engine.workspace.db,
        &approval.id,
        stored.revision,
        ApprovalState::Succeeded,
        "Fixture server confirms the expected document hash",
    )
    .unwrap();
    engine.accept(&request.task_id).unwrap();
}

#[test]
fn owner_decision_resumes_only_the_reserved_turn_and_execution_consumes_once() {
    let (dir, mut engine, request) = setup();
    let approval = engine
        .request_approval("request", request.clone(), 100, 30)
        .unwrap();
    let revision = engine
        .workspace
        .db
        .get::<Approval>("approvals", &approval.id)
        .unwrap()
        .unwrap()
        .revision;
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Await owner".into())
        .unwrap();
    drop(engine);
    let mut engine = Engine::open(dir.path()).unwrap();
    assert!(engine.wake_approvals(101).unwrap().is_empty());
    assert!(
        engine
            .decide_approval(&approval.id, revision + 1, &approval.fingerprint, true, 102)
            .is_err()
    );
    engine
        .decide_approval(&approval.id, revision, &approval.fingerprint, true, 102)
        .unwrap();
    engine
        .decide_approval(&approval.id, revision, &approval.fingerprint, true, 103)
        .unwrap();
    assert!(
        engine
            .decide_approval(&approval.id, i64::MAX, &approval.fingerprint, true, 103)
            .is_err()
    );
    assert!(
        engine
            .decide_approval(&approval.id, revision, &approval.fingerprint, false, 103)
            .is_err()
    );
    assert_eq!(
        engine.wake_approvals(103).unwrap(),
        vec![request.task_id.clone()]
    );
    assert!(engine.wake_approvals(103).unwrap().is_empty());
    assert!(
        engine
            .consume_approval(&request.task_id, request.origin_epoch, &approval.id, 103)
            .is_err()
    );
    let resumed = engine.claim(1, "local", 104).unwrap().remove(0);
    let epoch = resumed.lease.unwrap().epoch;
    assert_eq!(epoch, approval.operation.epoch);
    let lookup = ApprovalLedger::for_execution(&engine.workspace.db, &request.task_id, epoch)
        .unwrap()
        .unwrap();
    assert_eq!(lookup.id, approval.id);
    assert!(
        engine
            .consume_approval("other-task", epoch, &approval.id, 104)
            .is_err()
    );
    engine
        .consume_approval(&request.task_id, epoch, &approval.id, 104)
        .unwrap();
    assert!(
        engine
            .consume_approval(&request.task_id, epoch, &approval.id, 105)
            .is_err()
    );
}

#[test]
fn expiry_revocation_and_cancellation_cannot_grant_an_external_write() {
    let (_dir, mut engine, request) = setup();
    let approval = engine
        .request_approval("request", request.clone(), 100, 2)
        .unwrap();
    engine
        .park_approval(&request.task_id, request.origin_epoch, "Wait".into())
        .unwrap();
    assert_eq!(
        engine.wake_approvals(102).unwrap(),
        vec![request.task_id.clone()]
    );
    assert_eq!(
        engine
            .workspace
            .db
            .get::<Approval>("approvals", &approval.id)
            .unwrap()
            .unwrap()
            .value
            .state,
        polymas_core::policy::ApprovalState::Expired
    );
    let resumed = engine.claim(1, "local", 103).unwrap().remove(0);
    assert!(
        engine
            .consume_approval(
                &request.task_id,
                resumed.lease.unwrap().epoch,
                &approval.id,
                103
            )
            .is_err()
    );

    let (_dir2, mut e2, r2) = setup();
    let a2 = e2.request_approval("request", r2.clone(), 100, 30).unwrap();
    e2.park_approval(&r2.task_id, r2.origin_epoch, "Wait".into())
        .unwrap();
    e2.decide_approval(&a2.id, 1, &a2.fingerprint, true, 101)
        .unwrap();
    e2.wake_approvals(101).unwrap();
    let t2 = e2.claim(1, "local", 101).unwrap().remove(0);
    e2.configure(4, ConfigCommand::SetBindings { bindings: vec![] })
        .unwrap();
    assert!(
        e2.consume_approval(&r2.task_id, t2.lease.unwrap().epoch, &a2.id, 102)
            .is_err()
    );

    let (_dir3, mut e3, r3) = setup();
    let a3 = e3.request_approval("request", r3.clone(), 100, 30).unwrap();
    e3.park_approval(&r3.task_id, r3.origin_epoch, "Wait".into())
        .unwrap();
    e3.cancel_task(&r3.task_id).unwrap();
    assert!(
        e3.decide_approval(&a3.id, 1, &a3.fingerprint, true, 101)
            .is_err()
    );
    assert!(e3.wake_approvals(101).unwrap().is_empty());
}

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
