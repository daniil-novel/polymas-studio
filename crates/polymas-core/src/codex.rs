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
