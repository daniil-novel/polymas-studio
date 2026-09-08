use super::*;
use crate::policy::{Approval, ApprovalLedger, Operation, authorize_tool};
use crate::registry::AgentStatus;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
mod lifecycle;

fn resource_hash(connection: &crate::connections::Connection, namespace: &str) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(connection, namespace))?)
    ))
}

/// Caller supplies the active task, never the future epoch or resource digest.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationRequest {
    pub task_id: String,
    pub origin_epoch: u64,
    pub connection_id: String,
    pub namespace: String,
    pub tool: String,
    pub arguments: Value,
}

impl Engine {
    /// Called under the application mutex, with no network I/O. Reservation,
    /// approval and retry receipt either all commit or none are observable.
    pub fn request_approval(
        &mut self,
        key: &str,
        request: OperationRequest,
        timestamp: i64,
        ttl_seconds: i64,
    ) -> Result<Approval> {
        let task = self
            .scheduler
            .verify_lease(&request.task_id, request.origin_epoch)?;
        let live = &self.workspace.config().value;
        require(
            live.registry.agent(&task.assignee)?.status != AgentStatus::Archived,
            "Экземпляр недоступен",
        )?;
        let connection = self.workspace.connection(&request.connection_id)?;
        let snapshot = self.bindings_for_run(&task.run_id)?;
        let auth = authorize_tool(
            &task.assignee,
            &task.role.id,
            &connection.value,
            &request.tool,
            &live.bindings,
            &snapshot,
        )?;
        require(
            auth.approval_required && auth.namespaces.contains(&request.namespace),
            "Запрос разрешения требует доступного write tool и конкретного namespace",
        )?;
        let mut candidate = self.scheduler.clone();
        let ticket =
            candidate.reserve_approval_continuation(&request.task_id, request.origin_epoch)?;
        // The complete server-owned connection description includes its target,
        // configured environment, credential references and tool restrictions.
        let resource_scope_hash = resource_hash(&connection.value, &request.namespace)?;
        let operation = Operation {
            agent_id: task.assignee.clone(),
            task_id: request.task_id,
            run_id: task.run_id.clone(),
            origin_epoch: request.origin_epoch,
            epoch: ticket.epoch,
            connection_id: request.connection_id,
            connection_revision: connection.revision,
            namespace: request.namespace,
            resource_scope_hash,
            tool: request.tool,
            arguments: request.arguments,
        };
        let approval = self.workspace.db.transaction(|tx| {
            candidate.persist_delta(&self.scheduler, tx)?;
            let approval = ApprovalLedger::request_in(tx, key, operation, timestamp, ttl_seconds)?;
            if tx
                .get::<Value>("approval_resources", &approval.id)?
                .is_none()
            {
                tx.put(
                    "approval_resources",
                    &approval.id,
                    0,
                    &json!({"name":connection.value.name,"transport":connection.value.transport}),
                )?;
            }
            Ok(approval)
        })?;
        self.scheduler = candidate;
        Ok(approval)
    }
}
