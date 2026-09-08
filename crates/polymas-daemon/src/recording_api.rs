use crate::{ApiError, AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use polymas_core::{
    error::{Error, require},
    recording::EventWindow,
    scheduler::{Run, Task},
    workspace::HistoryEntry,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/runs/{id}/recording", get(manifest))
        .route("/api/runs/{id}/recording/events", get(events))
}
async fn manifest(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    let db = &engine.workspace.db;
    let run = db
        .get::<Run>("runs", &id)?
        .ok_or_else(|| Error::NotFound("Запуск не найден".into()))?
        .value;
    let config = db
        .get::<HistoryEntry>("configuration_history", &run.config_revision.to_string())?
        .ok_or_else(|| Error::Storage("Снимок запуска недоступен".into()))?
        .value
        .configuration;
    let mut tasks = vec![];
    for task_id in &run.task_ids {
        let task = db
            .get::<Task>("tasks", task_id)?
            .ok_or_else(|| Error::Storage("Задача записи недоступна".into()))?
            .value;
        // Only immutable descriptors; current state/result must not leak into past frames.
        tasks.push(json!({"id":task.id,"assignee":task.assignee,"createdAt":task.created_at,"dependencies":task.dependencies,"roleId":task.role.id,"roleVersion":task.role.version}));
    }
    let roles: Vec<_> = config
        .registry
        .roles
        .iter()
        .map(|role| json!({"id":role.id,"version":role.version,"name":role.draft.name}))
        .collect();
    let value = json!({"schemaVersion":1,"runId":run.id,"title":run.title,"createdAt":run.created_at,"configRevision":run.config_revision,"workspaceName":config.name,"agents":config.registry.agents,"roles":roles,"architecture":config.architecture,"tasks":tasks,"head":db.last_sequence()?});
    require(
        serde_json::to_vec(&value)?.len() <= 8 * 1024 * 1024,
        "Снимок записи превышает 8 МиБ",
    )?;
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WindowQuery {
    #[serde(default)]
    after: i64,
    through: i64,
    limit: Option<usize>,
}
async fn events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Result<Json<EventWindow>, ApiError> {
    let engine = state.engine.lock().await;
    let db = &engine.workspace.db;
    if db.get::<Run>("runs", &id)?.is_none() {
        return Err(Error::NotFound("Запуск не найден".into()).into());
    }
    Ok(Json(db.recorded_events(
        &id,
        query.after,
        query.through,
        query.limit.unwrap_or(500),
    )?))
}
