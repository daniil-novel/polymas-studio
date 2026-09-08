use crate::{ApiError, AppState};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use polymas_core::{
    connections::{Connection, Health},
    engine::Engine,
    error::{Error, Result, require},
    mcp_client::probe_connection,
    registry::now,
    secrets::{OsVault, Secret},
    storage::{EventInput, Stored},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/connections/save", post(save))
        .route("/api/connections/{id}/probe", post(probe))
        .route("/api/connections/{id}/toggle", post(toggle))
        .route("/api/connections/{id}/tools", post(tools))
        .route("/api/secrets/save", post(save_secret))
        .route("/api/secrets/remove", post(remove_secret))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveConnection {
    revision: i64,
    connection: Connection,
}
async fn save(
    State(state): State<AppState>,
    Json(input): Json<SaveConnection>,
) -> std::result::Result<Json<Stored<Connection>>, ApiError> {
    Ok(Json(
        state
            .engine
            .lock()
            .await
            .workspace
            .save_connection(input.revision, input.connection)?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: i64,
}
fn ensure_revision(actual: i64, expected: i64) -> Result<()> {
    if actual != expected {
        return Err(Error::Conflict(
            "Подключение изменилось; загрузите текущую версию".into(),
        ));
    }
    Ok(())
}
pub fn persist_connection(
    engine: &mut Engine,
    revision: i64,
    connection: Connection,
    kind: &str,
) -> Result<Stored<Connection>> {
    let next = engine.workspace.db.transaction(|tx| {
        let next = tx.put("connections", &connection.id, revision, &connection)?;
        tx.emit(EventInput::new(kind, json!({"connectionId":connection.id,"revision":next,"health":connection.health,"enabled":connection.enabled})))?;
        Ok(next)
    })?;
    Ok(Stored {
        revision: next,
        value: connection,
    })
}
pub struct Credentials {
    pub bearer: Option<Secret>,
    pub environment: BTreeMap<String, Secret>,
}
impl Credentials {
    pub fn diagnostic(&self, text: &str) -> String {
        let mut result = polymas_core::process::redact(text);
        for secret in self.environment.values().chain(self.bearer.iter()) {
            result = result.replace(secret.expose(), "[REDACTED]");
        }
        result.chars().take(4000).collect()
    }
}
pub async fn credentials(connection: &Connection, workspace_id: String) -> Result<Credentials> {
    let connection = connection.clone();
    tokio::task::spawn_blocking(move || {
        let vault = OsVault::new(&workspace_id)?;
        let bearer = connection
            .auth_secret_ref
            .as_deref()
            .map(|reference| vault.load(reference))
            .transpose()?;
        let environment = connection
            .secret_environment
            .iter()
            .map(|(name, reference)| Ok((name.clone(), vault.load(reference)?)))
            .collect::<Result<_>>()?;
        Ok(Credentials {
            bearer,
            environment,
        })
    })
    .await
    .map_err(|_| Error::Runtime("Не удалось открыть системное хранилище секретов".into()))?
}
async fn probe(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Revision>,
) -> std::result::Result<Json<Value>, ApiError> {
    // Never hold the engine lock across a network request or a credential dialog.
    let (stored, workspace_id) = {
        let engine = state.engine.lock().await;
        (
            engine.workspace.connection(&id)?,
            engine.workspace.config().value.id.clone(),
        )
    };
    ensure_revision(stored.revision, input.revision)?;
    let mut connection = stored.value;
    let loaded = credentials(&connection, workspace_id).await;
    let result = match &loaded {
        Ok(secrets) => {
            probe_connection(
                &connection,
                secrets.bearer.as_ref().map(|s| s.expose().to_owned()),
                secrets
                    .environment
                    .iter()
                    .map(|(k, v)| (k.clone(), v.expose().to_owned()))
                    .collect(),
            )
            .await
        }
        Err(error) => Err(Error::Denied(error.to_string())),
    };
    connection.checked_at = Some(now());
    let (protocol, server_name) = match result {
        Ok(probe) => {
            connection.health = Health::Ready;
            connection.last_error = None;
            connection.tools = probe.tools;
            connection
                .tool_allowlist
                .retain(|name| connection.tools.iter().any(|t| &t.name == name));
            let name = probe.server["serverInfo"]["name"]
                .as_str()
                .or(probe.server["name"].as_str())
                .unwrap_or("MCP server")
                .chars()
                .take(200)
                .collect::<String>();
            (Some(probe.protocol), Some(name))
        }
        Err(error) => {
            connection.health = if connection.enabled {
                Health::Degraded
            } else {
                Health::Error
            };
            connection.last_error = Some(match &loaded {
                Ok(secrets) => secrets.diagnostic(&error.to_string()),
                Err(_) => "Секрет отсутствует или системное хранилище недоступно".into(),
            });
            (None, None)
        }
    };
    let mut engine = state.engine.lock().await;
    // A disable/edit while the probe was in flight wins over the old probe.
    ensure_revision(engine.workspace.connection(&id)?.revision, input.revision)?;
    let stored = persist_connection(&mut engine, input.revision, connection, "mcp.probed")?;
    Ok(Json(
        json!({"connection":stored,"protocol":protocol,"serverName":server_name}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Toggle {
    revision: i64,
    enabled: bool,
}
async fn toggle(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Toggle>,
) -> std::result::Result<Json<Stored<Connection>>, ApiError> {
    let mut engine = state.engine.lock().await;
    let stored = engine.workspace.connection(&id)?;
    ensure_revision(stored.revision, input.revision)?;
    let mut connection = stored.value;
    connection.set_enabled(input.enabled)?;
    Ok(Json(persist_connection(
        &mut engine,
        input.revision,
        connection,
        "mcp.toggled",
    )?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectTools {
    revision: i64,
    tools: Vec<String>,
}
async fn tools(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SelectTools>,
) -> std::result::Result<Json<Stored<Connection>>, ApiError> {
    let mut engine = state.engine.lock().await;
    let stored = engine.workspace.connection(&id)?;
    ensure_revision(stored.revision, input.revision)?;
    let mut connection = stored.value;
    require(
        input.tools.len() <= 1000
            && input.tools.iter().collect::<BTreeSet<_>>().len() == input.tools.len()
            && input
                .tools
                .iter()
                .all(|name| connection.tools.iter().any(|tool| &tool.name == name)),
        "Выберите уникальные инструменты из проверенного списка",
    )?;
    connection.tool_allowlist = input.tools;
    Ok(Json(persist_connection(
        &mut engine,
        input.revision,
        connection,
        "mcp.tools_selected",
    )?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveSecret {
    value: String,
}
async fn save_secret(
    State(state): State<AppState>,
    Json(input): Json<SaveSecret>,
) -> std::result::Result<Json<Value>, ApiError> {
    let secret = Secret::new(input.value)?;
    let workspace = state
        .engine
        .lock()
        .await
        .workspace
        .config()
        .value
        .id
        .clone();
    let reference = tokio::task::spawn_blocking(move || OsVault::new(&workspace)?.save(&secret))
        .await
        .map_err(|_| Error::Runtime("Не удалось сохранить секрет".into()))??;
    Ok(Json(json!({"reference":reference})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveSecret {
    reference: String,
}
async fn remove_secret(
    State(state): State<AppState>,
    Json(input): Json<RemoveSecret>,
) -> std::result::Result<Json<Value>, ApiError> {
    let engine = state.engine.lock().await;
    let connections = engine
        .workspace
        .db
        .list::<Connection>("connections", 10000)?;
    require(
        !connections.iter().any(|c| {
            c.value.auth_secret_ref.as_ref() == Some(&input.reference)
                || c.value
                    .secret_environment
                    .values()
                    .any(|r| r == &input.reference)
        }),
        "Секрет используется подключением; сначала замените или удалите его ссылку",
    )?;
    // Keep this short local lock so another command cannot bind the secret between
    // the reference check and removal. No network operation occurs here.
    OsVault::new(&engine.workspace.config().value.id)?.remove(&input.reference)?;
    Ok(Json(json!({"ok":true})))
}
