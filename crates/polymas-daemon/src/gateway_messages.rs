use crate::{
    AppState,
    gateway::{Scope, validate},
};
use polymas_core::{
    engine::Engine,
    error::{Error, Result, require},
    messages::{MessageInput, Router},
    registry::AgentStatus,
    workspace::Configuration,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"polymas_list_peers","description":"List agents this attempt may message. Access is the intersection of the run architecture and current architecture; newly granted links require a new run. Returns bounded pages of IDs and names, never another agent's private conversation.","inputSchema":{"type":"object","properties":{"query":{"type":"string","maxLength":200},"cursor":{"type":"string","maxLength":200},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}),
        json!({"name":"polymas_send_message","description":"Queue one message for an allowed peer in this run. Reuse idempotencyKey only for an identical retry. A queued receipt does not mean the recipient has received or processed it. Exec delivers queued messages on the recipient's next turn; sending does not start a new agent task. Use artifact references for large evidence.","inputSchema":{"type":"object","properties":{"idempotencyKey":{"type":"string","minLength":1,"maxLength":128},"message":{"type":"object","properties":{"recipient":{"type":"string"},"kind":{"type":"string","enum":["task_request","evidence","question","answer","critique","result","escalation"],"default":"evidence"},"body":{"type":"string","minLength":1,"maxLength":65536},"replyTo":{"type":["string","null"]},"ttlSeconds":{"type":"integer","minimum":1,"maximum":3600,"default":600}},"required":["recipient","body"],"additionalProperties":false}},"required":["idempotencyKey","message"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}),
        json!({"name":"polymas_list_inbox","description":"Read this agent's delivered messages in the current run. Queued messages enter the next exec turn and are not returned before delivery. Pass the returned cursor as after to read the next page. Expired messages are excluded. This tool does not acknowledge processing.","inputSchema":{"type":"object","properties":{"after":{"type":"integer","minimum":0,"default":0},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20}},"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}),
        json!({"name":"polymas_ack_message","description":"Acknowledge that this agent processed one message already delivered to its context in the current run. Use the exact messageId from the inbox. Repeating the same acknowledgement is safe. Acknowledgement does not accept task results on behalf of a human.","inputSchema":{"type":"object","properties":{"messageId":{"type":"string","minLength":1,"maxLength":200}},"required":["messageId"],"additionalProperties":false},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}),
    ]
}
fn pinned(engine: &Engine, scope: &Scope) -> Result<Configuration> {
    engine.run_configuration(&scope.run_id)
}
fn permits(live: &Configuration, snapshot: &Configuration, from: &str, to: &str) -> bool {
    live.registry
        .agent(to)
        .is_ok_and(|a| a.status != AgentStatus::Archived)
        && snapshot
            .registry
            .agent(to)
            .is_ok_and(|a| a.status != AgentStatus::Archived)
        && live.architecture.permits_message(from, to)
        && snapshot.architecture.permits_message(from, to)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Send {
    idempotency_key: String,
    message: MessageInput,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ack {
    message_id: String,
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Inbox {
    after: i64,
    limit: usize,
}
impl Default for Inbox {
    fn default() -> Self {
        Self {
            after: 0,
            limit: 20,
        }
    }
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Peers {
    query: String,
    cursor: Option<String>,
    limit: usize,
}
impl Default for Peers {
    fn default() -> Self {
        Self {
            query: String::new(),
            cursor: None,
            limit: 20,
        }
    }
}

pub async fn dispatch(
    state: &AppState,
    scope: &Scope,
    name: &str,
    arguments: Value,
) -> Result<Value> {
    let mut engine = state.engine.lock().await;
    validate(&engine, scope)?;
    let timestamp = chrono::Utc::now().timestamp();
    let result = match name {
        "polymas_list_peers" => {
            let input: Peers = serde_json::from_value(arguments)?;
            require(
                input.query.len() <= 200
                    && input.cursor.as_ref().is_none_or(|s| s.len() <= 200)
                    && (1..=100).contains(&input.limit),
                "Некорректный поиск, cursor или размер страницы",
            )?;
            let snapshot = pinned(&engine, scope)?;
            let live = &engine.workspace.config().value;
            let query = input.query.to_lowercase();
            let mut agents: Vec<_> = snapshot
                .registry
                .agents
                .iter()
                .filter(|a| {
                    input.cursor.as_ref().is_none_or(|c| a.id > *c)
                        && permits(live, &snapshot, &scope.agent_id, &a.id)
                        && format!("{} {}", a.name, a.id)
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
            agents.sort_by(|a, b| a.id.cmp(&b.id));
            let has_more = agents.len() > input.limit;
            agents.truncate(input.limit);
            json!({"items":agents.iter().map(|a| json!({"id":a.id,"name":a.name,"roleId":a.role_id,"roleVersion":a.role_version})).collect::<Vec<_>>(),"nextCursor":if has_more {agents.last().map(|a| a.id.clone())}else{None}})
        }
        "polymas_send_message" => {
            let input: Send = serde_json::from_value(arguments)?;
            let snapshot = pinned(&engine, scope)?;
            let live = &engine.workspace.config().value;
            if !permits(live, &snapshot, &scope.agent_id, &input.message.recipient) {
                return Err(Error::Denied(
                    "Нет действующего канала к этому адресату в снимке и текущей архитектуре"
                        .into(),
                ));
            }
            let mut architecture = snapshot.architecture;
            architecture.max_messages_per_run = architecture
                .max_messages_per_run
                .min(live.architecture.max_messages_per_run);
            architecture.message_hop_limit = architecture
                .message_hop_limit
                .min(live.architecture.message_hop_limit);
            let message = Router::send(
                &mut engine.workspace.db,
                &architecture,
                &scope.run_id,
                &scope.agent_id,
                &input.idempotency_key,
                input.message,
                timestamp,
            )?;
            json!({"id":message.id,"state":message.state,"recipient":message.recipient,"expiresAt":message.expires_at,"delivery":"next_turn"})
        }
        "polymas_list_inbox" => {
            let input: Inbox = serde_json::from_value(arguments)?;
            serde_json::to_value(Router::inbox(
                &engine.workspace.db,
                &scope.run_id,
                &scope.agent_id,
                input.after,
                input.limit,
                timestamp,
            )?)?
        }
        "polymas_ack_message" => {
            let input: Ack = serde_json::from_value(arguments)?;
            Router::get_for_recipient(
                &engine.workspace.db,
                &scope.run_id,
                &scope.agent_id,
                &input.message_id,
            )?;
            Router::acknowledge(&mut engine.workspace.db, &scope.agent_id, &input.message_id)?;
            json!({"messageId":input.message_id,"state":"acknowledged"})
        }
        _ => return Err(Error::NotFound("Неизвестный инструмент сообщений".into())),
    };
    require(
        serde_json::to_vec(&result)?.len() <= 2 * 1024 * 1024,
        "Страница inbox превышает 2 МиБ; уменьшите limit",
    )?;
    Ok(result)
}
