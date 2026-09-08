use polymas_core::messages::{MessageInput, Router};
use polymas_core::storage::Store;
use polymas_core::topology::{Architecture, Edge};

fn architecture() -> Architecture {
    Architecture {
        edges: vec![
            Edge {
                from: "a".into(),
                to: "b".into(),
            },
            Edge {
                from: "b".into(),
                to: "a".into(),
            },
        ],
        ..Default::default()
    }
}
fn input(to: &str) -> MessageInput {
    MessageInput {
        recipient: to.into(),
        body: "Evidence from the assigned source".into(),
        ..Default::default()
    }
}
#[test]
fn retransmission_and_delivery_are_idempotent_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let mut db = Store::open(&path).unwrap();
    let message =
        Router::send(&mut db, &architecture(), "run", "a", "key", input("b"), 100).unwrap();
    for _ in 0..10 {
        assert_eq!(
            Router::send(&mut db, &architecture(), "run", "a", "key", input("b"), 100)
                .unwrap()
                .id,
            message.id
        );
    }
    drop(db);
    let mut db = Store::open(&path).unwrap();
    assert_eq!(Router::pending(&db, "b", 101, 10).unwrap().len(), 1);
    Router::deliver(
        &mut db,
        "b",
        std::slice::from_ref(&message.id),
        "attempt",
        101,
    )
    .unwrap();
    Router::deliver(&mut db, "b", &[message.id], "attempt", 101).unwrap();
    assert!(Router::pending(&db, "b", 102, 10).unwrap().is_empty());
}
#[test]
fn missing_edge_and_expired_messages_never_reach_context() {
    let mut db = Store::memory().unwrap();
    assert!(
        Router::send(
            &mut db,
            &Architecture::default(),
            "run",
            "a",
            "key",
            input("b"),
            100
        )
        .is_err()
    );
    let mut i = input("b");
    i.ttl_seconds = 1;
    let message = Router::send(&mut db, &architecture(), "run", "a", "key", i, 100).unwrap();
    assert!(Router::pending(&db, "b", 102, 10).unwrap().is_empty());
    assert!(Router::deliver(&mut db, "b", &[message.id], "attempt", 102).is_err());
}
#[test]
fn message_loops_stop_at_hop_budget_and_identity_cannot_be_forged() {
    let mut db = Store::memory().unwrap();
    let a = Architecture {
        message_hop_limit: 1,
        ..architecture()
    };
    let first = Router::send(&mut db, &a, "run", "a", "one", input("b"), 100).unwrap();
    let mut reply = input("a");
    reply.reply_to = Some(first.id.clone());
    reply.body = "Reply with new evidence".into();
    let second = Router::send(&mut db, &a, "run", "b", "two", reply, 101).unwrap();
    let mut next = input("b");
    next.reply_to = Some(second.id);
    assert!(Router::send(&mut db, &a, "run", "a", "three", next, 102).is_err());
    let mut wrong = input("b");
    wrong.reply_to = Some(first.id);
    assert!(Router::send(&mut db, &a, "run", "a", "forged", wrong, 102).is_err());
}

#[test]
fn inbox_queries_scope_the_run_before_limiting_and_expose_only_delivered_messages() {
    let mut db = Store::memory().unwrap();
    for n in 0..100 {
        Router::send(
            &mut db,
            &architecture(),
            "other-run",
            "a",
            &format!("m{n}"),
            input("b"),
            100,
        )
        .unwrap();
    }
    let first = Router::send(
        &mut db,
        &architecture(),
        "current-run",
        "a",
        "first",
        input("b"),
        101,
    )
    .unwrap();
    let second = Router::send(
        &mut db,
        &architecture(),
        "current-run",
        "a",
        "second",
        input("b"),
        102,
    )
    .unwrap();
    assert_eq!(
        Router::pending_for_run(&db, "current-run", "b", 103, 1).unwrap()[0].id,
        first.id
    );
    assert!(
        Router::inbox(&db, "current-run", "b", 0, 1, 103)
            .unwrap()
            .items
            .is_empty()
    );
    Router::deliver(
        &mut db,
        "b",
        &[first.id.clone(), second.id.clone()],
        "attempt",
        103,
    )
    .unwrap();
    let page = Router::inbox(&db, "current-run", "b", 0, 1, 104).unwrap();
    assert_eq!(page.items[0].id, first.id);
    assert!(page.has_more);
    let next = Router::inbox(&db, "current-run", "b", page.cursor, 1, 104).unwrap();
    assert_eq!(next.items[0].id, second.id);
    assert!(!next.has_more);
    assert!(Router::get_for_recipient(&db, "other-run", "b", &first.id).is_err());
    assert!(Router::get_for_recipient(&db, "current-run", "a", &first.id).is_err());
    assert!(
        Router::inbox(&db, "current-run", "a", 0, 100, 104)
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        Router::inbox(&db, "current-run", "b", 0, 100, 1000)
            .unwrap()
            .items
            .is_empty()
    );
}
