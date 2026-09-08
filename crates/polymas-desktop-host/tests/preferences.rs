use polymas_desktop_host::preferences::{CloseBehavior, Preferences};

#[test]
fn closing_asks_until_a_choice_is_saved_and_can_be_reset_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("desktop.sqlite3");
    let preferences = Preferences::open(&path).unwrap();
    assert_eq!(preferences.close_behavior().unwrap(), None);
    preferences
        .set_close_behavior(Some(CloseBehavior::Background))
        .unwrap();
    drop(preferences);
    let preferences = Preferences::open(&path).unwrap();
    assert_eq!(
        preferences.close_behavior().unwrap(),
        Some(CloseBehavior::Background)
    );
    preferences
        .set_close_behavior(Some(CloseBehavior::Stop))
        .unwrap();
    assert_eq!(
        preferences.close_behavior().unwrap(),
        Some(CloseBehavior::Stop)
    );
    preferences.set_close_behavior(None).unwrap();
    assert_eq!(
        Preferences::open(&path).unwrap().close_behavior().unwrap(),
        None
    );
    assert!(serde_json::from_str::<CloseBehavior>("\"kill_all\"").is_err());
}
