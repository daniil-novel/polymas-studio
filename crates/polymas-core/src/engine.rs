use crate::error::{Error, Result, require};
use crate::policy::Binding;
use crate::scheduler::{Run, Scheduler, Task, TaskInput, TaskState, WaitCondition};
use crate::storage::EventInput;
use crate::workspace::{ConfigCommand, Workspace};
use serde_json::{Value, json};
use std::path::Path;

/// Synchronous transactional application layer. The daemon owns one Engine
/// behind a short mutex; provider and MCP I/O always run outside this mutex.
pub struct Engine {
    pub workspace: Workspace,
    pub scheduler: Scheduler,
}
impl Engine {
    pub fn run_configuration(&self, run_id: &str) -> Result<crate::workspace::Configuration> {
        let run = self
            .scheduler
            .runs
            .iter()
            .find(|r| r.id == run_id)
            .ok_or_else(|| Error::NotFound("Запуск не найден".into()))?;
        Ok(self
            .workspace
            .db
            .get::<crate::workspace::HistoryEntry>(
                "configuration_history",
                &run.config_revision.to_string(),
            )?
            .ok_or_else(|| Error::Storage("Снимок запуска недоступен".into()))?
            .value
            .configuration)
    }
    pub fn pending_context_messages(
        &self,
        task_id: &str,
        timestamp: i64,
        limit: usize,
    ) -> Result<Vec<crate::messages::Message>> {
        let task = self.scheduler.task(task_id)?;
        let snapshot = self.run_configuration(&task.run_id)?;
        let live = &self.workspace.config().value;
        let active = |config: &crate::workspace::Configuration, id: &str| {
            config
                .registry
                .agent(id)
                .is_ok_and(|a| a.status != crate::registry::AgentStatus::Archived)
        };
        require(
            active(&snapshot, &task.assignee) && active(live, &task.assignee),
            "Получатель недоступен",
        )?;
        let senders: Vec<_> = snapshot
            .registry
            .agents
            .iter()
            .filter(|a| {
                active(&snapshot, &a.id)
                    && active(live, &a.id)
                    && snapshot.architecture.permits_message(&a.id, &task.assignee)
                    && live.architecture.permits_message(&a.id, &task.assignee)
            })
            .map(|a| a.id.clone())
            .collect();
        crate::messages::Router::pending_authorized(
            &self.workspace.db,
            &task.run_id,
            &task.assignee,
            &senders,
            timestamp,
            limit,
        )
    }
    pub fn park(
        &mut self,
        task_id: &str,
        epoch: u64,
        condition: WaitCondition,
        summary: String,
        requested_at: i64,
    ) -> Result<()> {
        self.transition(|scheduler| {
            scheduler.park(task_id, epoch, condition, summary, requested_at)
        })
    }
    pub fn wake_waiting(&mut self, timestamp: i64) -> Result<Vec<String>> {
        let mut decisions = vec![];
        let live = &self.workspace.config().value;
        let mut snapshots = std::collections::BTreeMap::new();
        for task in self
            .scheduler
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Waiting)
        {
            let Some(condition) = &task.waiting else {
                return Err(Error::Storage(
                    "У ожидающей задачи отсутствует условие".into(),
                ));
            };
            let run = self
                .scheduler
                .runs
                .iter()
                .find(|r| r.id == task.run_id)
                .ok_or_else(|| Error::Storage("Запуск ожидания не найден".into()))?;
            if run.stopped {
                continue;
            }
            let reason = if timestamp >= condition.deadline_at {
                Some(
                    "Истёк срок ожидания сообщения. Сообщите об ограничении или примените правило эскалации роли.",
                )
            } else {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    snapshots.entry(run.config_revision)
                {
                    let saved = self
                        .workspace
                        .db
                        .get::<crate::workspace::HistoryEntry>(
                            "configuration_history",
                            &run.config_revision.to_string(),
                        )?
                        .ok_or_else(|| Error::Storage("Снимок запуска недоступен".into()))?;
                    entry.insert(saved.value.configuration);
                }
                let snapshot = &snapshots[&run.config_revision];
                let senders = crate::messages::Router::pending_senders(
                    &self.workspace.db,
                    &task.run_id,
                    &task.assignee,
                    condition,
                    timestamp,
                )?;
                senders.into_iter().any(|sender| {
                    snapshot.registry.agent(&sender).is_ok_and(|a| a.status != crate::registry::AgentStatus::Archived)
                        && live.registry.agent(&sender).is_ok_and(|a| a.status != crate::registry::AgentStatus::Archived)
                        && snapshot.architecture.permits_message(&sender, &task.assignee)
                        && live.architecture.permits_message(&sender, &task.assignee)
                }).then_some("Получено подходящее сообщение. Обработайте входной batch с дедупликацией по messageId.")
            };
            if let Some(reason) = reason {
                decisions.push((
                    task.id.clone(),
                    task.run_id.clone(),
                    task.assignee.clone(),
                    reason.to_owned(),
                ));
            }
        }
        if decisions.is_empty() {
            return Ok(vec![]);
        }
        let mut candidate = self.scheduler.clone();
        for (id, _, _, reason) in &decisions {
            candidate.wake(id, reason.clone())?;
        }
        self.workspace.db.transaction(|tx| {
            candidate.persist_delta(&self.scheduler, tx)?;
            for (id, run_id, agent_id, reason) in &decisions {
                tx.emit(
                    EventInput::new("task.woken", json!({"taskId":id,"reason":reason}))
                        .agent(agent_id)
                        .run(run_id),
                )?;
            }
            Ok(())
        })?;
        self.scheduler = candidate;
        Ok(decisions.into_iter().map(|(id, _, _, _)| id).collect())
    }
    pub fn open(root: &Path) -> Result<Self> {
        let mut workspace = Workspace::open(root)?;
        let before = Scheduler::load(&workspace.db)?;
        let mut scheduler = before.clone();
        scheduler.recover_after_crash();
        workspace
            .db
            .transaction(|tx| scheduler.persist_delta(&before, tx))?;
        Ok(Self {
            workspace,
            scheduler,
        })
    }
    pub fn configure(&mut self, revision: i64, command: ConfigCommand) -> Result<Value> {
        if let ConfigCommand::ArchiveAgent { agent_id } = &command {
            require(
                !self.scheduler.tasks.iter().any(|t| {
                    &t.assignee == agent_id
                        && matches!(
                            t.state,
                            TaskState::Queued
                                | TaskState::Waiting
                                | TaskState::WaitingUser
                                | TaskState::Running
                                | TaskState::Stopping
                                | TaskState::Unknown
                        )
                }),
                "Сначала остановите и сверяйте все попытки агента",
            )?;
        }
        self.workspace.configure(revision, command)
    }
    pub fn submit(&mut self, key: &str, input: TaskInput, slots: usize) -> Result<Run> {
        let config = self.workspace.config();
        let mut candidate = self.scheduler.clone();
        let run = candidate.submit(
            key,
            &config.value.registry,
            &config.value.architecture,
            config.revision,
            input,
            slots,
        )?;
        if self
            .scheduler
            .runs
            .iter()
            .any(|existing| existing.id == run.id)
        {
            return Ok(run);
        }
        for task_id in &run.task_ids {
            let task = candidate.task(task_id)?;
            self.required_connections(task, &config.value.bindings)?;
        }
        let bindings = config.value.bindings.clone();
        self.workspace.db.transaction(|tx| {
            candidate.persist_delta(&self.scheduler,tx)?;
            if tx.get::<Vec<Binding>>("run_bindings",&run.id)?.is_none() {
                tx.put("run_bindings",&run.id,0,&bindings)?;
                tx.emit(EventInput::new("run.created",json!({"runId":run.id,"taskCount":run.task_ids.len(),"configRevision":run.config_revision})).run(&run.id))?;
            }
            Ok(())
        })?;
        self.scheduler = candidate;
        Ok(run)
    }
    fn required_connections(&self, task: &Task, bindings: &[Binding]) -> Result<()> {
        for binding in bindings
            .iter()
            .filter(|b| b.required && b.matches(&task.assignee, &task.role.id))
        {
            let connection = self.workspace.connection(&binding.connection_id)?.value;
            if !connection.enabled || connection.health != crate::connections::Health::Ready {
                return Err(Error::Denied(format!(
                    "Обязательный MCP «{}» недоступен",
                    connection.name
                )));
            }
        }
        Ok(())
    }
    pub fn bindings_for_run(&self, run_id: &str) -> Result<Vec<Binding>> {
        self.workspace
            .db
            .get("run_bindings", run_id)?
            .map(|s| s.value)
            .ok_or_else(|| Error::Storage("Снимок bindings не найден".into()))
    }
    fn transition<T>(&mut self, f: impl FnOnce(&mut Scheduler) -> Result<T>) -> Result<T> {
        let mut candidate = self.scheduler.clone();
        let result = f(&mut candidate)?;
        self.workspace
            .db
            .transaction(|tx| candidate.persist_delta(&self.scheduler, tx))?;
        self.scheduler = candidate;
        Ok(result)
    }
    pub fn claim(&mut self, capacity: usize, worker: &str, timestamp: i64) -> Result<Vec<Task>> {
        let mut candidate = self.scheduler.clone();
        let claims = candidate.claim_ready(capacity, worker, timestamp)?;
        for task in &claims {
            self.required_connections(task, &self.bindings_for_run(&task.run_id)?)?;
            self.required_connections(task, &self.workspace.config().value.bindings)?;
            self.workspace.agent_workspace(&task.assignee)?;
        }
        self.workspace
            .db
            .transaction(|tx| candidate.persist_delta(&self.scheduler, tx))?;
        self.scheduler = candidate;
        Ok(claims)
    }
    pub fn heartbeat(
        &mut self,
        task_id: &str,
        epoch: u64,
        timestamp: i64,
        pid: Option<u32>,
    ) -> Result<()> {
        self.transition(|s| s.heartbeat(task_id, epoch, timestamp, pid))
    }
    pub fn finish(
        &mut self,
        task_id: &str,
        epoch: u64,
        result: std::result::Result<String, String>,
    ) -> Result<()> {
        if let Ok(text) = &result {
            require(
                text.len() <= 4 * 1024 * 1024,
                "Результат превышает 4 МиБ; сохраните файл как артефакт",
            )?;
        }
        self.transition(|s| s.finish(task_id, epoch, result))
    }
    pub fn accept(&mut self, task_id: &str) -> Result<()> {
        let mut candidate = self.scheduler.clone();
        candidate.accept(task_id, true)?;
        let task = candidate.task(task_id)?;
        self.workspace.db.transaction(|tx| {
            for id in &task.artifact_ids {
                let stored = tx.get::<crate::artifacts::Artifact>("artifacts", id)?.ok_or_else(|| Error::Storage("Артефакт результата не найден".into()))?;
                let mut artifact = stored.value;
                require(artifact.task_id == task.id && artifact.agent_id == task.assignee && artifact.attempt == task.attempt && !artifact.partial, "Артефакт не принадлежит завершённой попытке этой задачи")?;
                artifact.accepted = true;
                tx.put("artifacts", id, stored.revision, &artifact)?;
                tx.emit(EventInput::new("artifact.accepted", json!({"artifactId":id,"taskId":task.id,"attempt":task.attempt,"author":"local:owner"})).agent(&task.assignee).run(&task.run_id))?;
            }
            candidate.persist_delta(&self.scheduler, tx)?;
            Ok(())
        })?;
        self.scheduler = candidate;
        Ok(())
    }
    pub fn reject(&mut self, task_id: &str) -> Result<()> {
        self.transition(|s| s.accept(task_id, false))
    }
    pub fn accept_run(&mut self, run_id: &str) -> Result<()> {
        self.transition(|s| s.accept_run_manually(run_id))
    }
    pub fn cancel_task(&mut self, task_id: &str) -> Result<()> {
        self.transition(|s| s.cancel_task(task_id))
    }
    pub fn cancel_run(&mut self, run_id: &str) -> Result<()> {
        self.transition(|s| s.cancel_run(run_id))
    }
    pub fn pause(&mut self, paused: bool) -> Result<()> {
        self.transition(|s| {
            s.paused = paused;
            Ok(())
        })
    }
    /// Only the local owner may reconcile an unknown external outcome. The
    /// daemon must verify process ownership/reaping before allowing this call.
    pub fn reconcile(&mut self, task_id: &str, retry: bool, note: &str) -> Result<()> {
        require(
            !note.trim().is_empty() && note.len() <= 10000,
            "Укажите результат сверки процессов и внешних действий",
        )?;
        let mut candidate = self.scheduler.clone();
        let task = candidate.task_mut(task_id)?;
        require(
            matches!(
                task.state,
                TaskState::Unknown | TaskState::Failed | TaskState::Cancelled | TaskState::Rejected
            ),
            "Этот исход не требует сверки",
        )?;
        require(
            !retry || task.attempt < task.role.draft.max_turns,
            "Исчерпан лимит попыток роли",
        )?;
        task.state = if retry {
            TaskState::Queued
        } else {
            TaskState::Cancelled
        };
        task.lease = None;
        task.continuation = None;
        task.completed_at = None;
        task.error = None;
        if retry {
            task.result = None;
            task.artifact_ids.clear();
        }
        self.workspace.db.transaction(|tx| {
            candidate.persist_delta(&self.scheduler, tx)?;
            tx.emit(EventInput::new(
                "task.reconciled",
                json!({"taskId":task_id,"retry":retry,"note":note,"author":"local:owner"}),
            ))?;
            Ok(())
        })?;
        self.scheduler = candidate;
        Ok(())
    }
}
