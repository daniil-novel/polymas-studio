use polymas_core::artifacts::{Artifact, ArtifactInput, Artifacts};
use polymas_core::backup::Backup;
use polymas_core::storage::{EventInput, Store};
use serde_json::json;
#[test]
fn backup_restores_a_consistent_database_with_referenced_blobs_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Store::open(&dir.path().join("live.sqlite3")).unwrap();
    let artifacts = Artifacts::new(&dir.path().join("artifacts")).unwrap();
    let a = artifacts
        .publish(
            &mut db,
            ArtifactInput {
                name: "result.txt".into(),
                media_type: "text/plain".into(),
                agent_id: "a".into(),
                task_id: "t".into(),
                attempt: 1,
                partial: false,
            },
            b"result",
        )
        .unwrap();
    db.transaction(|tx| {
        tx.emit(EventInput::new("test", json!({"n":1})))?;
        Ok(())
    })
    .unwrap();
    let backup = dir.path().join("backup");
    Backup::create(&db, &artifacts, &backup).unwrap();
    db.transaction(|tx| {
        tx.emit(EventInput::new("after_backup", json!({})))?;
        Ok(())
    })
    .unwrap();
    let restored = dir.path().join("restored");
    Backup::restore(&backup, &restored).unwrap();
    let db2 = Store::open(&restored.join("state.sqlite3")).unwrap();
    assert_eq!(db2.last_sequence().unwrap(), 2);
    let saved = db2
        .get::<Artifact>("artifacts", &a.id)
        .unwrap()
        .unwrap()
        .value;
    assert_eq!(
        Artifacts::new(&restored.join("artifacts"))
            .unwrap()
            .read(&saved)
            .unwrap(),
        b"result"
    );
    assert!(Backup::restore(&backup, &restored).is_err());
}
#[test]
fn corruption_fails_before_publishing_a_restored_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let db = Store::memory().unwrap();
    let artifacts = Artifacts::new(&dir.path().join("artifacts")).unwrap();
    let backup = dir.path().join("backup");
    Backup::create(&db, &artifacts, &backup).unwrap();
    std::fs::write(backup.join("state.sqlite3"), b"corrupt").unwrap();
    let target = dir.path().join("new");
    assert!(Backup::restore(&backup, &target).is_err());
    assert!(!target.exists());
}
