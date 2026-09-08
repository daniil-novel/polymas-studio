use polymas_core::error::Error;
use polymas_core::storage::{EventInput, Store};
use serde_json::{Value, json};

fn event(kind: &str) -> EventInput {
    EventInput::new(kind, json!({"source": "test"}))
}

#[test]
fn configuration_and_event_commit_together_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let mut db = Store::open(&path).unwrap();
    db.transaction(|tx| {
        tx.put("config", "main", 0, &json!({"n": 1000}))?;
        tx.emit(event("configuration.saved"))?;
        Ok(())
    })
    .unwrap();
    drop(db);
    let db = Store::open(&path).unwrap();
    let saved = db.get::<Value>("config", "main").unwrap().unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.value["n"], 1000);
    assert_eq!(db.events(0, 10, None).unwrap().len(), 1);
}

#[test]
fn failed_transaction_leaves_no_half_configuration_or_event() {
    let mut db = Store::memory().unwrap();
    let result: Result<(), Error> = db.transaction(|tx| {
        tx.put("config", "main", 0, &json!({"n": 7}))?;
        tx.emit(event("must.not.persist"))?;
        Err(Error::Validation("Reject the whole operation".into()))
    });
    assert!(result.is_err());
    assert!(db.get::<Value>("config", "main").unwrap().is_none());
    assert!(db.events(0, 100, None).unwrap().is_empty());
}

#[test]
fn stale_editor_gets_conflict_instead_of_lost_update() {
    let mut db = Store::memory().unwrap();
    db.transaction(|tx| tx.put("config", "main", 0, &json!({"name":"one"})))
        .unwrap();
    assert!(
        db.transaction(|tx| tx.put("config", "main", 0, &json!({"name":"two"})))
            .is_err()
    );
    assert_eq!(
        db.get::<Value>("config", "main").unwrap().unwrap().value["name"],
        "one"
    );
}

#[test]
fn event_cursor_is_monotonic_and_replay_only_reads() {
    let mut db = Store::memory().unwrap();
    for n in 0..5 {
        db.transaction(|tx| tx.emit(EventInput::new("test", json!({"n":n}))))
            .unwrap();
    }
    let first = db.events(0, 2, None).unwrap();
    let second = db.events(first[1].sequence, 10, None).unwrap();
    assert_eq!(second.len(), 3);
    assert!(second[0].sequence > first[1].sequence);
    assert_eq!(db.events(0, 100, None).unwrap().len(), 5);
}
