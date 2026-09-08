use polymas_core::policy::{ApprovalLedger, ApprovalState, Operation};
use polymas_core::storage::Store;
use serde_json::json;

fn operation() -> Operation {
    Operation {
        agent_id: "agent-a".into(),
        task_id: "task-a".into(),
        epoch: 7,
        connection_id: "db".into(),
        connection_revision: 2,
        tool: "write".into(),
        arguments: json!({"path":"report.txt","text":"approved text"}),
    }
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
