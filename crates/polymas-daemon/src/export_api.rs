use crate::{ApiError, AppState};
use axum::{Router, extract::State, http::header, response::IntoResponse, routing::get};
use polymas_core::{
    configuration_export::{MAX_EXPORT_CONNECTIONS, configuration_export},
    workspace::Snapshot,
};

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/exports/configuration", get(configuration))
}

async fn configuration(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let snapshot = {
        let engine = state.engine.lock().await;
        Snapshot {
            config: engine.workspace.config().clone(),
            // One extra record detects overflow; the UI snapshot is only a page.
            connections: engine
                .workspace
                .db
                .list("connections", MAX_EXPORT_CONNECTIONS + 1)?,
            cursor: engine.workspace.db.last_sequence()?,
        }
    };
    let bytes = configuration_export(&snapshot)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=polymas-configuration.json",
            ),
        ],
        bytes,
    ))
}
