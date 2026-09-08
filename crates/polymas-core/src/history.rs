//! Bounded owner history projections. Prompts, results and architecture never enter list rows.
use crate::{
    error::{Error, Result, require},
    scheduler::{CompletionRule, TaskState},
    storage::Store,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    All,
    Open,
    Accepted,
    Stopped,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct RunQuery {
    pub cursor: Option<String>,
    pub query: String,
    pub status: RunStatus,
    pub limit: Option<usize>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct RunTaskQuery {
    pub cursor: Option<String>,
    pub state: Option<TaskState>,
    pub limit: Option<usize>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub config_revision: i64,
    pub accepted: bool,
    pub stopped: bool,
    pub task_count: usize,
    pub completion: CompletionRule,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTaskSummary {
    pub id: String,
    pub run_id: String,
    pub assignee: String,
    pub state: TaskState,
    pub attempt: u32,
    pub created_at: String,
    pub completed_at: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    scope: String,
    time: String,
    id: String,
}
fn scope(value: impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}
fn page_input(
    limit: Option<usize>,
    cursor: Option<&str>,
    scope: &str,
) -> Result<(usize, Option<Cursor>)> {
    let limit = limit.unwrap_or(30);
    require(
        (1..=100).contains(&limit),
        "Размер страницы должен быть от 1 до 100",
    )?;
    let cursor = cursor
        .map(|value| -> Result<Cursor> {
            require(value.len() <= 1024, "Слишком длинный курсор истории")?;
            let cursor: Cursor = serde_json::from_str(value)
                .map_err(|_| Error::Validation("Некорректный курсор истории".into()))?;
            require(
                cursor.scope == scope
                    && !cursor.id.is_empty()
                    && cursor.id.len() <= 200
                    && cursor.time.len() <= 64
                    && chrono::DateTime::parse_from_rfc3339(&cursor.time).is_ok(),
                "Курсор не соответствует фильтру истории",
            )?;
            Ok(cursor)
        })
        .transpose()?;
    Ok((limit, cursor))
}
fn page<T>(
    mut items: Vec<T>,
    limit: usize,
    scope: String,
    key: impl Fn(&T) -> (&str, &str),
) -> Result<HistoryPage<T>> {
    let more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = if more {
        items
            .last()
            .map(|last| {
                let (time, id) = key(last);
                serde_json::to_string(&Cursor {
                    scope,
                    time: time.into(),
                    id: id.into(),
                })
            })
            .transpose()?
    } else {
        None
    };
    Ok(HistoryPage { items, next_cursor })
}

// SQLite booleans must be embedded as JSON booleans, not integer 0/1.
const RUN_PROJECTION: &str = "json_object('id',id,'title',json_extract(document,'$.title'),
    'createdAt',json_extract(document,'$.createdAt'),'configRevision',json_extract(document,'$.configRevision'),
    'accepted',json(CASE WHEN json_extract(document,'$.accepted') THEN 'true' ELSE 'false' END),
    'stopped',json(CASE WHEN json_extract(document,'$.stopped') THEN 'true' ELSE 'false' END),
    'taskCount',json_array_length(document,'$.taskIds'),
    'completion',json_extract(document,'$.input.completion'))";

impl Store {
    pub fn run_summary(&self, id: &str) -> Result<RunSummary> {
        let document: Option<String> = self
            .connection()
            .query_row(
                &format!("SELECT {RUN_PROJECTION} FROM entities WHERE collection='runs' AND id=?1"),
                [id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(&document.ok_or_else(|| {
            Error::NotFound("Запуск не найден".into())
        })?)?)
    }

    pub fn run_page(&self, query: &RunQuery) -> Result<HistoryPage<RunSummary>> {
        require(
            query.query.chars().count() <= 200,
            "Поиск ограничен 200 символами",
        )?;
        let search = query.query.to_lowercase();
        let scope = scope(("runs-v1", &search, query.status))?;
        let (limit, cursor) = page_input(query.limit, query.cursor.as_deref(), &scope)?;
        let status = match query.status {
            RunStatus::All => "1",
            RunStatus::Open => {
                "NOT json_extract(document,'$.accepted') AND NOT json_extract(document,'$.stopped')"
            }
            RunStatus::Accepted => "json_extract(document,'$.accepted')",
            RunStatus::Stopped => {
                "json_extract(document,'$.stopped') AND NOT json_extract(document,'$.accepted')"
            }
        };
        // Only static SQL fragments are interpolated; all user data are bound parameters.
        let cursor_clause = if cursor.is_some() {
            "(json_extract(document,'$.createdAt'),id)<(?2,?3)"
        } else {
            "?2 IS NULL AND ?3 IS NULL"
        };
        let mut statement = self.connection().prepare(&format!(
            "SELECT {RUN_PROJECTION} FROM entities WHERE collection='runs' AND ({status})
            AND (?1='' OR instr(polymas_lower(json_extract(document,'$.title')),?1)>0 OR instr(polymas_lower(id),?1)>0)
            AND {cursor_clause} ORDER BY json_extract(document,'$.createdAt') DESC,id DESC LIMIT ?4"))?;
        let rows = statement.query_map(
            params![
                search,
                cursor.as_ref().map(|c| &c.time),
                cursor.as_ref().map(|c| &c.id),
                (limit + 1) as i64
            ],
            |row| row.get::<_, String>(0),
        )?;
        let items = rows
            .map(|row| Ok(serde_json::from_str::<RunSummary>(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        page(items, limit, scope, |r| (&r.created_at, &r.id))
    }

    pub fn run_task_page(
        &self,
        run_id: &str,
        query: &RunTaskQuery,
    ) -> Result<HistoryPage<RunTaskSummary>> {
        require(
            !run_id.is_empty() && run_id.len() <= 200,
            "Некорректный ID запуска",
        )?;
        let scope = scope(("run-tasks-v1", run_id, query.state))?;
        let (limit, cursor) = page_input(query.limit, query.cursor.as_deref(), &scope)?;
        let exists = self
            .connection()
            .query_row(
                "SELECT 1 FROM entities WHERE collection='runs' AND id=?1",
                [run_id],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        if !exists {
            return Err(Error::NotFound("Запуск не найден".into()));
        }
        let state = query.state.map(serde_json::to_value).transpose()?;
        let state_clause = if state.is_some() {
            "json_extract(document,'$.state')=?2"
        } else {
            "?2 IS NULL"
        };
        let cursor_clause = if cursor.is_some() {
            "(json_extract(document,'$.createdAt'),id)<(?3,?4)"
        } else {
            "?3 IS NULL AND ?4 IS NULL"
        };
        let mut statement = self.connection().prepare(&format!(
            "SELECT json_object('id',id,'runId',json_extract(document,'$.runId'),
            'assignee',json_extract(document,'$.assignee'),'state',json_extract(document,'$.state'),
            'attempt',json_extract(document,'$.attempt'),'createdAt',json_extract(document,'$.createdAt'),
            'completedAt',json_extract(document,'$.completedAt')) FROM entities
            WHERE collection='tasks' AND json_extract(document,'$.runId')=?1 AND {state_clause}
            AND {cursor_clause} ORDER BY json_extract(document,'$.createdAt') DESC,id DESC LIMIT ?5"))?;
        let rows = statement.query_map(
            params![
                run_id,
                state.as_ref().and_then(|v| v.as_str()),
                cursor.as_ref().map(|c| &c.time),
                cursor.as_ref().map(|c| &c.id),
                (limit + 1) as i64
            ],
            |row| row.get::<_, String>(0),
        )?;
        let items = rows
            .map(|row| Ok(serde_json::from_str::<RunTaskSummary>(&row?)?))
            .collect::<Result<Vec<_>>>()?;
        page(items, limit, scope, |t| (&t.created_at, &t.id))
    }
}
