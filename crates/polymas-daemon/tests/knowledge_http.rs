use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use polymas_core::engine::Engine;
use polymas_daemon::{AppState, router};
use serde_json::{Value, json};
use tower::ServiceExt;
const TOKEN: &str = "knowledge-test-token-at-least-32-characters";
async fn request(
    state: &AppState,
    path: &str,
    body: Option<Value>,
    token: bool,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .uri(path)
        .header("content-type", "application/json");
    if token {
        builder = builder.header("authorization", format!("Bearer {TOKEN}"));
    }
    if body.is_some() {
        builder = builder.method("POST");
    }
    let response = router(state.clone())
        .oneshot(
            builder
                .body(
                    body.map(|b| Body::from(b.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn authenticated_knowledge_editor_preserves_provenance_and_rejects_stale_review() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(Engine::open(dir.path()).unwrap(), TOKEN.into(), vec![]).unwrap();
    assert_eq!(
        request(&state, "/api/knowledge/sources", None, false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let source = json!({"key":"source","command":{"action":"add_source","draft":{"name":"Architecture","uri":"https://example.org/design","version":"fixture-v1","namespace":"shared","connectionId":null,"retentionDays":30}}});
    let (status, saved) =
        request(&state, "/api/knowledge/command", Some(source.clone()), true).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        request(&state, "/api/knowledge/command", Some(source), true)
            .await
            .1,
        saved
    );
    let claim = json!({"key":"claim","command":{"action":"publish","draft":{"assertion":"Independent sessions","sourceId":saved["result"]["id"],"namespace":"shared"}}});
    let (status, saved) = request(&state, "/api/knowledge/command", Some(claim), true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["result"]["status"], "unverified");
    assert_eq!(saved["result"]["author"], "human:owner");
    let id = saved["result"]["id"].as_str().unwrap();
    let (status, detail) =
        request(&state, &format!("/api/knowledge/claims/{id}"), None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["claim"]["revision"], 1);
    assert_eq!(detail["source"]["value"]["version"], "fixture-v1");
    assert_eq!(detail["expired"], false);
    let review = json!({"key":"review","command":{"action":"verify","claimId":id,"revision":1,"status":"verified"}});
    assert_eq!(
        request(&state, "/api/knowledge/command", Some(review.clone()), true)
            .await
            .0,
        StatusCode::OK
    );
    let mut late = review;
    late["key"] = json!("late");
    assert_eq!(
        request(&state, "/api/knowledge/command", Some(late), true)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (status, page) = request(
        &state,
        "/api/knowledge/claims?namespace=shared&status=verified&limit=1",
        None,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["value"]["verifiedBy"], "human:owner");
    assert!(page["nextCursor"].is_null());
    // Turning agent memory off does not delete or hide the owner's knowledge.
    assert!(
        !state
            .engine
            .lock()
            .await
            .workspace
            .config()
            .value
            .architecture
            .memory_enabled
    );
    assert_eq!(
        request(&state, "/api/knowledge/sources", None, true)
            .await
            .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn editor_cannot_forge_an_agent_author_or_reference_an_unknown_connection() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(Engine::open(dir.path()).unwrap(), TOKEN.into(), vec![]).unwrap();
    let body = json!({"key":"bad","actor":"agent-forged","command":{"action":"add_source","draft":{"name":"x","uri":"https://example.org/design","version":"fixture-v1","namespace":"shared","connectionId":null,"retentionDays":30}}});
    assert_eq!(
        request(&state, "/api/knowledge/command", Some(body.clone()), true)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut body = body;
    body.as_object_mut().unwrap().remove("actor");
    body["command"]["draft"]["connectionId"] = json!("missing");
    assert_eq!(
        request(&state, "/api/knowledge/command", Some(body), true)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&state, "/api/knowledge/claims?limit=101", None, true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&state, "/api/knowledge/unknown", None, true)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
