use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::engine::Engine;
use polymas_daemon::{AppState, router};
use serde_json::json;
use tower::ServiceExt;
#[tokio::test]
async fn control_plane_rejects_missing_token_and_foreign_origin_before_reading_state() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(
        Engine::open(dir.path()).unwrap(),
        "test-token-at-least-32-characters-long".into(),
        vec!["http://127.0.0.1:1420".into()],
    )
    .unwrap();
    let app = router(state);
    for origin in [None, Some("https://untrusted.example")] {
        let mut r = Request::builder().uri("/api/state");
        if let Some(origin) = origin {
            r = r
                .header(
                    "authorization",
                    "Bearer test-token-at-least-32-characters-long",
                )
                .header("origin", origin);
        }
        let response = app
            .clone()
            .oneshot(r.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .header(
                    "authorization",
                    "Bearer test-token-at-least-32-characters-long",
                )
                .header("origin", "http://127.0.0.1:1420")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
#[tokio::test]
async fn an_authenticated_edit_survives_http_and_returns_a_conflict_for_stale_revision() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(
        Engine::open(dir.path()).unwrap(),
        "test-token-at-least-32-characters-long".into(),
        vec![],
    )
    .unwrap();
    let app = router(state);
    let request = json!({"revision":1,"command":{"type":"rename_workspace","name":"Team"}});
    for expected in [StatusCode::OK, StatusCode::CONFLICT] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/configure")
                    .header(
                        "authorization",
                        "Bearer test-token-at-least-32-characters-long",
                    )
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/state")
                .header(
                    "authorization",
                    "Bearer test-token-at-least-32-characters-long",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["config"]["value"]["name"], "Team");
}
