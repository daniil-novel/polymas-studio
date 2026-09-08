//! Owner-only knowledge editing. Agent tools use a separate scoped gateway.
use crate::{ApiError, AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use polymas_core::{
    error::Error,
    knowledge::{Claim, Knowledge, KnowledgeCommand, KnowledgeQuery, KnowledgeView, Source},
};
use serde::Deserialize;
use serde_json::{Value, json};
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/knowledge/command", post(command))
        .route("/api/knowledge/{view}", get(page))
        .route("/api/knowledge/claims/{id}", get(detail))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    key: String,
    command: KnowledgeCommand,
}
async fn command(
    State(state): State<AppState>,
    Json(input): Json<Command>,
) -> Result<Json<Value>, ApiError> {
    let mut engine = state.engine.lock().await;
    if let KnowledgeCommand::AddSource { draft } = &input.command
        && let Some(id) = &draft.connection_id
    {
        engine.workspace.connection(id)?;
    }
    let result = Knowledge::execute(
        &mut engine.workspace.db,
        "human:owner",
        &input.key,
        input.command,
        chrono::Utc::now().timestamp(),
    )?;
    Ok(Json(json!({"result":result})))
}
async fn page(
    State(state): State<AppState>,
    Path(view): Path<String>,
    Query(query): Query<KnowledgeQuery>,
) -> Result<Json<Value>, ApiError> {
    let view = match view.as_str() {
        "sources" => KnowledgeView::Sources,
        "claims" => KnowledgeView::Claims,
        "links" => KnowledgeView::Links,
        _ => return Err(Error::NotFound("Неизвестный раздел знаний".into()).into()),
    };
    let engine = state.engine.lock().await;
    Ok(Json(serde_json::to_value(Knowledge::page(
        &engine.workspace.db,
        view,
        &query,
        chrono::Utc::now().timestamp(),
    )?)?))
}
async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    let claim = engine
        .workspace
        .db
        .get::<Claim>("claims", &id)?
        .ok_or_else(|| Error::NotFound("Утверждение не найдено".into()))?;
    let source = engine
        .workspace
        .db
        .get::<Source>("sources", &claim.value.source_id)?;
    let now = chrono::Utc::now().timestamp();
    Ok(Json(
        json!({"claim":claim,"source":source,"expired":claim.value.expires_at<=now,"observedAt":now}),
    ))
}
