use polymas_core::secrets::{OsVault, Secret};

#[test]
fn secret_debug_never_contains_the_value() {
    let secret = Secret::new("test-sensitive-value".into()).unwrap();
    assert!(!format!("{secret:?}").contains("test-sensitive-value"));
    assert_eq!(secret.expose(), "test-sensitive-value");
    assert!(Secret::new(String::new()).is_err());
}

#[test]
#[ignore = "Opt-in native credential-store test; creates then removes its own fixture entry"]
fn native_vault_roundtrip_is_scoped_and_removes_only_its_fixture() {
    let workspace = format!("test-{}", uuid::Uuid::new_v4());
    let vault = OsVault::new(&workspace).unwrap();
    let secret = Secret::new("polymas-vault-fixture".into()).unwrap();
    let reference = vault.save(&secret).unwrap();
    let read = vault.load(&reference).unwrap();
    vault.remove(&reference).unwrap();
    assert_eq!(read.expose(), secret.expose());
    assert!(vault.load(&reference).is_err());
}
