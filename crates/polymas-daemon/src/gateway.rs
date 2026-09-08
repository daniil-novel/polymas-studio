//! Attempt capabilities are separate from the desktop credential and never persisted.
use crate::{ApiError, AppState};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use polymas_core::{
    engine::Engine,
    error::{Error, Result},
    scheduler::TaskState,
    secrets::Secret,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub agent_id: String,
    pub task_id: String,
    pub run_id: String,
    pub epoch: u64,
    pub expires_at: i64,
    #[serde(skip)]
    pub call_limit: Arc<tokio::sync::Semaphore>,
}
#[derive(Clone, Default)]
pub struct Scopes(Arc<Mutex<BTreeMap<String, Scope>>>);
#[derive(Clone)]
pub struct Launch {
    pub port: u16,
    pub executable: std::path::PathBuf,
}
/// Dropping a turn (including cancellation/unwind) removes its capability.
pub struct Grant {
    pub token: Secret,
    key: String,
    scopes: Scopes,
}
impl Drop for Grant {
    fn drop(&mut self) {
        if let Ok(mut scopes) = self.scopes.0.lock() {
            scopes.remove(&self.key);
        }
    }
}
fn digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
pub async fn issue(state: &AppState, task_id: &str) -> Result<Grant> {
    let engine = state.engine.lock().await;
    let task = engine.scheduler.task(task_id)?;
    let scope = Scope {
        agent_id: task.assignee.clone(),
        task_id: task.id.clone(),
        run_id: task.run_id.clone(),
        epoch: task
            .lease
            .as_ref()
            .ok_or_else(|| Error::Denied("Нет активной попытки".into()))?
            .epoch,
        expires_at: chrono::Utc::now().timestamp() + task.role.draft.timeout_seconds as i64 + 30,
        call_limit: Arc::new(tokio::sync::Semaphore::new(1)),
    };
    validate(&engine, &scope)?;
    let token = Secret::new(format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    ))?;
    let key = digest(token.expose());
    let mut scopes = state
        .gateway_scopes
        .0
        .lock()
        .map_err(|_| Error::Runtime("Gateway registry unavailable".into()))?;
    scopes.retain(|_, s| s.expires_at > chrono::Utc::now().timestamp());
    if scopes.len() >= 5000 || scopes.values().any(|s| s.task_id == task_id) {
        return Err(Error::Conflict(
            "Для попытки уже выдан доступ или исчерпана ёмкость".into(),
        ));
    }
    scopes.insert(key.clone(), scope);
    Ok(Grant {
        token,
        key,
        scopes: state.gateway_scopes.clone(),
    })
}
pub fn validate(engine: &Engine, scope: &Scope) -> Result<()> {
    let task = engine.scheduler.task(&scope.task_id)?;
    if task.state != TaskState::Running
        || task.assignee != scope.agent_id
        || task.run_id != scope.run_id
        || task.lease.as_ref().is_none_or(|l| l.epoch != scope.epoch)
        || chrono::Utc::now().timestamp() >= scope.expires_at
    {
        return Err(Error::Denied(
            "Доступ этой попытки завершён или отозван".into(),
        ));
    }
    Ok(())
}
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/gateway/context", get(context))
        .route("/gateway/tools", get(crate::gateway_tools::list))
        .route("/gateway/call", post(crate::gateway_tools::call))
        .layer(DefaultBodyLimit::max(128 * 1024))
        .route_layer(middleware::from_fn_with_state(state, authorize))
}
async fn authorize(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let scope = if token.len() == 64 && !request.headers().contains_key(header::ORIGIN) {
        state
            .gateway_scopes
            .0
            .lock()
            .ok()
            .and_then(|scopes| scopes.get(&digest(token)).cloned())
    } else {
        None
    };
    let Some(scope) = scope else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"code":"unauthorized","message":"Требуется credential активной попытки"})),
        )
            .into_response();
    };
    if let Err(error) = validate(&*state.engine.lock().await, &scope) {
        return ApiError(error).into_response();
    }
    request.extensions_mut().insert(scope);
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        header::HeaderValue::from_static("nosniff"),
    );
    response
}
async fn context(Extension(scope): Extension<Scope>) -> Json<Scope> {
    Json(scope)
}
