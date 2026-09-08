use polymas_core::connections::{Connection, ConnectionTransport, Health, read_file};
use std::path::PathBuf;

#[test]
fn enabling_untested_connection_never_turns_it_green() {
    let mut connection = Connection::files("Knowledge", vec![std::env::temp_dir()]);
    assert!(connection.set_enabled(true).is_err());
    assert!(!connection.enabled);
    connection.health = Health::Ready;
    connection.set_enabled(true).unwrap();
    assert!(connection.enabled);
    connection.set_enabled(false).unwrap();
    assert!(!connection.enabled);
}
#[test]
fn http_credentials_in_url_and_shell_wrappers_are_rejected() {
    let mut c = Connection::files("Knowledge", vec![std::env::temp_dir()]);
    c.transport = ConnectionTransport::Http {
        url: "https://user:password@example.com/mcp".into(),
    };
    assert!(c.validate().is_err());
    c.transport = ConnectionTransport::Http {
        url: "http://example.com/mcp".into(),
    };
    assert!(c.validate().is_err());
    c.transport = ConnectionTransport::Stdio {
        command: PathBuf::from("powershell.exe"),
        args: vec!["-Command".into(), "anything".into()],
    };
    assert!(c.validate().is_err());
}
#[test]
fn files_are_read_only_and_parent_escape_is_denied() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("evidence.txt"), "trusted evidence").unwrap();
    let roots = vec![dir.path().to_path_buf()];
    assert_eq!(
        read_file(&roots, 0, "evidence.txt").unwrap(),
        "trusted evidence"
    );
    assert!(read_file(&roots, 0, "../outside.txt").is_err());
    assert!(read_file(&roots, 0, "C:/Windows/win.ini").is_err());
}
