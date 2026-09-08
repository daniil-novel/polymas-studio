use polymas_core::artifacts::{ArtifactInput, Artifacts};
use polymas_core::storage::Store;
fn input() -> ArtifactInput {
    ArtifactInput {
        name: "report.md".into(),
        media_type: "text/markdown".into(),
        agent_id: "agent-a".into(),
        task_id: "task-a".into(),
        attempt: 1,
        partial: false,
    }
}
#[test]
fn immutable_artifact_verifies_content_and_preserves_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let store = Artifacts::new(dir.path()).unwrap();
    let mut db = Store::memory().unwrap();
    let first = store.publish(&mut db, input(), b"# Result").unwrap();
    let second = store.publish(&mut db, input(), b"# Changed").unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(first.task_id, "task-a");
    assert_eq!(store.read(&first).unwrap(), b"# Result");
    std::fs::write(dir.path().join(&first.sha256), b"corrupt").unwrap();
    assert!(store.read(&first).is_err());
}
#[test]
fn importing_a_file_cannot_escape_the_assigned_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("agent");
    std::fs::create_dir(&work).unwrap();
    std::fs::write(dir.path().join("private"), "other agent").unwrap();
    std::fs::write(work.join("result.txt"), "ok").unwrap();
    let store = Artifacts::new(&dir.path().join("artifacts")).unwrap();
    let mut db = Store::memory().unwrap();
    assert!(
        store
            .import_file(&mut db, input(), &work, "../private")
            .is_err()
    );
    let a = store
        .import_file(&mut db, input(), &work, "result.txt")
        .unwrap();
    assert_eq!(store.read(&a).unwrap(), b"ok");
}
