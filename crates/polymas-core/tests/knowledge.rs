use polymas_core::knowledge::{ClaimDraft, Knowledge, Relation, SourceDraft, Verification};
use polymas_core::storage::Store;

fn source() -> SourceDraft {
    SourceDraft {
        name: "Requirements".into(),
        uri: "https://example.org/spec".into(),
        version: "sha256:fixture-version".into(),
        namespace: "project-a".into(),
        connection_id: None,
        retention_days: 30,
    }
}
#[test]
fn claims_preserve_source_version_and_begin_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let mut db = Store::open(&path).unwrap();
    let first = Knowledge::add_source(&mut db, source()).unwrap();
    let claim = Knowledge::publish(
        &mut db,
        "agent-a",
        ClaimDraft {
            assertion: "One instance has one session".into(),
            source_id: first.id.clone(),
            namespace: "project-a".into(),
        },
        100,
    )
    .unwrap();
    assert_eq!(claim.status, Verification::Unverified);
    assert_eq!(claim.source_version, first.draft.version);
    let mut next = source();
    next.version = "sha256:changed".into();
    Knowledge::add_source(&mut db, next).unwrap();
    drop(db);
    let db = Store::open(&path).unwrap();
    assert_eq!(
        Knowledge::claims(&db, "project-a", 101).unwrap()[0].source_version,
        first.draft.version
    );
}
#[test]
fn namespace_escape_and_expired_sources_are_rejected_without_partial_claims() {
    let mut db = Store::memory().unwrap();
    let src = Knowledge::add_source(&mut db, source()).unwrap();
    assert!(
        Knowledge::publish(
            &mut db,
            "agent-a",
            ClaimDraft {
                assertion: "x".into(),
                source_id: src.id.clone(),
                namespace: "project-b".into()
            },
            100
        )
        .is_err()
    );
    assert!(Knowledge::claims(&db, "project-b", 100).unwrap().is_empty());
    let claim = Knowledge::publish(
        &mut db,
        "agent-a",
        ClaimDraft {
            assertion: "x".into(),
            source_id: src.id,
            namespace: "project-a".into(),
        },
        100,
    )
    .unwrap();
    assert!(
        Knowledge::claims(&db, "project-a", 100 + 31 * 86400)
            .unwrap()
            .is_empty()
    );
    assert!(Knowledge::verify(&mut db, &claim.id, Verification::Verified, "", 101).is_err());
}
#[test]
fn contradictions_are_explicit_and_verification_requires_a_reviewer() {
    let mut db = Store::memory().unwrap();
    let src = Knowledge::add_source(&mut db, source()).unwrap();
    let a = Knowledge::publish(
        &mut db,
        "a",
        ClaimDraft {
            assertion: "A".into(),
            source_id: src.id.clone(),
            namespace: "project-a".into(),
        },
        100,
    )
    .unwrap();
    let b = Knowledge::publish(
        &mut db,
        "b",
        ClaimDraft {
            assertion: "B".into(),
            source_id: src.id,
            namespace: "project-a".into(),
        },
        100,
    )
    .unwrap();
    let link = Knowledge::relate(&mut db, &a.id, &b.id, Relation::Contradicts, 101).unwrap();
    assert_eq!(link.from, a.id);
    let verified =
        Knowledge::verify(&mut db, &a.id, Verification::Verified, "human:owner", 101).unwrap();
    assert_eq!(verified.verified_by.as_deref(), Some("human:owner"));
    assert_eq!(Knowledge::claims(&db, "project-a", 101).unwrap().len(), 2);
}
