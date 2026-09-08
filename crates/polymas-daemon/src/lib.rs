//! Local authenticated control plane. The desktop and worker bridge use separate credentials.
pub mod bridge;
pub mod connection_api;
pub mod gateway;
pub mod gateway_messages;
pub mod gateway_tools;
pub mod gateway_wait;
pub mod run_api;
pub mod runtime;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use polymas_core::{
    engine::Engine,
    error::{Error, Result, require},
    secrets::Secret,
    workspace::ConfigCommand,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub active: Arc<Mutex<std::collections::BTreeMap<String, tokio::sync::watch::Sender<bool>>>>,
    pub runtime_options: Arc<tokio::sync::RwLock<Option<runtime::RuntimeOptions>>>,
    pub runtime_status: Arc<tokio::sync::RwLock<run_api::RuntimeStatus>>,
    pub shutdown: Arc<tokio::sync::Notify>,
    pub gateway_scopes: gateway::Scopes,
    pub gateway_launch: Arc<tokio::sync::RwLock<Option<gateway::Launch>>>,
    adoption: Arc<std::sync::atomic::AtomicU8>,
    token: Arc<Secret>,
    allowed_origins: Arc<BTreeSet<String>>,
}
impl AppState {
    pub fn new(mut engine: Engine, token: String, allowed_origins: Vec<String>) -> Result<Self> {
        require(
            token.len() >= 32 && token.len() <= 256,
            "Control token должен содержать не менее 32 символов",
        )?;
        polymas_core::sessions::SessionRegistry::migrate(&mut engine.workspace.db)?;
        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            active: Arc::new(Mutex::new(Default::default())),
            runtime_options: Arc::new(tokio::sync::RwLock::new(None)),
            runtime_status: Arc::new(tokio::sync::RwLock::new(Default::default())),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            gateway_scopes: Default::default(),
            gateway_launch: Default::default(),
            adoption: Arc::new(std::sync::atomic::AtomicU8::new(1)),
            token: Arc::new(Secret::new(token)?),
            allowed_origins: Arc::new(allowed_origins.into_iter().collect()),
        })
    }
    pub fn require_adoption(&self) {
        self.adoption.store(0, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn adopt(&self) -> Result<()> {
        use std::sync::atomic::Ordering::SeqCst;
        match self.adoption.compare_exchange(0, 1, SeqCst, SeqCst) {
            Ok(_) | Err(1) => Ok(()),
            _ => Err(Error::Denied(
                "Срок подключения к запущенной службе истёк".into(),
            )),
        }
    }
    pub fn expire_adoption(&self) -> bool {
        use std::sync::atomic::Ordering::SeqCst;
        self.adoption.compare_exchange(0, 2, SeqCst, SeqCst).is_ok()
    }
    pub fn is_adopted(&self) -> bool {
        self.adoption.load(std::sync::atomic::Ordering::SeqCst) == 1
    }
    pub async fn configure_gateway(&self, port: u16, executable: std::path::PathBuf) -> Result<()> {
        require(
            port > 0 && executable.is_absolute() && executable.is_file(),
            "Gateway требует порт и абсолютный путь доверенного executable",
        )?;
        *self.gateway_launch.write().await = Some(gateway::Launch { port, executable });
        Ok(())
    }
}
pub struct ApiError(pub Error);
impl From<Error> for ApiError {
    fn from(value: Error) -> Self {
        Self(value)
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            Error::Validation(_) => StatusCode::BAD_REQUEST,
            Error::Conflict(_) => StatusCode::CONFLICT,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Denied(_) => StatusCode::FORBIDDEN,
            Error::Capacity(_) => StatusCode::TOO_MANY_REQUESTS,
            Error::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            Error::Cancelled(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut value = serde_json::to_value(&self.0).unwrap_or(json!({"code":"runtime"}));
        value["message"] = json!(polymas_core::process::redact(&self.0.to_string()));
        (status, Json(value)).into_response()
    }
}
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(run_api::routes())
        .merge(connection_api::routes())
        .route("/api/state", get(snapshot))
        .route("/api/configure", post(configure))
        .route("/api/events", get(events))
        .route("/api/history", get(history))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize))
        .merge(gateway::routes(state.clone()))
        .with_state(state)
}
async fn authorize(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    let origin_allowed = match request.headers().get(header::ORIGIN) {
        None => true,
        Some(value) => value
            .to_str()
            .is_ok_and(|v| state.allowed_origins.contains(v)),
    };
    if !origin_allowed || !bool::from(token.as_bytes().ct_eq(state.token.expose().as_bytes())) {
        return (StatusCode::UNAUTHORIZED,Json(json!({"code":"unauthorized","message":"Требуется локальное авторизованное подключение"}))).into_response();
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        header::HeaderValue::from_static("nosniff"),
    );
    Ok::<_, ApiError>(response).into_response()
}
async fn snapshot(State(state): State<AppState>) -> std::result::Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    let mut value = serde_json::to_value(engine.workspace.snapshot()?)?;
    value["paused"] = json!(engine.scheduler.paused);
    value["runtime"] = serde_json::to_value(state.runtime_status.read().await.clone())?;
    value["agentRuntime"] = serde_json::to_value(
        engine
            .workspace
            .db
            .list::<runtime::AgentRuntime>("agent_runtime", 5000)?,
    )?;
    value["runs"]=json!(engine.scheduler.runs.iter().rev().take(100).map(|r| json!({"id":r.id,"title":r.title,"createdAt":r.created_at,"accepted":r.accepted,"stopped":r.stopped,"taskIds":r.task_ids,"configRevision":r.config_revision,"completion":r.input.completion})).collect::<Vec<_>>());
    value["taskCount"] = json!(engine.scheduler.tasks.len());
    value["tasks"]=json!(engine.scheduler.tasks.iter().rev().take(1000).map(|t| json!({"id":t.id,"runId":t.run_id,"assignee":t.assignee,"state":t.state,"attempt":t.attempt,"lease":t.lease,"createdAt":t.created_at,"completedAt":t.completed_at,"artifactIds":t.artifact_ids,"error":t.error})).collect::<Vec<_>>());
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigureRequest {
    revision: i64,
    command: ConfigCommand,
}
async fn configure(
    State(state): State<AppState>,
    Json(request): Json<ConfigureRequest>,
) -> std::result::Result<Json<Value>, ApiError> {
    let mut engine = state.engine.lock().await;
    let result = engine.configure(request.revision, request.command)?;
    Ok(Json(
        json!({"result":result,"revision":engine.workspace.config().revision}),
    ))
}
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct EventsQuery {
    after: i64,
    limit: Option<usize>,
    run_id: Option<String>,
}
async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventsQuery>,
) -> std::result::Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    require(query.after >= 0, "Некорректный cursor")?;
    let events = engine.workspace.db.events(
        query.after,
        query.limit.unwrap_or(500),
        query.run_id.as_deref(),
    )?;
    let cursor = events.last().map(|e| e.sequence).unwrap_or(query.after);
    Ok(Json(
        json!({"events":events,"cursor":cursor,"head":engine.workspace.db.last_sequence()?}),
    ))
}
async fn history(State(state): State<AppState>) -> std::result::Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"items":state.engine.lock().await.workspace.history()?}),
    ))
}
