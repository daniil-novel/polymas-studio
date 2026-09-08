use polymas_core::{
    communication::{ExchangeScope, PublicationInput, PublicationQuery, Publications},
    storage::Store,
    topology::CommunicationMode,
};

fn scope(author: &str) -> ExchangeScope {
    ExchangeScope {
        run_id: "run-one".into(),
        author: author.into(),
        mode: CommunicationMode::Blackboard,
        readable_authors: vec!["a".into(), "b".into()],
        max_publications: 3,
        hop_limit: 8,
    }
}
fn input(key: &str) -> PublicationInput {
    PublicationInput {
        key: key.into(),
        title: "Наблюдение".into(),
        body: "Evidence 🧠".into(),
        ..Default::default()
    }
}

#[test]
fn board_is_durable_append_only_and_exact_retries_do_not_spend_budget() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.db");
    let mut db = Store::open(&path).unwrap();
    let first = Publications::publish(&mut db, &scope("a"), input("one")).unwrap();
    let sequence = db.last_sequence().unwrap();
    drop(db);
    let mut db = Store::open(&path).unwrap();
    assert_eq!(
        Publications::publish(&mut db, &scope("a"), input("one"))
            .unwrap()
            .id,
        first.id
    );
    assert_eq!(db.last_sequence().unwrap(), sequence);
    let mut changed = input("one");
    changed.body = "changed".into();
    assert!(Publications::publish(&mut db, &scope("a"), changed).is_err());
    assert_eq!(
        Publications::publish(&mut db, &scope("b"), input("one"))
            .unwrap()
            .author,
        "b"
    );
    Publications::publish(&mut db, &scope("a"), input("three")).unwrap();
    assert!(Publications::publish(&mut db, &scope("a"), input("four")).is_err());
    assert_eq!(
        Publications::publish(&mut db, &scope("a"), input("one"))
            .unwrap()
            .body,
        "Evidence 🧠"
    );
    assert_eq!(
        Publications::page(&db, "run-one", &PublicationQuery::default(), None)
            .unwrap()
            .items
            .len(),
        3
    );
    db.integrity_check().unwrap();
}

#[test]
fn board_pages_filter_access_before_limit_and_reject_invalid_inputs_atomically() {
    let mut db = Store::memory().unwrap();
    Publications::publish(&mut db, &scope("a"), input("private")).unwrap();
    let visible = Publications::publish(&mut db, &scope("b"), input("visible")).unwrap();
    let query = PublicationQuery {
        limit: 1,
        ..Default::default()
    };
    let page = Publications::page(&db, "run-one", &query, Some(&["b".into()])).unwrap();
    assert_eq!(page.items[0].id, visible.id);
    assert!(!page.has_more);
    assert!(
        Publications::page(&db, "another-run", &query, None)
            .unwrap()
            .items
            .is_empty()
    );
    let before = db.last_sequence().unwrap();
    for bad in [
        PublicationInput {
            body: "🧠".repeat(4097),
            ..input("big")
        },
        PublicationInput {
            expected_revision: 1,
            ..input("edit")
        },
        PublicationInput {
            slug: Some("../secret".into()),
            ..input("path")
        },
        PublicationInput {
            key: " ".into(),
            ..input("blank")
        },
    ] {
        assert!(Publications::publish(&mut db, &scope("a"), bad).is_err());
    }
    assert_eq!(db.last_sequence().unwrap(), before);
    assert!(
        Publications::page(
            &db,
            "run-one",
            &PublicationQuery { limit: 51, ..query },
            None
        )
        .is_err()
    );
}
