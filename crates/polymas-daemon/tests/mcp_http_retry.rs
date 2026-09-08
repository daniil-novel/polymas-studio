//! Actual loopback HTTP failure behavior, independent of a provider or database.
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use polymas_core::{
    connections::{Connection, ConnectionTransport, Health, ToolDescription},
    mcp_client::call_tool,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Clone)]
struct Fixture {
    failure: StatusCode,
    calls: Arc<AtomicUsize>,
    initializations: Arc<AtomicUsize>,
}
async fn rpc(State(state): State<Fixture>, Json(request): Json<Value>) -> Response {
    let id = request["id"].clone();
    match request["method"].as_str().unwrap_or_default() {
        "initialize" => {
            state.initializations.fetch_add(1, Ordering::SeqCst);
            ([("mcp-session-id","fixture-session")],Json(json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"write-http-fixture","version":"1"}}}))).into_response()
        }
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/call" => {
            state.calls.fetch_add(1, Ordering::SeqCst);
            state.failure.into_response()
        }
        _ => Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"unsupported"}}))
            .into_response(),
    }
}
#[tokio::test]
async fn session_expiry_and_server_failure_do_not_retransmit_an_external_write() {
    for failure in [StatusCode::NOT_FOUND, StatusCode::INTERNAL_SERVER_ERROR] {
        let fixture = Fixture {
            failure,
            calls: Arc::new(AtomicUsize::new(0)),
            initializations: Arc::new(AtomicUsize::new(0)),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/mcp", post(rpc))
            .with_state(fixture.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut connection = Connection::files("HTTP fixture", vec![]);
        connection.transport = ConnectionTransport::Http {
            url: format!("http://{address}/mcp"),
        };
        connection.enabled = true;
        connection.health = Health::Ready;
        connection.read_only = false;
        connection.timeout_seconds = 3;
        connection.tool_allowlist = vec!["write".into()];
        connection.tools = vec![ToolDescription {
            name: "write".into(),
            description: "Test write".into(),
            input_schema: json!({"type":"object"}),
            read_only: false,
        }];
        let result = call_tool(
            &connection,
            "write",
            json!({"text":"fixture"}),
            None,
            Default::default(),
        )
        .await;
        server.abort();
        let _ = server.await;
        assert!(result.is_err());
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1, "{failure}");
        assert_eq!(
            fixture.initializations.load(Ordering::SeqCst),
            1,
            "expired session must not be reinitialized to resend"
        );
    }
}
