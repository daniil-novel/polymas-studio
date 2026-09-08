use super::*;
use crate::connections::Connection;
use crate::policy::ApprovalState;
use crate::storage::Stored;

impl Engine {
    pub fn check_approval_decision(
        &self,
        approval: &Approval,
        allow: bool,
        timestamp: i64,
    ) -> Result<()> {
        require(
            approval.state == ApprovalState::Pending && timestamp < approval.expires_at,
            "Разрешение уже решено или истекло",
        )?;
        self.verify_reservation(approval)?;
        if allow {
            self.operation_connection(&approval.operation)?;
        }
        Ok(())
    }
    fn operation_connection(&self, operation: &Operation) -> Result<Stored<Connection>> {
        let task = self.scheduler.task(&operation.task_id)?;
        require(
            task.assignee == operation.agent_id && task.run_id == operation.run_id,
            "Операция относится к другому экземпляру или запуску",
        )?;
        let live = &self.workspace.config().value;
        require(
            live.registry.agent(&task.assignee)?.status != AgentStatus::Archived,
            "Экземпляр недоступен",
        )?;
        let connection = self.workspace.connection(&operation.connection_id)?;
        require(
            connection.revision == operation.connection_revision
                && resource_hash(&connection.value, &operation.namespace)?
                    == operation.resource_scope_hash,
            "Область подключения изменилась после запроса",
        )?;
        let auth = authorize_tool(
            &task.assignee,
            &task.role.id,
            &connection.value,
            &operation.tool,
            &live.bindings,
            &self.bindings_for_run(&task.run_id)?,
        )?;
        require(
            auth.approval_required && auth.namespaces.contains(&operation.namespace),
            "Право записи отозвано или namespace недоступен",
        )?;
        Ok(connection)
    }
    fn verify_reservation(&self, approval: &Approval) -> Result<()> {
        let op = &approval.operation;
        let task = self.scheduler.task(&op.task_id)?;
        require(
            task.assignee == op.agent_id
                && task.run_id == op.run_id
                && task
                    .continuation
                    .as_ref()
                    .is_some_and(|t| t.origin_epoch == op.origin_epoch && t.epoch == op.epoch)
                && matches!(
                    task.state,
                    TaskState::Running | TaskState::WaitingUser | TaskState::Queued
                )
                && op.fingerprint()? == approval.fingerprint,
            "Продолжение отменено или операция не соответствует резерву",
        )?;
        Ok(())
    }
    pub fn approval_to_park(&self, task_id: &str, epoch: u64) -> Result<Option<Approval>> {
        let task = self.scheduler.verify_lease(task_id, epoch)?;
        require(task.state == TaskState::Running, "Попытка останавливается")?;
        let Some(approval) = ApprovalLedger::for_attempt(&self.workspace.db, task_id, epoch)?
        else {
            return Ok(None);
        };
        self.verify_reservation(&approval)?;
        Ok(Some(approval))
    }
    pub fn park_approval(&mut self, task_id: &str, epoch: u64, summary: String) -> Result<()> {
        let approval = self
            .approval_to_park(task_id, epoch)?
            .ok_or_else(|| Error::NotFound("Запрос разрешения отсутствует".into()))?;
        self.transition(|s| s.park_approval(task_id, epoch, &approval.id, summary))
    }
    pub fn decide_approval(
        &mut self,
        id: &str,
        expected_revision: i64,
        fingerprint: &str,
        allow: bool,
        timestamp: i64,
    ) -> Result<Approval> {
        let stored = self
            .workspace
            .db
            .get::<Approval>("approvals", id)?
            .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
        let desired = if allow {
            ApprovalState::Approved
        } else {
            ApprovalState::Denied
        };
        require(
            stored.value.fingerprint == fingerprint,
            "Показанная операция изменилась",
        )?;
        // A lost owner response can be retried without changing an earlier choice.
        if stored.value.state == desired
            && Some(stored.revision) == expected_revision.checked_add(1)
        {
            return Ok(stored.value);
        }
        require(
            stored.revision == expected_revision,
            "Решение уже изменено; обновите карточку",
        )?;
        self.check_approval_decision(&stored.value, allow, timestamp)?;
        ApprovalLedger::decide(&mut self.workspace.db, id, allow, timestamp)
    }
    pub fn wake_approvals(&mut self, timestamp: i64) -> Result<Vec<String>> {
        let mut ready = vec![];
        for task in self
            .scheduler
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::WaitingUser)
        {
            let wait = task
                .approval_wait
                .as_ref()
                .ok_or_else(|| Error::Storage("Ожидание не содержит approval ID".into()))?;
            let approval = self
                .workspace
                .db
                .get::<Approval>("approvals", &wait.approval_id)?
                .ok_or_else(|| Error::Storage("Разрешение ожидания отсутствует".into()))?
                .value;
            self.verify_reservation(&approval)?;
            require(
                approval.operation.task_id == task.id
                    && approval.operation.origin_epoch == wait.origin_epoch,
                "Ожидание относится к другой операции",
            )?;
            if approval.state != ApprovalState::Pending || timestamp >= approval.expires_at {
                ready.push((task.id.clone(), approval.id));
            }
        }
        if ready.is_empty() {
            return Ok(vec![]);
        }
        let mut candidate = self.scheduler.clone();
        self.workspace.db.transaction(|tx| {
            for (task_id, id) in &ready {
                let approval = ApprovalLedger::expire_in(tx, id, timestamp)?;
                require(matches!(approval.state, ApprovalState::Approved | ApprovalState::Denied | ApprovalState::Expired),
                    "Неожиданный исход операции до её продолжения")?;
                let reason = format!("Решение владельца для операции {id}: {:?}. Проверьте polymas_operation_status. Исполнение разрешено только через polymas_execute_operation с этим approvalId.", approval.state);
                candidate.wake_approval(task_id, id, reason)?;
                tx.emit(EventInput::new("task.approval_woken", json!({"taskId":task_id,"approvalId":id,"state":approval.state}))
                    .agent(&approval.operation.agent_id).run(&approval.operation.run_id))?;
            }
            candidate.persist_delta(&self.scheduler, tx)
        })?;
        self.scheduler = candidate;
        Ok(ready.into_iter().map(|(id, _)| id).collect())
    }
    /// Rechecked before, during and after network I/O by the gateway.
    pub fn verify_operation(&self, operation: &Operation) -> Result<Stored<Connection>> {
        let task = self
            .scheduler
            .verify_lease(&operation.task_id, operation.epoch)?;
        require(task.state == TaskState::Running, "Попытка останавливается")?;
        self.operation_connection(operation)
    }
    pub fn consume_approval(
        &mut self,
        task_id: &str,
        epoch: u64,
        id: &str,
        timestamp: i64,
    ) -> Result<Approval> {
        let approval = ApprovalLedger::for_execution(&self.workspace.db, task_id, epoch)?
            .ok_or_else(|| Error::Denied("Нет разрешения для этой попытки".into()))?;
        require(approval.id == id, "Указано разрешение другой операции")?;
        self.verify_operation(&approval.operation)?;
        ApprovalLedger::consume(&mut self.workspace.db, id, &approval.operation, timestamp)
    }
}
