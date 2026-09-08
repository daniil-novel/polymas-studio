use crate::error::{Error, Result, require};
use crate::registry::id;
use crate::storage::{EventInput, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
mod query;
mod recovery;
mod requests;
pub use query::{ApprovalPage, ApprovalQuery};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    Agent { id: String },
    Role { id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub connection_id: String,
    pub subject: Subject,
    pub tools: BTreeSet<String>,
    pub allow_write: bool,
    pub required: bool,
    pub namespace: String,
}
impl Binding {
    pub fn validate(&self) -> Result<()> {
        require(
            !self.id.is_empty() && !self.connection_id.is_empty(),
            "Binding требует идентификаторов",
        )?;
        require(
            !self.namespace.is_empty()
                && self.namespace.len() <= 120
                && self
                    .namespace
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_-./".contains(c)),
            "Некорректный namespace",
        )?;
        require(
            !self.tools.is_empty()
                && self.tools.len() <= 1000
                && self.tools.iter().all(|t| !t.is_empty() && t.len() <= 200),
            "Укажите разрешённые инструменты",
        )
    }
    pub fn matches(&self, agent_id: &str, role_id: &str) -> bool {
        match &self.subject {
            Subject::Agent { id } => id == agent_id,
            Subject::Role { id } => id == role_id,
        }
    }
}
#[derive(Debug)]
pub struct Authorization {
    pub namespaces: BTreeSet<String>,
    pub approval_required: bool,
}

/// Capabilities can shrink during a turn, but only a new turn may acquire new
/// capabilities. Server-side live configuration is consulted on every call.
pub fn authorize_tool(
    agent_id: &str,
    role_id: &str,
    connection: &crate::connections::Connection,
    tool: &str,
    live: &[Binding],
    snapshot: &[Binding],
) -> Result<Authorization> {
    if !connection.allows_tool(tool) {
        return Err(Error::Denied(
            "Подключение или инструмент недоступны".into(),
        ));
    }
    let descriptor = connection
        .tools
        .iter()
        .find(|t| t.name == tool)
        .ok_or_else(|| Error::Denied("Инструмент недоступен".into()))?;
    let scopes = |bindings: &[Binding]| -> BTreeSet<String> {
        bindings
            .iter()
            .filter(|b| {
                b.validate().is_ok()
                    && b.matches(agent_id, role_id)
                    && b.connection_id == connection.id
                    && b.tools.contains(tool)
                    && (descriptor.read_only || b.allow_write)
            })
            .map(|b| b.namespace.clone())
            .collect()
    };
    let current = scopes(live);
    let previous = scopes(snapshot);
    let namespaces: BTreeSet<_> = current.intersection(&previous).cloned().collect();
    if namespaces.is_empty() {
        return Err(Error::Denied(
            "Нет действующего binding для этого агента и инструмента".into(),
        ));
    }
    Ok(Authorization {
        namespaces,
        approval_required: !descriptor.read_only,
    })
}

/// The exact operation approved by a person. A new lease or connection revision
/// invalidates an old approval, even if its arguments happen to be identical.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operation {
    pub agent_id: String,
    pub task_id: String,
    // Old persisted approvals remain readable, but cannot authorize unscoped work.
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub resource_scope_hash: String,
    #[serde(default)]
    pub origin_epoch: u64,
    pub epoch: u64,
    pub connection_id: String,
    pub connection_revision: i64,
    pub tool: String,
    pub arguments: Value,
}
impl Operation {
    pub fn fingerprint(&self) -> Result<String> {
        require(
            self.origin_epoch > 0 && self.epoch > self.origin_epoch && self.connection_revision > 0,
            "Операция не привязана к запуску",
        )?;
        for value in [
            &self.agent_id,
            &self.task_id,
            &self.run_id,
            &self.connection_id,
            &self.tool,
        ] {
            require(
                !value.is_empty() && value.len() <= 200,
                "Некорректный идентификатор операции",
            )?;
        }
        require(
            !self.namespace.is_empty()
                && self.namespace.len() <= 120
                && self
                    .namespace
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_-./".contains(c)),
            "Операция требует конкретного namespace",
        )?;
        require(
            self.resource_scope_hash.len() == 64
                && self
                    .resource_scope_hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Операция требует hash области ресурса",
        )?;
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
    Expired,
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
        db.transaction(|tx| requests::create(tx, operation, timestamp, ttl_seconds))
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
            if allow {
                require(
                    stored.value.operation.fingerprint()? == stored.value.fingerprint,
                    "Операция разрешения изменилась или не привязана к области",
                )?;
            }
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
                .agent(&stored.value.operation.agent_id)
                .run(&stored.value.operation.run_id),
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
                || stored.value.operation.fingerprint()? != fingerprint
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
                .agent(&operation.agent_id)
                .run(&operation.run_id),
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
                .agent(&stored.value.operation.agent_id)
                .run(&stored.value.operation.run_id),
            )?;
            Ok(stored.value)
        })
    }
}
