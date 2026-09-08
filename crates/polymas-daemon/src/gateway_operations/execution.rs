use super::*;
use crate::connection_api::credentials;
use polymas_core::mcp_client::call_tool;
use std::time::Duration;

pub(super) async fn execute(
    state: &AppState,
    scope: &Scope,
    input: ApprovalInput,
) -> Result<Value> {
    let (approval, connection, workspace_id) = {
        let engine = state.engine.lock().await;
        let approval = scoped_approval(&engine, scope, &input.approval_id)?;
        require(
            approval.operation.epoch == scope.epoch,
            "Исполнение доступно только зарезервированному продолжению",
        )?;
        let connection = engine.verify_operation(&approval.operation)?;
        if matches!(
            approval.state,
            ApprovalState::Executing
                | ApprovalState::Succeeded
                | ApprovalState::Failed
                | ApprovalState::Unknown
        ) {
            return receipt(&engine, scope, &approval);
        }
        require(
            effective_state(&approval) == ApprovalState::Approved,
            "Для этой операции нет действующего решения владельца",
        )?;
        (
            approval,
            connection,
            engine.workspace.config().value.id.clone(),
        )
    };
    // Credential failures are resolved before consume and before any external I/O.
    let credentials = credentials(&connection.value, workspace_id).await?;
    {
        let mut engine = state.engine.lock().await;
        validate(&engine, scope)?;
        engine.consume_approval(
            &scope.task_id,
            scope.epoch,
            &approval.id,
            chrono::Utc::now().timestamp(),
        )?;
    }
    let mut guard = ExecutionGuard {
        state: state.clone(),
        id: approval.id.clone(),
        finished: false,
    };
    let outcome = {
        let operation = tokio::time::timeout(
            Duration::from_secs(connection.value.timeout_seconds),
            call_tool(
                &connection.value,
                &approval.operation.tool,
                approval.operation.arguments.clone(),
                credentials.bearer.as_ref().map(|s| s.expose().to_owned()),
                credentials
                    .environment
                    .iter()
                    .map(|(k, v)| (k.clone(), v.expose().to_owned()))
                    .collect(),
            ),
        );
        tokio::pin!(operation);
        let mut check = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                result = &mut operation => break result.unwrap_or_else(|_|Err(Error::Timeout("Ответ не получен вовремя; внешний исход требует сверки".into()))),
                _ = check.tick() => {
                    let engine = state.engine.lock().await;
                    if validate(&engine, scope).is_err() || engine.verify_operation(&approval.operation).is_err() {
                        break Err(Error::Denied("Доступ отозван во время операции; внешний исход требует сверки".into()));
                    }
                }
            }
        }
    };
    let mut engine = state.engine.lock().await;
    let accessible =
        validate(&engine, scope).is_ok() && engine.verify_operation(&approval.operation).is_ok();
    let (outcome_state, result) = match outcome {
        Ok(value) if accessible => {
            // A tool error can follow partial side effects. Only an affirmative
            // MCP result is success; every ambiguous effect needs owner review.
            let status = if value["isError"] == true {
                ApprovalState::Unknown
            } else {
                ApprovalState::Succeeded
            };
            (
                status,
                json!({"response":credentials.sanitize(value),"diagnostic":null}),
            )
        }
        Ok(_) => (
            ApprovalState::Unknown,
            json!({"response":null,"diagnostic":"Доступ отозван; результат не передан агенту"}),
        ),
        Err(error) => (
            ApprovalState::Unknown,
            json!({"response":null,"diagnostic":credentials.diagnostic(&error.to_string())}),
        ),
    };
    let current = engine
        .workspace
        .db
        .get::<Approval>("approvals", &approval.id)?
        .ok_or_else(|| Error::Storage("Операция исполнения отсутствует".into()))?
        .value;
    let completed = if current.state == ApprovalState::Executing {
        ApprovalLedger::complete_with_result(
            &mut engine.workspace.db,
            &approval.id,
            outcome_state,
            Some(result),
        )?
    } else {
        // Recovery or owner reconciliation wins over a late network response.
        current
    };
    guard.finished = true;
    require(
        accessible,
        "Доступ к результату операции отозван; владельцу нужна сверка",
    )?;
    receipt(&engine, scope, &completed)
}

/// HTTP cancellation can drop the future while its model lease is still active.
/// Persist uncertainty even in that case; process recovery is the final fallback.
struct ExecutionGuard {
    state: AppState,
    id: String,
    finished: bool,
}
impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let state = self.state.clone();
        let id = self.id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut engine = state.engine.lock().await;
                if engine
                    .workspace
                    .db
                    .get::<Approval>("approvals", &id)
                    .ok()
                    .flatten()
                    .is_some_and(|s| s.value.state == ApprovalState::Executing)
                {
                    let _ = ApprovalLedger::complete(
                        &mut engine.workspace.db,
                        &id,
                        ApprovalState::Unknown,
                    );
                }
            });
        }
    }
}
