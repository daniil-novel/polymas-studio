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
        origin_epoch: 6,
        epoch: 7,
        connection_id: "db".into(),
        connection_revision: 2,
        tool: "write".into(),
        arguments: json!({"path":"report.txt","text":"approved text"}),
    }
}
#[test]
fn an_approval_is_bound_to_distinct_origin_and_execution_attempts() {
    let mut value = serde_json::to_value(operation()).unwrap();
    value["originEpoch"] = json!(6);
    let requested: Operation = serde_json::from_value(value.clone()).unwrap();
    let mut db = Store::memory().unwrap();
    let approval =
        ApprovalLedger::request_for_attempt(&mut db, "request", requested.clone(), 100, 30)
            .unwrap();
    assert_eq!(
        ApprovalLedger::for_attempt(&db, "task-a", 6)
            .unwrap()
            .unwrap()
            .id,
        approval.id
    );
    assert!(
        ApprovalLedger::for_attempt(&db, "task-a", 7)
            .unwrap()
            .is_none()
    );
    ApprovalLedger::decide(&mut db, &approval.id, true, 101).unwrap();
    value["originEpoch"] = json!(5);
    let other: Operation = serde_json::from_value(value.clone()).unwrap();
    assert!(ApprovalLedger::consume(&mut db, &approval.id, &other, 102).is_err());
    for origin in [0, 7, 8] {
        value["originEpoch"] = json!(origin);
        assert!(
            serde_json::from_value::<Operation>(value.clone())
                .unwrap()
                .fingerprint()
                .is_err()
        );
    }
    value.as_object_mut().unwrap().remove("originEpoch");
    assert!(
        serde_json::from_value::<Operation>(value)
            .unwrap()
            .fingerprint()
            .is_err()
    );
    ApprovalLedger::consume(&mut db, &approval.id, &requested, 102).unwrap();
}
#[test]
fn attempt_requests_retry_atomically_without_duplicates_or_extending_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("approvals.db");
    let mut db = Store::open(&path).unwrap();
    let first =
        ApprovalLedger::request_for_attempt(&mut db, "request-1", operation(), 100, 30).unwrap();
    let head = db.last_sequence().unwrap();
    drop(db);
    let mut db = Store::open(&path).unwrap();
    let retried =
        ApprovalLedger::request_for_attempt(&mut db, "request-1", operation(), 110, 60).unwrap();
    assert_eq!(retried.id, first.id);
    assert_eq!(retried.expires_at, 130);
    assert_eq!(db.last_sequence().unwrap(), head);
    let mut changed = operation();
    changed.arguments["text"] = json!("other");
    assert!(ApprovalLedger::request_for_attempt(&mut db, "request-1", changed, 111, 30).is_err());
    assert!(
        ApprovalLedger::request_for_attempt(&mut db, "request-2", operation(), 111, 30).is_err()
    );
    let lookup = ApprovalLedger::for_attempt(&db, "task-a", 6)
        .unwrap()
        .unwrap();
    assert_eq!(lookup.id, first.id);
    assert!(
        ApprovalLedger::for_attempt(&db, "task-a", 8)
            .unwrap()
            .is_none()
    );
    assert!(
        ApprovalLedger::for_attempt(&db, "other-task", 7)
            .unwrap()
            .is_none()
    );
    ApprovalLedger::decide(&mut db, &first.id, false, 112).unwrap();
    assert_eq!(
        ApprovalLedger::request_for_attempt(&mut db, "request-1", operation(), 140, 60)
            .unwrap()
            .state,
        ApprovalState::Denied
    );
    let mut fresh = operation();
    fresh.origin_epoch = 7;
    fresh.epoch = 8;
    let new = ApprovalLedger::request_for_attempt(&mut db, "request-1", fresh, 140, 60).unwrap();
    assert_ne!(new.id, first.id);
    assert_eq!(new.state, ApprovalState::Pending);
    assert_eq!(
        db.list::<polymas_core::policy::Approval>("approvals", 20)
            .unwrap()
            .len(),
        2
    );
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
