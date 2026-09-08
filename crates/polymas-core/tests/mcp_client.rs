use polymas_core::connections::{Connection, ConnectionTransport};
use polymas_core::mcp_client::{call_tool, probe_connection};
use serde_json::json;

fn python_path() -> std::path::PathBuf {
    let output = std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-c", "import sys;print(sys.executable)"])
        .output()
        .unwrap();
    std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}
#[tokio::test]
async fn real_stdio_handshake_lists_and_calls_a_tool() {
    let mut connection = Connection::files("Fixture", vec![std::env::temp_dir()]);
    connection.transport = ConnectionTransport::Stdio {
        command: python_path(),
        args: vec![
            "-u".into(),
            format!(
                "{}/tests/fixtures/mcp_server.py",
                env!("CARGO_MANIFEST_DIR")
            ),
        ],
    };
    let probe = probe_connection(&connection, None, Default::default())
        .await
        .unwrap();
    assert_eq!(probe.tools[0].name, "echo");
    connection.tools = probe.tools;
    connection.health = polymas_core::connections::Health::Ready;
    connection.tool_allowlist = vec!["echo".into()];
    connection.read_only = false;
    connection.set_enabled(true).unwrap();
    let result = call_tool(
        &connection,
        "echo",
        json!({"text":"real transport"}),
        None,
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(result["content"][0]["text"], "real transport");
}
#[tokio::test]
async fn a_disabled_connection_or_unlisted_tool_is_denied_before_spawn() {
    let mut c = Connection::files("files", vec![std::env::temp_dir()]);
    assert!(
        call_tool(
            &c,
            "read_text",
            json!({"root":0,"path":"x"}),
            None,
            Default::default()
        )
        .await
        .is_err()
    );
    c.health = polymas_core::connections::Health::Ready;
    c.enabled = true;
    assert!(
        call_tool(&c, "write_file", json!({}), None, Default::default())
            .await
            .is_err()
    );
}
