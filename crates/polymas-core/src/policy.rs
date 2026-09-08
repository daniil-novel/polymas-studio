use crate::error::{Error, Result, require};
use crate::registry::id;
use crate::storage::{EventInput, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The exact operation approved by a person. A new lease or connection revision
/// invalidates an old approval, even if its arguments happen to be identical.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub agent_id: String,
    pub task_id: String,
    pub epoch: u64,
    pub connection_id: String,
    pub connection_revision: i64,
    pub tool: String,
    pub arguments: Value,
}
impl Operation {
    pub fn fingerprint(&self) -> Result<String> {
        require(
            self.epoch > 0 && self.connection_revision > 0,
            "Операция не привязана к запуску",
        )?;
        for value in [
            &self.agent_id,
            &self.task_id,
            &self.connection_id,
            &self.tool,
        ] {
            require(
                !value.is_empty() && value.len() <= 200,
                "Некорректный идентификатор операции",
            )?;
        }
        require(self.arguments.is_object(), "Аргументы должны быть объектом")?;
        let bytes = serde_json::to_vec(&canonical(serde_json::to_value(self)?))?;
        require(bytes.len() <= 65536, "Операция превышает 64 КиБ")?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
}
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: std::collections::BTreeMap<_, _> =
                map.into_iter().map(|(k, v)| (k, canonical(v))).collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        other => other,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Executing,
    Succeeded,
    Failed,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: String,
    pub operation: Operation,
    pub fingerprint: String,
    pub state: ApprovalState,
    pub created_at: i64,
    pub expires_at: i64,
    pub decided_at: Option<i64>,
}
pub struct ApprovalLedger;
impl ApprovalLedger {
    pub fn request(
        db: &mut Store,
        operation: Operation,
        timestamp: i64,
        ttl_seconds: i64,
    ) -> Result<Approval> {
        require(
            (1..=86400).contains(&ttl_seconds),
            "Срок разрешения: 1–86400 секунд",
        )?;
        let approval = Approval {
            id: id("approval"),
            fingerprint: operation.fingerprint()?,
            operation,
            state: ApprovalState::Pending,
            created_at: timestamp,
            expires_at: timestamp
                .checked_add(ttl_seconds)
                .ok_or_else(|| Error::Validation("Некорректное время".into()))?,
            decided_at: None,
        };
        db.transaction(|tx| {
            tx.put("approvals", &approval.id, 0, &approval)?;
            tx.emit(EventInput::new("approval.requested", json!({"approvalId": approval.id, "taskId": approval.operation.task_id, "tool": approval.operation.tool})).agent(&approval.operation.agent_id))?;
            Ok(approval)
        })
    }
    pub fn decide(db: &mut Store, id: &str, allow: bool, timestamp: i64) -> Result<Approval> {
        db.transaction(|tx| {
            let mut stored = tx
                .get::<Approval>("approvals", id)?
                .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
            require(
                stored.value.state == ApprovalState::Pending && timestamp < stored.value.expires_at,
                "Разрешение уже решено или истекло",
            )?;
            stored.value.state = if allow {
                ApprovalState::Approved
            } else {
                ApprovalState::Denied
            };
            stored.value.decided_at = Some(timestamp);
            tx.put("approvals", id, stored.revision, &stored.value)?;
            tx.emit(
                EventInput::new(
                    "approval.decided",
                    json!({"approvalId":id,"state":stored.value.state}),
                )
                .agent(&stored.value.operation.agent_id),
            )?;
            Ok(stored.value)
        })
    }
    /// Atomically reserve execution before contacting an external service.
    /// A crash after this point is unknown, never permission to execute again.
    pub fn consume(
        db: &mut Store,
        id: &str,
        operation: &Operation,
        timestamp: i64,
    ) -> Result<Approval> {
        let fingerprint = operation.fingerprint()?;
        db.transaction(|tx| {
            let mut stored = tx
                .get::<Approval>("approvals", id)?
                .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
            if stored.value.state != ApprovalState::Approved
                || timestamp >= stored.value.expires_at
                || stored.value.fingerprint != fingerprint
            {
                return Err(Error::Denied(
                    "Нет действующего разрешения для этой операции".into(),
                ));
            }
            stored.value.state = ApprovalState::Executing;
            tx.put("approvals", id, stored.revision, &stored.value)?;
            tx.emit(
                EventInput::new(
                    "approval.execution_started",
                    json!({"approvalId":id,"fingerprint":fingerprint}),
                )
                .agent(&operation.agent_id),
            )?;
            Ok(stored.value)
        })
    }
    pub fn complete(db: &mut Store, id: &str, state: ApprovalState) -> Result<Approval> {
        require(
            matches!(
                state,
                ApprovalState::Succeeded | ApprovalState::Failed | ApprovalState::Unknown
            ),
            "Некорректный исход операции",
        )?;
        db.transaction(|tx| {
            let mut stored = tx
                .get::<Approval>("approvals", id)?
                .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
            require(
                stored.value.state == ApprovalState::Executing,
                "Операция не выполняется",
            )?;
            stored.value.state = state;
            tx.put("approvals", id, stored.revision, &stored.value)?;
            tx.emit(
                EventInput::new(
                    "approval.execution_finished",
                    json!({"approvalId":id,"state":state}),
                )
                .agent(&stored.value.operation.agent_id),
            )?;
            Ok(stored.value)
        })
    }
}
