use polymas_core::policy::{ApprovalLedger, ApprovalState, Operation};
use polymas_core::storage::Store;
use serde_json::json;

fn operation() -> Operation {
    Operation {
        agent_id: "agent-a".into(),
        task_id: "task-a".into(),
        run_id: "run-a".into(),
        namespace: "reports/team".into(),
        resource_scope_hash: "a".repeat(64),
        epoch: 7,
        connection_id: "db".into(),
        connection_revision: 2,
        tool: "write".into(),
        arguments: json!({"path":"report.txt","text":"approved text"}),
    }
}
#[test]
fn approval_binds_run_namespace_and_resource_scope_and_legacy_unscoped_records_fail_closed() {
    let mut db = Store::memory().unwrap();
    let approval = ApprovalLedger::request(&mut db, operation(), 100, 300).unwrap();
    ApprovalLedger::decide(&mut db, &approval.id, true, 101).unwrap();
    let mut other_run = operation();
    other_run.run_id = "run-b".into();
    let mut other_namespace = operation();
    other_namespace.namespace = "reports/private".into();
    let mut other_scope = operation();
    other_scope.resource_scope_hash = "b".repeat(64);
    for changed in [other_run, other_namespace, other_scope] {
        assert!(ApprovalLedger::consume(&mut db, &approval.id, &changed, 102).is_err());
    }
    let mut legacy = serde_json::to_value(&approval).unwrap();
    legacy["id"] = json!("legacy");
    legacy["state"] = json!("pending");
    let op = legacy["operation"].as_object_mut().unwrap();
    for key in ["runId", "namespace", "resourceScopeHash"] {
        op.remove(key);
    }
    db.transaction(|tx| {
        tx.put("approvals", "legacy", 0, &legacy)?;
        Ok(())
    })
    .unwrap();
    assert!(
        db.get::<polymas_core::policy::Approval>("approvals", "legacy")
            .unwrap()
            .is_some()
    );
    assert!(ApprovalLedger::decide(&mut db, "legacy", true, 102).is_err());
    assert!(ApprovalLedger::consume(&mut db, "legacy", &operation(), 102).is_err());
    for value in ["", "with newline\n", "scope with spaces"] {
        let mut invalid = operation();
        invalid.namespace = value.into();
        assert!(invalid.fingerprint().is_err());
    }
    let mut invalid = operation();
    invalid.resource_scope_hash = "not-a-digest".into();
    assert!(invalid.fingerprint().is_err());
    ApprovalLedger::consume(&mut db, &approval.id, &operation(), 102).unwrap();
    let events = db.events(0, 50, Some("run-a")).unwrap();
    assert_eq!(events.len(), 3);
}
#[test]
fn approval_is_exact_single_use_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let mut db = Store::open(&path).unwrap();
    let approval = ApprovalLedger::request(&mut db, operation(), 100, 300).unwrap();
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &operation(), 101).is_err());
    ApprovalLedger::decide(&mut db, &approval.id, true, 101).unwrap();
    drop(db);
    let mut db = Store::open(&path).unwrap();
    ApprovalLedger::consume(&mut db, &approval.id, &operation(), 102).unwrap();
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &operation(), 103).is_err());
}
#[test]
fn changed_arguments_epoch_and_expiry_require_new_approval() {
    let mut db = Store::memory().unwrap();
    let approval = ApprovalLedger::request(&mut db, operation(), 100, 10).unwrap();
    ApprovalLedger::decide(&mut db, &approval.id, true, 101).unwrap();
    let mut changed = operation();
    changed.arguments["text"] = json!("different text");
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &changed, 102).is_err());
    let mut changed = operation();
    changed.epoch += 1;
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &changed, 102).is_err());
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &operation(), 111).is_err());
}
#[test]
fn rejection_never_executes_and_object_key_order_does_not_change_semantics() {
    let mut db = Store::memory().unwrap();
    let first = operation();
    let mut reordered = operation();
    reordered.arguments =
        serde_json::from_str("{\"text\":\"approved text\",\"path\":\"report.txt\"}").unwrap();
    assert_eq!(
        first.fingerprint().unwrap(),
        reordered.fingerprint().unwrap()
    );
    let approval = ApprovalLedger::request(&mut db, first, 100, 30).unwrap();
    assert_eq!(
        ApprovalLedger::decide(&mut db, &approval.id, false, 101)
            .unwrap()
            .state,
        ApprovalState::Denied
    );
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &reordered, 102).is_err());
}
