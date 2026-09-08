use polymas_core::{
    knowledge::{
        Claim, Knowledge, KnowledgeCommand, KnowledgeQuery, KnowledgeView, Source, Verification,
    },
    storage::Store,
};
use serde_json::{Value, json};

fn execute(db: &mut Store, key: &str, value: Value) -> Value {
    Knowledge::execute(
        db,
        "human:owner",
        key,
        serde_json::from_value(value).unwrap(),
        100,
    )
    .unwrap()
}
fn add_source(db: &mut Store, key: &str, namespace: &str) -> String {
    execute(db, key, json!({"action":"add_source","draft":{"name":"Design record","uri":"https://example.org/design","version":"fixture-v1","namespace":namespace,"connectionId":null,"retentionDays":30}}))["id"].as_str().unwrap().into()
}

#[test]
fn commands_replay_once_and_stale_review_cannot_overwrite_a_decision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("knowledge.sqlite3");
    let mut db = Store::open(&path).unwrap();
    let source = add_source(&mut db, "source", "team-a");
    let command = json!({"action":"publish","draft":{"assertion":"One instance owns its sessions","sourceId":source,"namespace":"team-a"}});
    let claim = execute(&mut db, "publish", command.clone());
    let head = db.last_sequence().unwrap();
    drop(db);
    let mut db = Store::open(&path).unwrap();
    assert_eq!(execute(&mut db, "publish", command.clone()), claim);
    assert_eq!(db.last_sequence().unwrap(), head);
    assert_eq!(db.list::<Claim>("claims", 100).unwrap().len(), 1);
    let conflicting = json!({"action":"publish","draft":{"assertion":"Different command","sourceId":source,"namespace":"team-a"}});
    assert!(
        Knowledge::execute(
            &mut db,
            "human:owner",
            "publish",
            serde_json::from_value(conflicting).unwrap(),
            101
        )
        .is_err()
    );
    let review = json!({"action":"verify","claimId":claim["id"],"revision":1,"status":"verified"});
    execute(&mut db, "review", review.clone());
    let head = db.last_sequence().unwrap();
    assert!(
        Knowledge::execute(
            &mut db,
            "human:owner",
            "late-review",
            serde_json::from_value(review).unwrap(),
            102
        )
        .is_err()
    );
    assert_eq!(db.last_sequence().unwrap(), head);
    let stored = db
        .get::<Claim>("claims", claim["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored.revision, 2);
    assert_eq!(stored.value.status, Verification::Verified);
    assert_eq!(stored.value.verified_by.as_deref(), Some("human:owner"));
    assert_eq!(db.list::<Source>("sources", 100).unwrap().len(), 1);
}

#[test]
fn pages_filter_before_limit_and_links_do_not_resurface_expired_claims() {
    let mut db = Store::memory().unwrap();
    let a = add_source(&mut db, "a", "team-a");
    let b = add_source(&mut db, "b", "team-b");
    let mut ids = vec![];
    for i in 0..7 {
        let namespace = if i % 2 == 0 { "team-b" } else { "team-a" };
        let source = if i % 2 == 0 { &b } else { &a };
        let claim = execute(
            &mut db,
            &format!("c{i}"),
            json!({"action":"publish","draft":{"assertion":format!("record {i}"),"sourceId":source,"namespace":namespace}}),
        );
        if namespace == "team-a" {
            ids.push(claim["id"].as_str().unwrap().to_string());
        }
    }
    execute(
        &mut db,
        "link",
        json!({"action":"relate","from":ids[0],"to":ids[1],"relation":"contradicts"}),
    );
    let mut query = KnowledgeQuery {
        namespace: Some("team-a".into()),
        limit: 1,
        ..Default::default()
    };
    let mut found = vec![];
    loop {
        let page = Knowledge::page(&db, KnowledgeView::Claims, &query, 101).unwrap();
        assert!(page.items.len() <= 1);
        for item in page.items {
            assert_eq!(item.value["namespace"], "team-a");
            found.push(item.value["id"].as_str().unwrap().to_string());
        }
        query.cursor = page.next_cursor;
        if query.cursor.is_none() {
            break;
        }
    }
    found.sort();
    ids.sort();
    assert_eq!(found, ids);
    query.cursor = None;
    query.query = "record 3".into();
    assert_eq!(
        Knowledge::page(&db, KnowledgeView::Claims, &query, 101)
            .unwrap()
            .items
            .len(),
        1
    );
    query.query.clear();
    assert_eq!(
        Knowledge::page(&db, KnowledgeView::Links, &query, 101)
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        Knowledge::page(&db, KnowledgeView::Links, &query, 100 + 31 * 86400)
            .unwrap()
            .items
            .is_empty()
    );
    query.include_expired = true;
    assert_eq!(
        Knowledge::page(&db, KnowledgeView::Claims, &query, 100 + 31 * 86400)
            .unwrap()
            .items
            .len(),
        1
    );
    query.limit = 101;
    assert!(Knowledge::page(&db, KnowledgeView::Sources, &query, 101).is_err());
}

#[test]
fn invalid_command_has_no_receipt_and_can_be_corrected_without_partial_writes() {
    let mut db = Store::memory().unwrap();
    let invalid: KnowledgeCommand = serde_json::from_value(json!({"action":"publish","draft":{"assertion":"x","sourceId":"missing","namespace":"team-a"}})).unwrap();
    assert!(Knowledge::execute(&mut db, "human:owner", "key", invalid, 100).is_err());
    assert_eq!(db.last_sequence().unwrap(), 0);
    assert!(
        db.list::<Value>("knowledge_commands", 100)
            .unwrap()
            .is_empty()
    );
    add_source(&mut db, "key", "team-a");
}

#[test]
fn text_search_matches_cyrillic_case_and_treats_percent_as_literal() {
    let mut db = Store::memory().unwrap();
    let source = add_source(&mut db, "source", "shared");
    execute(
        &mut db,
        "claim",
        json!({"action":"publish","draft":{"assertion":"Проверено 100% УСЛОВИЙ","sourceId":source,"namespace":"shared"}}),
    );
    for text in ["пРОВЕРЕНО", "100%", "условий"] {
        let query = KnowledgeQuery {
            query: text.into(),
            ..Default::default()
        };
        assert_eq!(
            Knowledge::page(&db, KnowledgeView::Claims, &query, 101)
                .unwrap()
                .items
                .len(),
            1,
            "{text}"
        );
    }
    let query = KnowledgeQuery {
        query: "_".into(),
        ..Default::default()
    };
    assert!(
        Knowledge::page(&db, KnowledgeView::Claims, &query, 101)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn scoped_memory_pages_filter_source_connections_before_limiting_claims_and_links() {
    let mut db = Store::memory().unwrap();
    let mut visible = vec![];
    let mut hidden = None;
    for i in 0..8 {
        let connection = if i % 3 == 0 {
            "shared-connection"
        } else {
            "private-connection"
        };
        let source = execute(
            &mut db,
            &format!("s{i}"),
            json!({"action":"add_source","draft":{"name":format!("source {i}"),"uri":"https://example.org/spec","version":"fixture-v1","namespace":"shared","connectionId":connection,"retentionDays":30}}),
        );
        let claim = execute(
            &mut db,
            &format!("c{i}"),
            json!({"action":"publish","draft":{"assertion":format!("assertion {i}"),"sourceId":source["id"],"namespace":"shared"}}),
        );
        if connection == "shared-connection" {
            visible.push(claim["id"].as_str().unwrap().to_string());
        } else {
            hidden = Some(claim["id"].as_str().unwrap().to_string());
        }
    }
    execute(
        &mut db,
        "visible-link",
        json!({"action":"relate","from":visible[0],"to":visible[1],"relation":"supports"}),
    );
    execute(
        &mut db,
        "hidden-link",
        json!({"action":"relate","from":visible[0],"to":hidden,"relation":"contradicts"}),
    );
    let mut query = KnowledgeQuery {
        namespace: Some("shared".into()),
        limit: 1,
        ..Default::default()
    };
    let mut found = vec![];
    loop {
        let page = Knowledge::page_for_connection(
            &db,
            KnowledgeView::Claims,
            &query,
            101,
            "shared-connection",
        )
        .unwrap();
        found.extend(
            page.items
                .into_iter()
                .map(|i| i.value["id"].as_str().unwrap().to_string()),
        );
        query.cursor = page.next_cursor;
        if query.cursor.is_none() {
            break;
        }
    }
    found.sort();
    visible.sort();
    assert_eq!(found, visible);
    query.cursor = None;
    query.limit = 100;
    let links =
        Knowledge::page_for_connection(&db, KnowledgeView::Links, &query, 101, "shared-connection")
            .unwrap();
    assert_eq!(links.items.len(), 1);
    assert_eq!(links.items[0].value["relation"], "supports");
    query.namespace = None;
    assert!(
        Knowledge::page_for_connection(
            &db,
            KnowledgeView::Claims,
            &query,
            101,
            "shared-connection"
        )
        .is_err()
    );
}
