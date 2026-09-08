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
fn receipts_written_before_graph_links_remain_retryable_after_upgrade() {
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct LegacyInput {
        key: String,
        title: String,
        body: String,
        slug: Option<String>,
        expected_revision: u32,
    }
    let mut db = Store::memory().unwrap();
    let exchange = scope("a");
    let draft = input("legacy");
    let first = Publications::publish(&mut db, &exchange, draft.clone()).unwrap();
    let old = LegacyInput {
        key: draft.key.clone(),
        title: draft.title.clone(),
        body: draft.body.clone(),
        slug: None,
        expected_revision: 0,
    };
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(exchange.mode, old)).unwrap())
    );
    let key = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(&exchange.run_id, &exchange.author, &draft.key)).unwrap()
        )
    );
    // Authored pre-links receipt using the exact historical field order.
    db.transaction(|tx| {
        let saved = tx.get::<Value>("publication_receipts", &key)?.unwrap();
        tx.put(
            "publication_receipts",
            &key,
            saved.revision,
            &json!({"fingerprint":fingerprint,"entry_id":first.id}),
        )
    })
    .unwrap();
    let before = db.last_sequence().unwrap();
    assert_eq!(
        Publications::publish(&mut db, &exchange, draft.clone())
            .unwrap()
            .id,
        first.id
    );
    assert_eq!(db.last_sequence().unwrap(), before);
    assert!(
        Publications::publish(
            &mut db,
            &exchange,
            PublicationInput {
                body: "different".into(),
                ..draft
            }
        )
        .is_err()
    );
}

#[test]
fn graph_records_contradictions_and_bounds_causal_depth_without_losing_provenance() {
    use polymas_core::{communication::PublicationLink, knowledge::Relation};
    let mut db = Store::memory().unwrap();
    let graph = ExchangeScope {
        mode: CommunicationMode::Graph,
        hop_limit: 1,
        max_publications: 10,
        ..scope("a")
    };
    let root = Publications::publish(&mut db, &graph, input("root")).unwrap();
    let critique_input = PublicationInput {
        links: vec![PublicationLink {
            entry_id: root.id.clone(),
            relation: Relation::Contradicts,
        }],
        ..input("critique")
    };
    let critic = ExchangeScope {
        author: "b".into(),
        ..graph.clone()
    };
    let critique = Publications::publish(&mut db, &critic, critique_input.clone()).unwrap();
    assert_eq!(critique.depth, 1);
    assert_eq!(critique.contributors, vec!["a", "b"]);
    assert_eq!(critique.links[0].relation, Relation::Contradicts);
    let continuation = PublicationInput {
        links: vec![PublicationLink {
            entry_id: critique.id.clone(),
            relation: Relation::DerivedFrom,
        }],
        ..input("refine")
    };
    let before = db.last_sequence().unwrap();
    assert!(Publications::publish(&mut db, &graph, continuation.clone()).is_err());
    assert_eq!(db.last_sequence().unwrap(), before);
    let larger = ExchangeScope {
        hop_limit: 2,
        ..graph.clone()
    };
    assert_eq!(
        Publications::publish(&mut db, &larger, continuation)
            .unwrap()
            .depth,
        2
    );
    let unrelated = ExchangeScope {
        run_id: "another-run".into(),
        ..critic.clone()
    };
    assert!(Publications::publish(&mut db, &unrelated, critique_input.clone()).is_err());
    let private = ExchangeScope {
        readable_authors: vec!["b".into()],
        ..critic
    };
    assert!(Publications::publish(&mut db, &private, critique_input).is_err());
    let nonexistent = PublicationInput {
        links: vec![PublicationLink {
            entry_id: "future-node".into(),
            relation: Relation::Supports,
        }],
        ..input("future")
    };
    assert!(Publications::publish(&mut db, &graph, nonexistent).is_err());
    assert_eq!(
        Publications::page(
            &db,
            "run-one",
            &PublicationQuery::default(),
            Some(&["a".into()])
        )
        .unwrap()
        .items
        .len(),
        1
    );
}

fn wiki(author: &str) -> ExchangeScope {
    ExchangeScope {
        mode: CommunicationMode::Wiki,
        ..scope(author)
    }
}
fn page_input(key: &str, expected_revision: u32) -> PublicationInput {
    PublicationInput {
        slug: Some("shared-plan".into()),
        expected_revision,
        ..input(key)
    }
}

#[test]
fn wiki_concurrent_writers_cannot_lose_a_revision_or_consume_the_losing_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wiki.db");
    let mut db = Store::open(&path).unwrap();
    let original = Publications::publish(&mut db, &wiki("a"), page_input("create", 0)).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|author| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut other = Store::open(&path).unwrap();
                barrier.wait();
                Publications::publish(&mut other, &wiki(author), page_input("update", 1))
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let winner = results.iter().find_map(|r| r.as_ref().ok()).unwrap();
    assert_eq!(winner.revision, 2);
    let loser = if winner.author == "a" { "b" } else { "a" };
    let third = Publications::publish(&mut db, &wiki(loser), page_input("update", 2)).unwrap();
    assert_eq!(third.revision, 3);
    assert_eq!(third.contributors, vec!["a", "b"]);
    let all = Publications::page(&db, "run-one", &PublicationQuery::default(), None).unwrap();
    assert_eq!(all.items.len(), 3);
    assert_eq!(all.items[0].id, original.id);
    let latest = Publications::page(
        &db,
        "run-one",
        &PublicationQuery {
            latest_only: true,
            slug: Some("shared-plan".into()),
            ..Default::default()
        },
        None,
    )
    .unwrap();
    assert_eq!(latest.items[0].id, third.id);
    assert_eq!(latest.items.len(), 1);
    let mut revoked = wiki(loser);
    revoked.readable_authors = vec![loser.into()];
    assert!(Publications::publish(&mut db, &revoked, page_input("update", 2)).is_err());
    assert!(
        Publications::page(
            &db,
            "run-one",
            &PublicationQuery {
                latest_only: true,
                ..Default::default()
            },
            Some(&["b".into()])
        )
        .unwrap()
        .items
        .is_empty()
    );
}

#[test]
fn wiki_is_run_scoped_and_rejects_filesystem_paths_and_unknown_revisions() {
    let mut db = Store::memory().unwrap();
    for bad in [
        page_input("missing", 1),
        PublicationInput {
            slug: Some("../escape".into()),
            ..page_input("path", 0)
        },
    ] {
        assert!(Publications::publish(&mut db, &wiki("a"), bad).is_err());
    }
    let a = Publications::publish(&mut db, &wiki("a"), page_input("new", 0)).unwrap();
    let other = ExchangeScope {
        run_id: "run-two".into(),
        ..wiki("a")
    };
    let b = Publications::publish(&mut db, &other, page_input("new", 0)).unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(a.revision, b.revision);
    assert!(Publications::publish(&mut db, &wiki("b"), page_input("overwrite", 0)).is_err());
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
