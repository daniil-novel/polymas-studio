use crate::error::{Error, Result, require};
use crate::registry::Sandbox;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub const SUPPORTED_CODEX_VERSION: &str = "0.144.1";
pub const MAX_EVENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Default)]
pub struct CodexRequest {
    pub workspace: PathBuf,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub sandbox: Sandbox,
    pub prompt: String,
    pub gateway: Option<McpLaunch>,
    pub timeout_seconds: u64,
    pub environment: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct McpLaunch {
    pub command: PathBuf,
    pub args: Vec<String>,
    pub env_vars: Vec<String>,
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.into()).to_string()
}

pub fn build_arguments(request: &CodexRequest) -> Result<Vec<String>> {
    require(
        request.workspace.is_absolute(),
        "Рабочая папка должна быть абсолютной",
    )?;
    require(
        !request.prompt.is_empty() && request.prompt.len() <= 1024 * 1024,
        "Пустой или слишком большой ввод Codex",
    )?;
    let mut args = vec!["exec".into()];
    if let Some(session) = &request.session_id {
        uuid::Uuid::parse_str(session).map_err(|_| {
            Error::Validation("Для resume нужен точный UUID сессии этого агента".into())
        })?;
        args.push("resume".into());
    }
    args.extend(
        [
            "--json",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
        ]
        .map(str::to_owned),
    );
    for config in [
        "features.multi_agent=false",
        "features.multi_agent_v2=false",
        "approval_policy=\"never\"",
        "web_search=\"disabled\"",
    ] {
        args.extend(["-c".into(), config.into()]);
    }
    let sandbox = match request.sandbox {
        Sandbox::ReadOnly => "read-only",
        Sandbox::WorkspaceWrite => "workspace-write",
    };
    args.extend([
        "-c".into(),
        format!("sandbox_mode={}", toml_string(sandbox)),
    ]);
    if let Some(model) = &request.model {
        require(
            !model.is_empty() && model.len() <= 200 && !model.contains(['\n', '\r']),
            "Некорректная модель",
        )?;
        args.extend(["--model".into(), model.clone()]);
    }
    if let Some(gateway) = &request.gateway {
        require(
            gateway.command.is_absolute(),
            "MCP gateway должен быть абсолютным доверенным executable",
        )?;
        require(
            gateway.env_vars.iter().all(|s| {
                s.starts_with("POLYMAS_") && s.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            }),
            "Некорректное окружение gateway",
        )?;
        let args_value = toml::Value::Array(
            gateway
                .args
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        )
        .to_string();
        let env_value = toml::Value::Array(
            gateway
                .env_vars
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        )
        .to_string();
        for config in [
            format!(
                "mcp_servers.polymas.command={}",
                toml_string(&gateway.command.to_string_lossy())
            ),
            format!("mcp_servers.polymas.args={args_value}"),
            format!("mcp_servers.polymas.env_vars={env_value}"),
            "mcp_servers.polymas.required=true".into(),
        ] {
            args.extend(["-c".into(), config]);
        }
    }
    if let Some(session) = &request.session_id {
        args.push(session.clone());
    }
    args.push("-".into());
    Ok(args)
}

#[derive(Default)]
pub struct JsonlDecoder {
    buffer: Vec<u8>,
}
impl JsonlDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Value>> {
        let mut values = vec![];
        for byte in bytes {
            if *byte == b'\n' {
                if !self.buffer.iter().all(u8::is_ascii_whitespace) {
                    values.push(self.parse()?);
                }
                self.buffer.clear();
            } else {
                require(
                    self.buffer.len() < MAX_EVENT_BYTES,
                    "Превышен максимальный размер JSONL-события",
                )?;
                self.buffer.push(*byte);
            }
        }
        Ok(values)
    }
    fn parse(&self) -> Result<Value> {
        let value: Value = serde_json::from_slice(&self.buffer)
            .map_err(|_| Error::Runtime("Повреждён JSONL поток Codex".into()))?;
        require(
            value.is_object() && value.get("type").is_some_and(Value::is_string),
            "JSONL-событие без type",
        )?;
        Ok(value)
    }
    pub fn finish(&mut self) -> Result<Vec<Value>> {
        let result = if self.buffer.iter().all(u8::is_ascii_whitespace) {
            vec![]
        } else {
            vec![self.parse()?]
        };
        self.buffer.clear();
        Ok(result)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbe {
    pub path: PathBuf,
    pub version: String,
    pub compatible: bool,
    pub authenticated: bool,
    pub auth_method: String,
}

pub fn resolve_codex(explicit: Option<&std::path::Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        require(
            path.is_absolute() && path.is_file(),
            "Укажите существующий абсолютный путь Codex",
        )?;
        require(
            !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("ps1" | "cmd" | "bat")
            ),
            "Укажите бинарник Codex, не shell wrapper",
        )?;
        return Ok(path.canonicalize()?);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    for directory in std::env::split_paths(&path) {
        let binary = directory.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if binary.is_file() {
            return Ok(binary.canonicalize()?);
        }
        if cfg!(windows) {
            for root in [
                directory.join("node_modules/@openai/codex"),
                directory.join("node_modules/.pnpm/node_modules/@openai/codex"),
            ] {
                for relative in [
                    "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
                    "vendor/x86_64-pc-windows-msvc/bin/codex.exe",
                ] {
                    let binary = root.join(relative);
                    if binary.is_file() {
                        return Ok(binary.canonicalize()?);
                    }
                }
            }
        }
    }
    Err(Error::Runtime(
        "Codex CLI не найден. Установите его и укажите путь к бинарнику в настройках.".into(),
    ))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexResult {
    pub session_id: String,
    pub output: String,
    pub usage: Option<Value>,
    pub pid: u32,
}

pub struct CodexStream {
    expected_session: Option<String>,
    session: Option<String>,
    output: String,
    usage: Option<Value>,
    completed: bool,
    failed: bool,
}
impl CodexStream {
    pub fn new(expected_session: Option<String>) -> Self {
        Self {
            expected_session,
            session: None,
            output: String::new(),
            usage: None,
            completed: false,
            failed: false,
        }
    }
    pub fn observe(&mut self, event: &Value) -> Result<()> {
        match event["type"].as_str().unwrap_or("") {
            "thread.started" => {
                let session = event["thread_id"]
                    .as_str()
                    .ok_or_else(|| Error::Runtime("Codex не вернул session ID".into()))?;
                uuid::Uuid::parse_str(session)
                    .map_err(|_| Error::Runtime("Codex вернул некорректный session UUID".into()))?;
                if self
                    .expected_session
                    .as_ref()
                    .is_some_and(|id| id != session)
                    || self.session.as_ref().is_some_and(|id| id != session)
                {
                    return Err(Error::Denied(
                        "Codex вернул другую сессию; попытка прервана".into(),
                    ));
                }
                self.session = Some(session.into());
            }
            "item.completed" if event["item"]["type"] == "agent_message" => {
                self.output = event["item"]["text"].as_str().unwrap_or_default().into();
            }
            "turn.completed" => {
                self.completed = true;
                self.usage = event.get("usage").filter(|v| v.is_object()).cloned();
            }
            "turn.failed" => self.failed = true,
            _ => {}
        }
        Ok(())
    }
    pub fn finish(self) -> Result<CodexResult> {
        require(
            self.completed && !self.failed && !self.output.trim().is_empty(),
            "Codex не подтвердил завершение turn с результатом",
        )?;
        Ok(CodexResult {
            session_id: self
                .session
                .ok_or_else(|| Error::Runtime("Нет подтверждённой сессии".into()))?,
            output: self.output,
            usage: self.usage,
            pid: 0,
        })
    }
}

pub async fn probe_codex(explicit: Option<&std::path::Path>) -> Result<RuntimeProbe> {
    use std::time::Duration;
    let path = resolve_codex(explicit)?;
    async fn output(path: &std::path::Path, args: &[&str]) -> Result<(bool, String)> {
        let mut command = tokio::process::Command::new(path);
        command
            .args(args)
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let output = tokio::time::timeout(Duration::from_secs(10), command.output())
            .await
            .map_err(|_| Error::Timeout("Codex не ответил на проверку за 10 секунд".into()))??;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok((output.status.success(), crate::process::redact(&text)))
    }
    let (ok, version_text) = output(&path, &["--version"]).await?;
    require(
        ok && version_text.starts_with("codex-cli "),
        "Файл не является поддерживаемым Codex CLI",
    )?;
    let version = version_text
        .trim()
        .trim_start_matches("codex-cli ")
        .to_string();
    let (authenticated, auth) = output(&path, &["login", "status"]).await?;
    Ok(RuntimeProbe {
        path,
        compatible: version == SUPPORTED_CODEX_VERSION,
        version,
        authenticated,
        auth_method: if auth.contains("ChatGPT") {
            "chatgpt"
        } else if authenticated {
            "api_key"
        } else {
            "none"
        }
        .into(),
    })
}

fn safe_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(crate::process::redact(text)),
        Value::Array(items) => Value::Array(items.iter().map(safe_value).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if [
                            "access_token",
                            "refresh_token",
                            "api_key",
                            "password",
                            "authorization",
                        ]
                        .contains(&k.to_lowercase().as_str())
                        {
                            Value::String("[REDACTED]".into())
                        } else {
                            safe_value(v)
                        },
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Observable progress is separate from tool data and private model reasoning.
pub fn observable_event(source: &Value) -> Option<crate::process::ProcessEvent> {
    if source["item"]["type"] == "reasoning" {
        return None;
    }
    let kind = match source["type"].as_str().unwrap_or("unknown") {
        "thread.started" => "runtime.session",
        "turn.started" => "runtime.turn_started",
        "turn.completed" => "runtime.turn_completed",
        "turn.failed" | "error" => "runtime.error",
        "item.started" => "runtime.item_started",
        "item.completed" => "runtime.item_completed",
        "item.updated" => "runtime.item_updated",
        _ => "runtime.unknown_event",
    };
    let body = if source["item"]["type"] == "mcp_tool_call" {
        let item = &source["item"];
        let mut summary = serde_json::Map::new();
        for field in ["id", "type", "server", "tool", "status"] {
            if let Some(value) = item[field].as_str() {
                summary.insert(
                    field.into(),
                    Value::String(crate::process::redact(
                        &value.chars().take(200).collect::<String>(),
                    )),
                );
            }
        }
        summary.insert("hasResult".into(), Value::Bool(!item["result"].is_null()));
        summary.insert(
            "hasError".into(),
            Value::Bool(!item["error"].is_null() || item["result"]["isError"] == true),
        );
        serde_json::json!({"type":source["type"],"item":summary})
    } else {
        safe_value(source)
    };
    Some(crate::process::ProcessEvent {
        kind: kind.into(),
        body,
    })
}

pub async fn run_codex(
    executable: PathBuf,
    request: CodexRequest,
    sender: tokio::sync::mpsc::Sender<crate::process::ProcessEvent>,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<CodexResult> {
    use crate::process::{ProcessSpec, run_process};
    let args = build_arguments(&request)?;
    let mut stream = CodexStream::new(request.session_id.clone());
    let (raw_sender, mut receiver) = tokio::sync::mpsc::channel(128);
    let handle = tokio::spawn(run_process(
        ProcessSpec {
            executable,
            args,
            workspace: request.workspace,
            stdin: request.prompt,
            timeout_seconds: request.timeout_seconds.max(1),
            environment: request.environment,
        },
        raw_sender,
        cancel,
    ));
    while let Some(event) = receiver.recv().await {
        let event = if event.kind == "runtime.event" {
            if let Err(error) = stream.observe(&event.body) {
                handle.abort();
                let _ = handle.await;
                return Err(error);
            }
            let Some(observed) = observable_event(&event.body) else {
                continue;
            };
            observed
        } else {
            event
        };
        if !matches!(
            tokio::time::timeout(std::time::Duration::from_secs(5), sender.send(event)).await,
            Ok(Ok(()))
        ) {
            handle.abort();
            let _ = handle.await;
            return Err(Error::Runtime("Журнал не принял события Codex".into()));
        }
    }
    let outcome = handle
        .await
        .map_err(|e| Error::Runtime(format!("Supervisor завершился: {e}")))??;
    if outcome.cancelled {
        return Err(Error::Cancelled(
            "Codex остановлен; возможные внешние действия требуют проверки".into(),
        ));
    }
    if outcome.timed_out {
        return Err(Error::Timeout("Превышено время выполнения Codex".into()));
    }
    if !outcome.success {
        return Err(Error::Runtime(format!(
            "Codex завершился с кодом {:?}: {}",
            outcome.exit_code, outcome.stderr_tail
        )));
    }
    let mut result = stream.finish()?;
    result.pid = outcome.pid;
    Ok(result)
}
