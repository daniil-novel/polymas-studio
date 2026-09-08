use polymas_core::{
    messages::{MessageInput, Router},
    storage::Store,
    topology::Architecture,
};
use serde_json::json;

#[test]
fn legacy_architecture_defaults_to_direct_and_modes_roundtrip_without_changing_topology() {
    let legacy = serde_json::to_value(Architecture::default()).unwrap();
    for mode in ["direct", "blackboard", "wiki", "graph"] {
        let mut document = legacy.clone();
        document["communicationMode"] = json!(mode);
        let architecture: Architecture = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(serde_json::to_value(architecture).unwrap(), document);
    }
    let mut old = legacy;
    old.as_object_mut().unwrap().remove("communicationMode");
    let restored: Architecture = serde_json::from_value(old).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap()["communicationMode"],
        "direct"
    );
    assert!(
        serde_json::from_value::<Architecture>(json!({"communicationMode":"telepathy"})).is_err()
    );
}

#[test]
fn shared_mode_cannot_bypass_publication_protocol_with_direct_messages() {
    let mut db = Store::memory().unwrap();
    let architecture: Architecture = serde_json::from_value(json!({
        "communicationMode":"blackboard", "edges":[{"from":"a","to":"b"}]
    }))
    .unwrap();
    let before = db.last_sequence().unwrap();
    assert!(
        Router::send(
            &mut db,
            &architecture,
            "run",
            "a",
            "one",
            MessageInput {
                recipient: "b".into(),
                body: "must use board".into(),
                ..Default::default()
            },
            100
        )
        .is_err()
    );
    assert_eq!(db.last_sequence().unwrap(), before);
}
