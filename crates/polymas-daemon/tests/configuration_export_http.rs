use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::{connections::Connection, engine::Engine};
use polymas_daemon::{AppState, router};
use serde_json::Value;
use tower::ServiceExt;

const OWNER: &str = "configuration-export-owner-fixture-32-characters";

#[tokio::test]
async fn export_is_an_owner_only_complete_read_without_history_or_mutation() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    let connection = Connection::files("Files", vec![dir.path().to_owned()]);
    engine.workspace.save_connection(0, connection).unwrap();
    engine
        .workspace
        .db
        .transaction(|tx| {
            tx.put(
                "private-fixture",
                "entry",
                0,
                &serde_json::json!({"result":"PRIVATE-RESULT"}),
            )?;
            Ok(())
        })
        .unwrap();
    let state = AppState::new(engine, OWNER.into(), vec!["http://127.0.0.1:1420".into()]).unwrap();
    let app = router(state.clone());
    let request = |token: &str, origin: &str| {
        Request::builder()
            .uri("/api/exports/configuration")
            .header("authorization", format!("Bearer {token}"))
            .header("origin", origin)
            .body(Body::empty())
            .unwrap()
    };
    for (token, origin) in [
        ("agent-fixture-token", "http://127.0.0.1:1420"),
        (OWNER, "https://foreign.invalid"),
    ] {
        let response = app.clone().oneshot(request(token, origin)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let before = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    let response = app
        .clone()
        .oneshot(request(OWNER, "http://127.0.0.1:1420"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=polymas-configuration.json"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!text.contains("PRIVATE-RESULT"));
    assert!(
        !text.contains(
            &dir.path()
                .to_string_lossy()
                .to_string()
                .replace('\\', "\\\\")
        )
    );
    assert_eq!(document["connections"].as_array().unwrap().len(), 1);
    assert_eq!(document["architecture"]["paused"], true);
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .last_sequence()
            .unwrap(),
        before
    );
    let repeated = app
        .oneshot(request(OWNER, "http://127.0.0.1:1420"))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(bytes, repeated);
}

#[tokio::test]
async fn a_connection_list_larger_than_the_limit_is_not_silently_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(dir.path()).unwrap();
    engine
        .workspace
        .db
        .transaction(|tx| {
            for index in 0..1001 {
                let mut value = Connection::files("Fixture", vec![dir.path().to_owned()]);
                value.id = format!("connection_{index}");
                tx.put("connections", &value.id, 0, &value)?;
            }
            Ok(())
        })
        .unwrap();
    let state = AppState::new(engine, OWNER.into(), vec![]).unwrap();
    let before = state
        .engine
        .lock()
        .await
        .workspace
        .db
        .last_sequence()
        .unwrap();
    let response = router(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/exports/configuration")
                .header("authorization", format!("Bearer {OWNER}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("1000"));
    assert_eq!(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .last_sequence()
            .unwrap(),
        before
    );
}
