use polymas_core::connections::{Connection, Health};
use polymas_core::policy::{Binding, Subject, authorize_tool};
use std::collections::BTreeSet;

fn setup() -> (tempfile::TempDir, Connection, Binding) {
    let dir = tempfile::tempdir().unwrap();
    let mut connection = Connection::files("files", vec![dir.path().into()]);
    connection.health = Health::Ready;
    connection.tools = polymas_core::connections::files_tools();
    connection.tool_allowlist = vec!["read_text".into(), "list_files".into()];
    connection.enabled = true;
    let binding = Binding {
        id: "binding-a".into(),
        connection_id: connection.id.clone(),
        subject: Subject::Agent {
            id: "agent-a".into(),
        },
        tools: BTreeSet::from(["read_text".into()]),
        allow_write: false,
        required: true,
        namespace: "research".into(),
    };
    (dir, connection, binding)
}
#[test]
fn revocation_is_immediate_and_new_grants_do_not_expand_a_running_snapshot() {
    let (_dir, conn, binding) = setup();
    assert!(
        authorize_tool(
            "agent-a",
            "role-a",
            &conn,
            "read_text",
            std::slice::from_ref(&binding),
            std::slice::from_ref(&binding)
        )
        .is_ok()
    );
    assert!(
        authorize_tool(
            "agent-a",
            "role-a",
            &conn,
            "read_text",
            &[],
            std::slice::from_ref(&binding)
        )
        .is_err()
    );
    assert!(authorize_tool("agent-a", "role-a", &conn, "read_text", &[binding], &[]).is_err());
}
#[test]
fn another_agent_disabled_connection_and_unlisted_tool_are_denied() {
    let (_dir, mut conn, binding) = setup();
    let bindings = vec![binding];
    assert!(
        authorize_tool(
            "agent-b",
            "role-a",
            &conn,
            "read_text",
            &bindings,
            &bindings
        )
        .is_err()
    );
    assert!(
        authorize_tool(
            "agent-a",
            "role-a",
            &conn,
            "list_files",
            &bindings,
            &bindings
        )
        .is_err()
    );
    conn.enabled = false;
    assert!(
        authorize_tool(
            "agent-a",
            "role-a",
            &conn,
            "read_text",
            &bindings,
            &bindings
        )
        .is_err()
    );
}
