use crate::{
    ApiError, AppState,
    runtime::{CodexExecutor, RuntimeOptions},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use polymas_core::{
    codex::RuntimeProbe,
    error::{Error, Result, require},
    history::{HistoryPage, RunQuery, RunSummary, RunTaskQuery, RunTaskSummary},
    scheduler::TaskInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub probe: Option<RuntimeProbe>,
    pub ready: bool,
    pub capacity: usize,
    pub error: Option<String>,
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/runtime", get(status))
        .route("/api/runtime/probe", post(probe))
        .route("/api/runs", post(submit).get(runs))
        .route("/api/runs/{id}", get(run))
        .route("/api/runs/{id}/tasks", get(run_tasks))
        .route("/api/tasks/{id}", get(task))
        .route("/api/tasks/{id}/action", post(task_action))
        .route("/api/runs/{id}/action", post(run_action))
        .route("/api/control", post(control))
}
pub async fn configure_runtime(
    state: &AppState,
    path: Option<PathBuf>,
    capacity: usize,
) -> Result<()> {
    let result = polymas_core::codex::probe_codex(path.as_deref()).await;
    match result {
        Ok(probe) => {
            let ready = probe.compatible && probe.authenticated;
            let options = if ready {
                Some(RuntimeOptions::new(
                    Arc::new(CodexExecutor {
                        executable: probe.path.clone(),
                    }),
                    capacity,
                )?)
            } else {
                None
            };
            *state.runtime_options.write().await = options;
            *state.runtime_status.write().await = RuntimeStatus {
                error: if ready {
                    None
                } else {
                    Some("Требуется совместимая версия Codex и вход через Codex login".into())
                },
                probe: Some(probe),
                ready,
                capacity,
            };
            Ok(())
        }
        Err(error) => {
            *state.runtime_options.write().await = None;
            *state.runtime_status.write().await = RuntimeStatus {
                probe: None,
                ready: false,
                capacity,
                error: Some(error.to_string()),
            };
            Err(error)
        }
    }
}
async fn status(State(state): State<AppState>) -> Json<RuntimeStatus> {
    Json(state.runtime_status.read().await.clone())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProbeInput {
    path: Option<PathBuf>,
}
async fn probe(
    State(state): State<AppState>,
    Json(input): Json<ProbeInput>,
) -> std::result::Result<Json<RuntimeStatus>, ApiError> {
    require(
        state.active.lock().await.is_empty(),
        "Остановите текущие процессы перед сменой Codex",
    )?;
    let capacity = state.runtime_status.read().await.capacity.max(1);
    // A failed probe is a useful diagnostic result rather than a fabricated Ready.
    let _ = configure_runtime(&state, input.path, capacity).await;
    Ok(Json(state.runtime_status.read().await.clone()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitInput {
    key: String,
    input: TaskInput,
}
async fn submit(
    State(state): State<AppState>,
    Json(request): Json<SubmitInput>,
) -> std::result::Result<Json<Value>, ApiError> {
    let runtime = state.runtime_status.read().await.clone();
    if !runtime.ready {
        return Err(Error::Conflict("Codex не готов; проверьте исполнителя и вход".into()).into());
    }
    let mut engine = state.engine.lock().await;
    let occupied = engine
        .scheduler
        .tasks
        .iter()
        .filter(|t| {
            matches!(
                t.state,
                polymas_core::scheduler::TaskState::Running
                    | polymas_core::scheduler::TaskState::Stopping
                    | polymas_core::scheduler::TaskState::Unknown
            )
        })
        .count();
    let run = engine.submit(
        &request.key,
        request.input,
        runtime.capacity.saturating_sub(occupied),
    )?;
    Ok(Json(serde_json::to_value(run)?))
}
async fn task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<Json<Value>, ApiError> {
    Ok(Json(serde_json::to_value(
        state.engine.lock().await.scheduler.task(&id)?,
    )?))
}
async fn runs(
    State(state): State<AppState>,
    Query(query): Query<RunQuery>,
) -> std::result::Result<Json<HistoryPage<RunSummary>>, ApiError> {
    Ok(Json(
        state.engine.lock().await.workspace.db.run_page(&query)?,
    ))
}
async fn run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<Json<RunSummary>, ApiError> {
    Ok(Json(
        state.engine.lock().await.workspace.db.run_summary(&id)?,
    ))
}
async fn run_tasks(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RunTaskQuery>,
) -> std::result::Result<Json<HistoryPage<RunTaskSummary>>, ApiError> {
    Ok(Json(
        state
            .engine
            .lock()
            .await
            .workspace
            .db
            .run_task_page(&id, &query)?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action {
    action: String,
    note: Option<String>,
}
async fn task_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(action): Json<Action>,
) -> std::result::Result<Json<Value>, ApiError> {
    let mut engine = state.engine.lock().await;
    match action.action.as_str() {
        "accept" => engine.accept(&id)?,
        "reject" => engine.reject(&id)?,
        "cancel" => engine.cancel_task(&id)?,
        "retry" | "reconcile" => engine.reconcile(
            &id,
            action.action == "retry",
            action.note.as_deref().unwrap_or_default(),
        )?,
        _ => return Err(Error::Validation("Неизвестное действие".into()).into()),
    }
    drop(engine);
    signal_stopping(&state).await;
    Ok(Json(json!({"ok":true})))
}
async fn run_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(action): Json<Action>,
) -> std::result::Result<Json<Value>, ApiError> {
    let mut engine = state.engine.lock().await;
    match action.action.as_str() {
        "cancel" => engine.cancel_run(&id)?,
        "accept" => engine.accept_run(&id)?,
        _ => return Err(Error::Validation("Неизвестное действие".into()).into()),
    }
    drop(engine);
    signal_stopping(&state).await;
    Ok(Json(json!({"ok":true})))
}
async fn control(
    State(state): State<AppState>,
    Json(action): Json<Action>,
) -> std::result::Result<Json<Value>, ApiError> {
    match action.action.as_str() {
        "pause" => state.engine.lock().await.pause(true)?,
        "adopt" => state.adopt()?,
        "resume" => state.engine.lock().await.pause(false)?,
        "stop_all" => stop_all(&state).await?,
        "shutdown" => {
            stop_all(&state).await?;
            state.shutdown.notify_one();
        }
        _ => return Err(Error::Validation("Неизвестное действие".into()).into()),
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn stop_all(state: &AppState) -> Result<()> {
    let mut engine = state.engine.lock().await;
    engine.pause(true)?;
    let ids: Vec<_> = engine
        .scheduler
        .runs
        .iter()
        .filter(|r| !r.stopped && !r.accepted)
        .map(|r| r.id.clone())
        .collect();
    for id in ids {
        engine.cancel_run(&id)?;
    }
    drop(engine);
    signal_stopping(state).await;
    Ok(())
}
pub async fn signal_stopping(state: &AppState) {
    let ids: Vec<_> = state
        .engine
        .lock()
        .await
        .scheduler
        .tasks
        .iter()
        .filter(|t| t.state == polymas_core::scheduler::TaskState::Stopping)
        .map(|t| t.id.clone())
        .collect();
    let active = state.active.lock().await;
    for id in ids {
        if let Some(cancel) = active.get(&id) {
            cancel.send(true).ok();
        }
    }
}
