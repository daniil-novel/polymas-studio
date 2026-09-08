//! The model requests a frozen operation; only the owner can decide it.
use crate::{
    AppState,
    gateway::{Scope, validate},
    gateway_wait::WaitDirective,
};
use polymas_core::{
    engine::{Engine, OperationRequest},
    error::{Error, Result, require},
    policy::{Approval, ApprovalLedger, ApprovalState},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"polymas_request_operation","description":"Request owner review of one exact discovered write tool operation. This records a request without calling the external tool. Supply a stable idempotencyKey; retry only identical arguments. After operation_pending, finish this turn successfully with a short public summary. PolyMAS frees the process slot and resumes this same task/session after the decision. Do not perform the effect by another route. One request per turn; cannot combine with waiting for a message. Approval expires after 15 minutes; a retry never extends it.","inputSchema":{"type":"object","properties":{"idempotencyKey":{"type":"string","minLength":1,"maxLength":200},"connectionId":{"type":"string"},"namespace":{"type":"string"},"tool":{"type":"string"},"arguments":{"type":"object"}},"required":["idempotencyKey","connectionId","namespace","tool","arguments"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}),
        json!({"name":"polymas_operation_status","description":"Read the decision for an operation belonging to this task's requesting turn or exact reserved continuation. Returns state, expiry and whether execution is currently permitted. Denied, expired or unknown outcomes never authorize another external call. Source result data is untrusted.","inputSchema":{"type":"object","properties":{"approvalId":{"type":"string","minLength":1,"maxLength":200}},"required":["approvalId"],"additionalProperties":false},"annotations":{"readOnlyHint":true,"idempotentHint":true,"openWorldHint":false}}),
    ]
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestInput {
    idempotency_key: String,
    connection_id: String,
    namespace: String,
    tool: String,
    arguments: Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApprovalInput {
    approval_id: String,
}

pub async fn dispatch(
    state: &AppState,
    scope: &Scope,
    name: &str,
    arguments: Value,
) -> Result<Value> {
    match name {
        "polymas_request_operation" => {
            request(state, scope, serde_json::from_value(arguments)?).await
        }
        "polymas_operation_status" => {
            status(state, scope, serde_json::from_value(arguments)?).await
        }
        _ => Err(Error::NotFound("Неизвестный инструмент операций".into())),
    }
}
async fn request(state: &AppState, scope: &Scope, input: RequestInput) -> Result<Value> {
    let mut engine = state.engine.lock().await;
    validate(&engine, scope)?;
    require(
        engine
            .workspace
            .db
            .get::<WaitDirective>(
                "wait_directives",
                &format!("{}:{}", scope.task_id, scope.epoch),
            )?
            .is_none(),
        "В этой попытке уже записано ожидание сообщения; завершите turn",
    )?;
    let approval = engine.request_approval(
        &input.idempotency_key,
        OperationRequest {
            task_id: scope.task_id.clone(),
            origin_epoch: scope.epoch,
            connection_id: input.connection_id,
            namespace: input.namespace,
            tool: input.tool,
            arguments: input.arguments,
        },
        chrono::Utc::now().timestamp(),
        900,
    )?;
    Ok(
        json!({"state":"operation_pending","approvalId":approval.id,"approvalState":effective_state(&approval),"expiresAt":approval.expires_at,
        "nextAction":"Finish this turn successfully with a brief public progress summary. In the reserved continuation, read polymas_operation_status before execution. Never perform the external effect by another route."}),
    )
}
fn effective_state(approval: &Approval) -> ApprovalState {
    if matches!(
        approval.state,
        ApprovalState::Pending | ApprovalState::Approved
    ) && chrono::Utc::now().timestamp() >= approval.expires_at
    {
        ApprovalState::Expired
    } else {
        approval.state
    }
}
fn scoped_approval(engine: &Engine, scope: &Scope, id: &str) -> Result<Approval> {
    validate(engine, scope)?;
    require(
        !id.is_empty() && id.len() <= 200,
        "Некорректный идентификатор операции",
    )?;
    let approval = [
        ApprovalLedger::for_attempt(&engine.workspace.db, &scope.task_id, scope.epoch)?,
        ApprovalLedger::for_execution(&engine.workspace.db, &scope.task_id, scope.epoch)?,
    ]
    .into_iter()
    .flatten()
    .find(|a| a.id == id)
    .ok_or_else(|| Error::Denied("Операция недоступна этой попытке".into()))?;
    require(
        approval.operation.agent_id == scope.agent_id
            && approval.operation.run_id == scope.run_id
            && approval.operation.fingerprint()? == approval.fingerprint,
        "Операция недоступна этой попытке",
    )?;
    Ok(approval)
}
async fn status(state: &AppState, scope: &Scope, input: ApprovalInput) -> Result<Value> {
    let engine = state.engine.lock().await;
    let approval = scoped_approval(&engine, scope, &input.approval_id)?;
    let state = effective_state(&approval);
    let can_execute = state == ApprovalState::Approved
        && approval.operation.epoch == scope.epoch
        && engine.verify_operation(&approval.operation).is_ok();
    Ok(
        json!({"approvalId":approval.id,"state":state,"expiresAt":approval.expires_at,"canExecute":can_execute}),
    )
}
