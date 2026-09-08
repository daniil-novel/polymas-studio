use super::*;
use rusqlite::params;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ApprovalQuery {
    pub after: String,
    pub limit: Option<usize>,
    pub run_id: Option<String>,
    pub state: Option<ApprovalState>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPage {
    pub items: Vec<Value>,
    pub next_cursor: Option<String>,
}
impl Store {
    pub fn approval_page(&self, query: &ApprovalQuery) -> Result<ApprovalPage> {
        let limit = query.limit.unwrap_or(30);
        require(
            (1..=100).contains(&limit)
                && query.after.len() <= 200
                && query
                    .run_id
                    .as_ref()
                    .is_none_or(|v| !v.is_empty() && v.len() <= 200),
            "Некорректная страница разрешений",
        )?;
        let state = query.state.map(serde_json::to_value).transpose()?;
        let mut statement = self.connection().prepare("SELECT json_object(
            'id',id,'revision',revision,'state',json_extract(document,'$.state'),
            'createdAt',json_extract(document,'$.createdAt'),'expiresAt',json_extract(document,'$.expiresAt'),
            'agentId',json_extract(document,'$.operation.agentId'),'taskId',json_extract(document,'$.operation.taskId'),
            'runId',json_extract(document,'$.operation.runId'),'connectionId',json_extract(document,'$.operation.connectionId'),
            'tool',json_extract(document,'$.operation.tool'),'namespace',json_extract(document,'$.operation.namespace'))
            FROM entities WHERE collection='approvals' AND id>?1
            AND (?2 IS NULL OR json_extract(document,'$.state')=?2)
            AND (?3 IS NULL OR json_extract(document,'$.operation.runId')=?3)
            ORDER BY id LIMIT ?4")?;
        let rows = statement.query_map(
            params![
                query.after,
                state.as_ref().and_then(Value::as_str),
                query.run_id,
                (limit + 1) as i64
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut items = vec![];
        let mut more = false;
        for row in rows {
            let row = row?;
            if items.len() == limit {
                more = true;
                break;
            }
            items.push(serde_json::from_str::<Value>(&row)?);
        }
        let next_cursor = if more {
            items
                .last()
                .and_then(|v| v["id"].as_str())
                .map(str::to_owned)
        } else {
            None
        };
        Ok(ApprovalPage { items, next_cursor })
    }
}
