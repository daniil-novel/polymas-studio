use super::*;
use crate::storage::WriteTransaction;

#[derive(Serialize, Deserialize)]
struct AttemptRequest {
    approval_id: String,
    request_key_hash: String,
    fingerprint: String,
}
fn attempt_key(task_id: &str, epoch: u64) -> Result<String> {
    require(
        !task_id.is_empty() && task_id.len() <= 200 && epoch > 0,
        "Некорректная попытка разрешения",
    )?;
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(task_id, epoch))?)
    ))
}
pub(super) fn create(
    tx: &WriteTransaction<'_>,
    operation: Operation,
    timestamp: i64,
    ttl_seconds: i64,
) -> Result<Approval> {
    require(
        timestamp >= 0 && (1..=86400).contains(&ttl_seconds),
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
    tx.put("approvals", &approval.id, 0, &approval)?;
    tx.emit(EventInput::new("approval.requested", json!({"approvalId": approval.id, "taskId": approval.operation.task_id, "tool": approval.operation.tool})).agent(&approval.operation.agent_id).run(&approval.operation.run_id))?;
    Ok(approval)
}
impl ApprovalLedger {
    /// One frozen request per model attempt. Retrying a lost MCP response returns
    /// the original decision/expiry; another operation requires another turn.
    pub fn request_for_attempt(
        db: &mut Store,
        request_key: &str,
        operation: Operation,
        timestamp: i64,
        ttl_seconds: i64,
    ) -> Result<Approval> {
        db.transaction(|tx| Self::request_in(tx, request_key, operation, timestamp, ttl_seconds))
    }
    pub(crate) fn request_in(
        tx: &WriteTransaction<'_>,
        request_key: &str,
        operation: Operation,
        timestamp: i64,
        ttl_seconds: i64,
    ) -> Result<Approval> {
        require(
            !request_key.trim().is_empty() && request_key.len() <= 200,
            "Укажите ключ запроса разрешения",
        )?;
        let key = attempt_key(&operation.task_id, operation.origin_epoch)?;
        let fingerprint = operation.fingerprint()?;
        let request_key_hash = format!("{:x}", Sha256::digest(request_key.as_bytes()));
        if let Some(receipt) = tx.get::<AttemptRequest>("approval_attempts", &key)? {
            if receipt.value.request_key_hash != request_key_hash
                || receipt.value.fingerprint != fingerprint
            {
                return Err(Error::Conflict(
                    "В этой попытке уже запрошена другая операция; завершите текущий turn".into(),
                ));
            }
            let approval = tx
                .get::<Approval>("approvals", &receipt.value.approval_id)?
                .ok_or_else(|| Error::Storage("Разрешение попытки отсутствует".into()))?
                .value;
            require(
                approval.fingerprint == fingerprint,
                "Разрешение попытки изменилось",
            )?;
            return Ok(approval);
        }
        let approval = create(tx, operation, timestamp, ttl_seconds)?;
        tx.put(
            "approval_attempts",
            &key,
            0,
            &AttemptRequest {
                approval_id: approval.id.clone(),
                request_key_hash,
                fingerprint,
            },
        )?;
        Ok(approval)
    }
    pub fn for_attempt(db: &Store, task_id: &str, epoch: u64) -> Result<Option<Approval>> {
        let key = attempt_key(task_id, epoch)?;
        let Some(receipt) = db.get::<AttemptRequest>("approval_attempts", &key)? else {
            return Ok(None);
        };
        let approval = db
            .get::<Approval>("approvals", &receipt.value.approval_id)?
            .ok_or_else(|| Error::Storage("Разрешение попытки отсутствует".into()))?
            .value;
        require(
            approval.operation.task_id == task_id
                && approval.operation.origin_epoch == epoch
                && approval.fingerprint == receipt.value.fingerprint,
            "Разрешение относится к другой попытке",
        )?;
        Ok(Some(approval))
    }
}
