use crate::error::{Error, Result, require};
use crate::registry::{Registry, RoleVersion, id, now};
use crate::topology::{Admission, Architecture, Target};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompletionRule {
    #[default]
    AllRequired,
    Quorum {
        required: usize,
    },
    FirstAccepted,
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct TaskInput {
    pub title: String,
    pub prompt: String,
    pub criteria: String,
    pub target: Target,
    pub completion: CompletionRule,
    pub priority: i32,
    pub dependencies: Vec<String>,
    pub new_session: bool,
}
impl Default for TaskInput {
    fn default() -> Self {
        Self {
            title: String::new(),
            prompt: String::new(),
            criteria: String::new(),
            target: Target::System { broadcast: false },
            completion: CompletionRule::AllRequired,
            priority: 0,
            dependencies: vec![],
            new_session: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Waiting,
    WaitingUser,
    Stopping,
    AwaitingAcceptance,
    Accepted,
    Rejected,
    Failed,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Lease {
    pub epoch: u64,
    pub worker_id: String,
    pub acquired_at: i64,
    pub heartbeat_at: i64,
    pub pid: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub run_id: String,
    pub assignee: String,
    pub prompt: String,
    pub criteria: String,
    pub role: RoleVersion,
    pub dependencies: Vec<String>,
    pub priority: i32,
    pub new_session: bool,
    pub state: TaskState,
    pub lease: Option<Lease>,
    pub attempt: u32,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub waiting: Option<WaitCondition>,
    #[serde(default)]
    pub resume_note: Option<String>,
    #[serde(default)]
    pub approval_wait: Option<ApprovalWait>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalWait {
    pub approval_id: String,
    pub origin_epoch: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitCondition {
    pub sender: Option<String>,
    pub correlation_id: Option<String>,
    pub deadline_at: i64,
}
impl WaitCondition {
    pub fn validate(&self, timestamp: i64) -> Result<()> {
        require(
            self.deadline_at > timestamp && self.deadline_at <= timestamp.saturating_add(86400),
            "Ожидание должно иметь срок от 1 секунды до 24 часов",
        )?;
        require(
            self.sender
                .iter()
                .chain(self.correlation_id.iter())
                .all(|s| !s.is_empty() && s.len() <= 200),
            "Некорректный адресат или correlation ID",
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub title: String,
    pub input: TaskInput,
    pub config_revision: i64,
    pub architecture: Architecture,
    pub task_ids: Vec<String>,
    pub created_at: String,
    pub accepted: bool,
    pub stopped: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Scheduler {
    pub runs: Vec<Run>,
    pub tasks: Vec<Task>,
    pub paused: bool,
    #[serde(default)]
    receipts: BTreeMap<String, (String, String)>,
    #[serde(default)]
    epoch: u64,
}

impl Scheduler {
    pub fn load(db: &crate::storage::Store) -> Result<Self> {
        let mut scheduler = db
            .get::<Self>("scheduler", "metadata")?
            .map(|s| s.value)
            .unwrap_or_default();
        scheduler.runs = db
            .list::<Run>("runs", 100000)?
            .into_iter()
            .map(|s| s.value)
            .collect();
        scheduler.tasks = db
            .list::<Task>("tasks", 100000)?
            .into_iter()
            .map(|s| s.value)
            .collect();
        scheduler
            .runs
            .sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        scheduler
            .tasks
            .sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(scheduler)
    }
    pub fn persist_delta(
        &self,
        before: &Self,
        tx: &crate::storage::WriteTransaction<'_>,
    ) -> Result<()> {
        let previous_tasks: BTreeMap<_, _> = before.tasks.iter().map(|t| (&t.id, t)).collect();
        let previous_runs: BTreeMap<_, _> = before.runs.iter().map(|r| (&r.id, r)).collect();
        for task in &self.tasks {
            if previous_tasks.get(&task.id).copied() == Some(task) {
                continue;
            }
            let revision = tx
                .get::<Task>("tasks", &task.id)?
                .map(|s| s.revision)
                .unwrap_or(0);
            tx.put("tasks", &task.id, revision, task)?;
            let previous = previous_tasks.get(&task.id).map(|t| t.state);
            if previous != Some(task.state) {
                tx.emit(crate::storage::EventInput::new("task.state_changed",serde_json::json!({"taskId":task.id,"state":task.state,"previousState":previous,"attempt":task.attempt,"epoch":task.lease.as_ref().map(|l| l.epoch)})).agent(&task.assignee).run(&task.run_id))?;
            }
        }
        for run in &self.runs {
            if previous_runs.get(&run.id).copied() == Some(run) {
                continue;
            }
            let revision = tx
                .get::<Run>("runs", &run.id)?
                .map(|s| s.revision)
                .unwrap_or(0);
            tx.put("runs", &run.id, revision, run)?;
        }
        if self.epoch != before.epoch
            || self.receipts != before.receipts
            || self.paused != before.paused
        {
            let metadata = Self {
                runs: vec![],
                tasks: vec![],
                paused: self.paused,
                receipts: self.receipts.clone(),
                epoch: self.epoch,
            };
            let revision = tx
                .get::<Self>("scheduler", "metadata")?
                .map(|s| s.revision)
                .unwrap_or(0);
            tx.put("scheduler", "metadata", revision, &metadata)?;
            if self.paused != before.paused {
                tx.emit(crate::storage::EventInput::new(
                    "scheduler.pause_changed",
                    serde_json::json!({"paused":self.paused}),
                ))?;
            }
        }
        Ok(())
    }
    pub fn task(&self, id: &str) -> Result<&Task> {
        self.tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| Error::NotFound("Задача не найдена".into()))
    }
    pub fn task_mut(&mut self, id: &str) -> Result<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| Error::NotFound("Задача не найдена".into()))
    }
    pub fn submit(
        &mut self,
        key: &str,
        registry: &Registry,
        architecture: &Architecture,
        config_revision: i64,
        input: TaskInput,
        available_slots: usize,
    ) -> Result<Run> {
        require(
            !key.is_empty() && key.len() <= 128,
            "Некорректный ключ запуска",
        )?;
        let fingerprint = serde_json::to_string(&(config_revision, &input))?;
        if let Some((prior, run_id)) = self.receipts.get(key) {
            if prior != &fingerprint {
                return Err(Error::Conflict(
                    "Ключ задачи уже использован с другим содержимым".into(),
                ));
            }
            return self
                .runs
                .iter()
                .find(|r| &r.id == run_id)
                .cloned()
                .ok_or_else(|| Error::Storage("Потеряна запись запуска".into()));
        }
        require(
            !input.prompt.trim().is_empty() && input.prompt.len() <= 100_000,
            "Задача обязательна и не должна превышать 100 КБ",
        )?;
        require(
            input.criteria.len() <= 20_000
                && input.title.len() <= 500
                && (-100..=100).contains(&input.priority),
            "Некорректные поля задачи",
        )?;
        require(
            self.tasks.len() < 100_000,
            "Архивируйте историю перед добавлением задач",
        )?;
        let recipients = architecture.resolve(registry, &input.target)?;
        if architecture.admission == Admission::StartAll
            && (recipients.len() > available_slots
                || recipients.len() > architecture.max_concurrency)
        {
            return Err(Error::Capacity(format!(
                "Нужны {} одновременных слотов; доступны {}. Запуск не создан",
                recipients.len(),
                available_slots.min(architecture.max_concurrency)
            )));
        }
        if let CompletionRule::Quorum { required } = input.completion {
            require(
                required > 0 && required <= recipients.len(),
                "Кворум должен быть от 1 до числа исполнителей",
            )?;
        }
        for dependency in &input.dependencies {
            self.task(dependency)?;
        }
        let run_id = id("run");
        let tasks: Vec<Task> = recipients
            .into_iter()
            .map(|assignee| {
                let agent = registry.agent(&assignee)?;
                Ok(Task {
                    id: id("task"),
                    run_id: run_id.clone(),
                    assignee,
                    prompt: input.prompt.clone(),
                    criteria: input.criteria.clone(),
                    role: registry.role(&agent.role_id, agent.role_version)?.clone(),
                    dependencies: input.dependencies.clone(),
                    priority: input.priority,
                    new_session: input.new_session,
                    state: TaskState::Queued,
                    lease: None,
                    attempt: 0,
                    result: None,
                    error: None,
                    created_at: now(),
                    completed_at: None,
                    artifact_ids: vec![],
                    waiting: None,
                    resume_note: None,
                    approval_wait: None,
                })
            })
            .collect::<Result<_>>()?;
        let run = Run {
            id: run_id,
            title: if input.title.trim().is_empty() {
                input.prompt.chars().take(80).collect()
            } else {
                input.title.clone()
            },
            input,
            config_revision,
            architecture: architecture.clone(),
            task_ids: tasks.iter().map(|t| t.id.clone()).collect(),
            created_at: now(),
            accepted: false,
            stopped: false,
        };
        self.receipts
            .insert(key.into(), (fingerprint, run.id.clone()));
        self.tasks.extend(tasks);
        self.runs.push(run.clone());
        Ok(run)
    }
    pub fn claim_ready(
        &mut self,
        capacity: usize,
        worker_id: &str,
        timestamp: i64,
    ) -> Result<Vec<Task>> {
        if self.paused {
            return Ok(vec![]);
        }
        require(
            capacity <= 5000 && !worker_id.is_empty(),
            "Некорректная вместимость worker",
        )?;
        let occupied = |state| {
            matches!(
                state,
                TaskState::Running | TaskState::Stopping | TaskState::Unknown
            )
        };
        let mut busy: HashSet<String> = self
            .tasks
            .iter()
            .filter(|t| occupied(t.state))
            .map(|t| t.assignee.clone())
            .collect();
        let mut remaining = capacity.saturating_sub(busy.len());
        let accepted: HashSet<_> = self
            .tasks
            .iter()
            .filter(|t| t.state == TaskState::Accepted)
            .map(|t| t.id.clone())
            .collect();
        let mut per_run: BTreeMap<String, usize> = BTreeMap::new();
        for t in self.tasks.iter().filter(|t| occupied(t.state)) {
            *per_run.entry(t.run_id.clone()).or_default() += 1;
        }
        let mut order: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.state == TaskState::Queued)
            .map(|(i, _)| i)
            .collect();
        order.sort_by_key(|&i| (-self.tasks[i].priority, i));
        let mut claims = vec![];
        for i in order {
            if remaining == 0 {
                break;
            }
            let task = &self.tasks[i];
            if busy.contains(&task.assignee)
                || !task.dependencies.iter().all(|d| accepted.contains(d))
            {
                continue;
            }
            let run = self
                .runs
                .iter()
                .find(|r| r.id == task.run_id)
                .ok_or_else(|| Error::Storage("Потерян run задачи".into()))?;
            if run.stopped || run.architecture.paused {
                continue;
            }
            let running = per_run.get(&run.id).copied().unwrap_or(0);
            if running >= run.architecture.max_concurrency {
                continue;
            }
            if run.architecture.admission == Admission::StartAll && running == 0 {
                let queued: Vec<_> = self
                    .tasks
                    .iter()
                    .filter(|t| t.run_id == run.id && t.state == TaskState::Queued)
                    .collect();
                if queued.len() > remaining
                    || queued.iter().any(|t| {
                        busy.contains(&t.assignee)
                            || !t.dependencies.iter().all(|d| accepted.contains(d))
                    })
                {
                    continue;
                }
            }
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or_else(|| Error::Capacity("Переполнение epoch".into()))?;
            let task = &mut self.tasks[i];
            task.state = TaskState::Running;
            task.attempt += 1;
            task.lease = Some(Lease {
                epoch: self.epoch,
                worker_id: worker_id.into(),
                acquired_at: timestamp,
                heartbeat_at: timestamp,
                pid: None,
            });
            busy.insert(task.assignee.clone());
            *per_run.entry(task.run_id.clone()).or_default() += 1;
            remaining -= 1;
            claims.push(task.clone());
        }
        Ok(claims)
    }
    pub fn verify_lease(&self, task_id: &str, epoch: u64) -> Result<&Task> {
        let t = self.task(task_id)?;
        if !matches!(t.state, TaskState::Running | TaskState::Stopping)
            || !t.lease.as_ref().is_some_and(|l| l.epoch == epoch)
        {
            return Err(Error::Denied(
                "Lease устарел или задача уже завершена".into(),
            ));
        }
        Ok(t)
    }
    pub fn heartbeat(
        &mut self,
        task_id: &str,
        epoch: u64,
        timestamp: i64,
        pid: Option<u32>,
    ) -> Result<()> {
        self.verify_lease(task_id, epoch)?;
        if let Some(lease) = &mut self.task_mut(task_id)?.lease {
            lease.heartbeat_at = timestamp;
            if pid.is_some() {
                lease.pid = pid;
            }
        }
        Ok(())
    }
    pub fn finish(
        &mut self,
        task_id: &str,
        epoch: u64,
        result: std::result::Result<String, String>,
    ) -> Result<()> {
        self.verify_lease(task_id, epoch)?;
        let t = self.task_mut(task_id)?;
        let stopping = t.state == TaskState::Stopping;
        match result {
            Ok(text) => {
                t.result = Some(text);
                t.state = if stopping {
                    TaskState::Cancelled
                } else {
                    TaskState::AwaitingAcceptance
                };
            }
            Err(error) => {
                t.error = Some(error);
                t.state = if stopping {
                    TaskState::Cancelled
                } else {
                    TaskState::Failed
                };
            }
        }
        t.completed_at = Some(now());
        Ok(())
    }
    /// Called only after the supervisor reaped a successful yielding process.
    pub fn park(
        &mut self,
        task_id: &str,
        epoch: u64,
        condition: WaitCondition,
        summary: String,
        timestamp: i64,
    ) -> Result<()> {
        self.verify_lease(task_id, epoch)?;
        condition.validate(timestamp)?;
        require(summary.len() <= 65536, "Сводка ожидания превышает 64 КиБ")?;
        let task = self.task_mut(task_id)?;
        require(
            task.state == TaskState::Running && task.attempt < task.role.draft.max_turns,
            "Ожидание невозможно после остановки или исчерпания turn budget",
        )?;
        task.state = TaskState::Waiting;
        task.waiting = Some(condition);
        task.result = Some(summary);
        task.error = None;
        task.completed_at = None;
        task.lease = None;
        Ok(())
    }
    /// The engine verifies the external wake condition; the scheduler only
    /// performs the fenced state change. A cancelled wait cannot be resurrected.
    pub fn wake(&mut self, task_id: &str, reason: String) -> Result<()> {
        require(
            !reason.is_empty() && reason.len() <= 1000,
            "Нужна краткая причина пробуждения",
        )?;
        let task = self.task_mut(task_id)?;
        require(
            task.state == TaskState::Waiting && task.waiting.is_some(),
            "Задача не ожидает события",
        )?;
        require(
            task.attempt < task.role.draft.max_turns,
            "Исчерпан turn budget роли",
        )?;
        task.state = TaskState::Queued;
        task.waiting = None;
        task.resume_note = Some(reason);
        task.result = None;
        task.artifact_ids.clear();
        Ok(())
    }
    /// Only after successful process reap; this state holds no model-process lease.
    pub fn park_approval(
        &mut self,
        task_id: &str,
        epoch: u64,
        approval_id: &str,
        summary: String,
    ) -> Result<()> {
        self.verify_lease(task_id, epoch)?;
        require(
            !approval_id.is_empty() && approval_id.len() <= 200 && summary.len() <= 65536,
            "Некорректное ожидание решения",
        )?;
        let task = self.task_mut(task_id)?;
        require(
            task.state == TaskState::Running && task.attempt < task.role.draft.max_turns,
            "Ожидание невозможно после остановки или исчерпания turn budget",
        )?;
        task.state = TaskState::WaitingUser;
        task.approval_wait = Some(ApprovalWait {
            approval_id: approval_id.into(),
            origin_epoch: epoch,
        });
        task.waiting = None;
        task.lease = None;
        task.result = Some(summary);
        task.error = None;
        task.completed_at = None;
        Ok(())
    }
    /// The application layer must verify the final operation outcome first.
    /// Merely approving an unexecuted request is not a wake condition.
    pub fn wake_approval(
        &mut self,
        task_id: &str,
        approval_id: &str,
        reason: String,
    ) -> Result<()> {
        require(
            !reason.trim().is_empty() && reason.len() <= 1000,
            "Нужна краткая причина продолжения",
        )?;
        let task = self.task_mut(task_id)?;
        require(
            task.state == TaskState::WaitingUser
                && task
                    .approval_wait
                    .as_ref()
                    .is_some_and(|wait| wait.approval_id == approval_id),
            "Задача не ожидает эту операцию",
        )?;
        require(
            task.attempt < task.role.draft.max_turns,
            "Исчерпан turn budget роли",
        )?;
        task.state = TaskState::Queued;
        task.approval_wait = None;
        task.resume_note = Some(reason);
        task.result = None;
        task.artifact_ids.clear();
        Ok(())
    }
    pub fn accept(&mut self, task_id: &str, accept: bool) -> Result<()> {
        let task = self.task_mut(task_id)?;
        require(
            task.state == TaskState::AwaitingAcceptance,
            "Принять можно только проверяемый завершённый результат",
        )?;
        task.state = if accept {
            TaskState::Accepted
        } else {
            TaskState::Rejected
        };
        let run_id = task.run_id.clone();
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| Error::Storage("Run не найден".into()))?;
        let count = self
            .tasks
            .iter()
            .filter(|t| t.run_id == run_id && t.state == TaskState::Accepted)
            .count();
        run.accepted = match run.input.completion {
            CompletionRule::AllRequired => count == run.task_ids.len(),
            CompletionRule::Quorum { required } => count >= required,
            CompletionRule::FirstAccepted => count > 0,
            CompletionRule::Manual => false,
        };
        if run.accepted && matches!(run.input.completion, CompletionRule::FirstAccepted) {
            for t in self.tasks.iter_mut().filter(|t| t.run_id == run_id) {
                if matches!(
                    t.state,
                    TaskState::Queued | TaskState::Waiting | TaskState::WaitingUser
                ) {
                    t.state = TaskState::Cancelled;
                    t.waiting = None;
                    t.approval_wait = None;
                } else if t.state == TaskState::Running {
                    t.state = TaskState::Stopping;
                }
            }
        }
        Ok(())
    }
    pub fn accept_run_manually(&mut self, run_id: &str) -> Result<()> {
        require(
            self.tasks
                .iter()
                .filter(|t| t.run_id == run_id)
                .all(|t| t.state == TaskState::Accepted),
            "Сначала примите все обязательные результаты",
        )?;
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| Error::NotFound("Run не найден".into()))?;
        run.accepted = true;
        Ok(())
    }
    pub fn cancel_task(&mut self, task_id: &str) -> Result<()> {
        let t = self.task_mut(task_id)?;
        match t.state {
            TaskState::Queued | TaskState::Waiting | TaskState::WaitingUser => {
                t.state = TaskState::Cancelled;
                t.waiting = None;
                t.approval_wait = None;
                t.completed_at = Some(now());
            }
            TaskState::Running => t.state = TaskState::Stopping,
            _ => {}
        }
        Ok(())
    }
    pub fn cancel_run(&mut self, run_id: &str) -> Result<()> {
        let r = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| Error::NotFound("Run не найден".into()))?;
        r.stopped = true;
        let tasks = r.task_ids.clone();
        for id in tasks {
            self.cancel_task(&id)?;
        }
        Ok(())
    }
    pub fn recover_after_crash(&mut self) {
        for task in &mut self.tasks {
            if matches!(task.state, TaskState::Running | TaskState::Stopping) {
                task.state = TaskState::Unknown;
                task.error=Some("Служба перезапущена; исход попытки неизвестен. Сверьте процессы и внешние действия перед повтором.".into());
            }
        }
    }
}
