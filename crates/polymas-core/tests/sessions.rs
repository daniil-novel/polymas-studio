use polymas_core::{
    sessions::SessionRegistry,
    storage::{EventInput, Store},
};
use serde_json::json;
const OLD: &str = "0199a213-81c0-7800-8aa1-bbab2a035a53";
const NEW: &str = "0199a213-81c0-7800-8aa1-bbab2a035a54";
const EVENT: &str = "0199a213-81c0-7800-8aa1-bbab2a035a55";
const STARTED: &str = "0199a213-81c0-7800-8aa1-bbab2a035a56";

#[test]
fn ownership_survives_a_new_conversation_and_legacy_task_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.sqlite3");
    let mut db = Store::open(&path).unwrap();
    db.transaction(|tx| {
        tx.put(
            "agent_runtime",
            "a",
            0,
            &json!({"agentId":"a","sessionId":NEW}),
        )?;
        tx.put(
            "task_runtime",
            "old-task",
            0,
            &json!({"agentId":"a","sessionId":OLD}),
        )?;
        tx.emit(EventInput::new("runtime.result_saved", json!({"sessionId":EVENT})).agent("a"))?;
        tx.emit(
            EventInput::new("runtime.session", json!({"data":{"thread_id":STARTED}})).agent("a"),
        )?;
        Ok(())
    })
    .unwrap();
    SessionRegistry::migrate(&mut db).unwrap();
    drop(db);
    let mut db = Store::open(&path).unwrap();
    SessionRegistry::migrate(&mut db).unwrap();
    for session in [OLD, NEW, EVENT, STARTED] {
        SessionRegistry::verify(&db, session, "a").unwrap();
        assert!(SessionRegistry::verify(&db, session, "b").is_err());
        assert!(
            db.transaction(|tx| SessionRegistry::register(tx, session, "b"))
                .is_err()
        );
    }
    db.transaction(|tx| SessionRegistry::register(tx, &OLD.to_uppercase(), "a"))
        .unwrap();
    assert!(SessionRegistry::verify(&db, "0199a213-81c0-7800-8aa1-bbab2a035a99", "a").is_err());
}

#[test]
fn conflicting_legacy_owners_abort_the_migration_without_partial_ownership() {
    let mut db = Store::memory().unwrap();
    db.transaction(|tx| {
        tx.put(
            "agent_runtime",
            "a",
            0,
            &json!({"agentId":"a","sessionId":OLD}),
        )?;
        tx.put(
            "task_runtime",
            "foreign",
            0,
            &json!({"agentId":"b","sessionId":OLD}),
        )?;
        Ok(())
    })
    .unwrap();
    assert!(SessionRegistry::migrate(&mut db).is_err());
    assert!(SessionRegistry::verify(&db, OLD, "a").is_err());
    assert!(SessionRegistry::verify(&db, OLD, "b").is_err());
}
