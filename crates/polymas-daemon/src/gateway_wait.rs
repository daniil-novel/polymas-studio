use crate::{
    AppState,
    gateway::{Scope, validate},
};
use polymas_core::{
    error::{Error, Result, require},
    registry::AgentStatus,
    scheduler::WaitCondition,
    storage::EventInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaitInput {
    pub sender: Option<String>,
    pub correlation_id: Option<String>,
    pub timeout_seconds: i64,
    pub summary: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitDirective {
    pub task_id: String,
    pub epoch: u64,
    pub requested_at: i64,
    pub condition: WaitCondition,
    pub input: WaitInput,
}
pub fn definition() -> Value {
    json!({"name":"polymas_wait_for_message","description":"Request continuation after an authorized queued message in this run, or a deadline. Optional sender and correlationId narrow the condition. After this receipt, finish the current turn with a brief public progress summary. PolyMAS parks only after the process exits successfully, then resumes this same agent session. This is intermediate work, not a completed result. Repeating identical arguments in this attempt is safe; changing a pending wait is rejected. Waiting consumes one turn from the role budget.","inputSchema":{"type":"object","properties":{"sender":{"type":["string","null"],"maxLength":200},"correlationId":{"type":["string","null"],"maxLength":200},"timeoutSeconds":{"type":"integer","minimum":1,"maximum":86400},"summary":{"type":"string","minLength":1,"maxLength":20000}},"required":["timeoutSeconds","summary"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}})
}
pub async fn request(state: &AppState, scope: &Scope, input: WaitInput) -> Result<Value> {
    let mut engine = state.engine.lock().await;
    validate(&engine, scope)?;
    require(
        engine
            .communication_scope(&scope.run_id, &scope.agent_id)?
            .mode
            == polymas_core::topology::CommunicationMode::Direct,
        "Ожидание адресного сообщения недоступно в режиме публикаций",
    )?;
    crate::gateway_operations::require_previous_resolved(&engine, scope)?;
    require(
        polymas_core::policy::ApprovalLedger::for_attempt(
            &engine.workspace.db,
            &scope.task_id,
            scope.epoch,
        )?
        .is_none(),
        "В этой попытке уже запрошено решение владельца; завершите turn",
    )?;
    require(
        (1..=86400).contains(&input.timeout_seconds)
            && !input.summary.trim().is_empty()
            && input.summary.len() <= 20000,
        "Укажите срок 1–86400 секунд и сводку до 20000 байт",
    )?;
    let task = engine.scheduler.task(&scope.task_id)?;
    require(
        task.attempt < task.role.draft.max_turns,
        "Нет бюджета для следующего turn; завершите задачу с указанием ограничения",
    )?;
    if let Some(sender) = &input.sender {
        let snapshot = engine.run_configuration(&scope.run_id)?;
        let live = &engine.workspace.config().value;
        require(
            [&snapshot, live].iter().all(|config| {
                config
                    .registry
                    .agent(sender)
                    .is_ok_and(|a| a.status != AgentStatus::Archived)
                    && config.architecture.permits_message(sender, &scope.agent_id)
            }),
            "Указанный отправитель не имеет действующего канала в обеих архитектурах",
        )?;
    }
    let timestamp = chrono::Utc::now().timestamp();
    let condition = WaitCondition {
        sender: input.sender.clone(),
        correlation_id: input.correlation_id.clone(),
        deadline_at: timestamp + input.timeout_seconds,
    };
    condition.validate(timestamp)?;
    let key = format!("{}:{}", scope.task_id, scope.epoch);
    let directive = if let Some(saved) = engine
        .workspace
        .db
        .get::<WaitDirective>("wait_directives", &key)?
    {
        if saved.value.input != input {
            return Err(Error::Conflict(
                "В этой попытке уже записано другое ожидание".into(),
            ));
        }
        saved.value
    } else {
        let directive = WaitDirective {
            task_id: scope.task_id.clone(),
            epoch: scope.epoch,
            requested_at: timestamp,
            condition,
            input,
        };
        engine.workspace.db.transaction(|tx| {
            tx.put("wait_directives",&key,0,&directive)?;
            tx.emit(EventInput::new("task.wait_requested",json!({"taskId":scope.task_id,"epoch":scope.epoch,"condition":directive.condition})).agent(&scope.agent_id).run(&scope.run_id))?;
            Ok(())
        })?;
        directive
    };
    Ok(
        json!({"state":"wait_requested","condition":directive.condition,"nextAction":"Finish this turn successfully. PolyMAS will free the process slot and resume this same session on the matching message or deadline."}),
    )
}
