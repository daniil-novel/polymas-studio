use crate::{ApiError, AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use polymas_core::{
    error::Error,
    policy::{Approval, ApprovalLedger, ApprovalPage, ApprovalQuery, ApprovalState},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/approvals", get(list))
        .route("/api/approvals/{id}", get(detail))
        .route("/api/approvals/{id}/decision", post(decide))
        .route("/api/approvals/{id}/reconcile", post(reconcile))
}
async fn list(
    State(state): State<AppState>,
    Query(query): Query<ApprovalQuery>,
) -> Result<Json<ApprovalPage>, ApiError> {
    Ok(Json(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .approval_page(&query)?,
    ))
}
async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    let approval = engine
        .workspace
        .db
        .get::<Approval>("approvals", &id)?
        .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
    let connection = engine
        .workspace
        .db
        .get::<Value>("approval_resources", &id)?
        .map(|s| s.value);
    let timestamp = chrono::Utc::now().timestamp();
    let reconciliation = engine
        .workspace
        .db
        .get::<Value>("approval_reconciliations", &id)?
        .map(|s| s.value);
    let permission = engine.check_approval_decision(&approval.value, true, timestamp);
    let can_deny = engine
        .check_approval_decision(&approval.value, false, timestamp)
        .is_ok();
    Ok(Json(
        json!({"approval":approval,"connection":connection,"reconciliation":reconciliation,"canApprove":permission.is_ok(),"canDeny":can_deny,
        "unavailableReason":permission.err().map(|e| polymas_core::process::redact(&e.to_string()))}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReconcileInput {
    expected_revision: i64,
    outcome: ApprovalState,
    note: String,
}
async fn reconcile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ReconcileInput>,
) -> Result<Json<Approval>, ApiError> {
    Ok(Json(ApprovalLedger::reconcile_unknown(
        &mut state.engine.lock().await.workspace.db,
        &id,
        input.expected_revision,
        input.outcome,
        &input.note,
    )?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Decision {
    expected_revision: i64,
    fingerprint: String,
    allow: bool,
}
async fn decide(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Decision>,
) -> Result<Json<Approval>, ApiError> {
    Ok(Json(state.engine.lock().await.decide_approval(
        &id,
        input.expected_revision,
        &input.fingerprint,
        input.allow,
        chrono::Utc::now().timestamp(),
    )?))
}
