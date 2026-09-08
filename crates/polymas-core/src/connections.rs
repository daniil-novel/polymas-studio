use crate::error::{Result, require};
use crate::registry::id;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    #[default]
    Unconfigured,
    Ready,
    Degraded,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConnectionTransport {
    Files { roots: Vec<PathBuf> },
    Stdio { command: PathBuf, args: Vec<String> },
    Http { url: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub recipe: String,
    pub transport: ConnectionTransport,
    pub enabled: bool,
    pub health: Health,
    pub checked_at: Option<String>,
    pub last_error: Option<String>,
    pub auth_secret_ref: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub secret_environment: BTreeMap<String, String>,
    pub tools: Vec<ToolDescription>,
    pub tool_allowlist: Vec<String>,
    pub read_only: bool,
    pub timeout_seconds: u64,
}
impl Connection {
    pub fn files(name: &str, roots: Vec<PathBuf>) -> Self {
        Self {
            id: id("connection"),
            name: name.into(),
            recipe: "files".into(),
            transport: ConnectionTransport::Files { roots },
            enabled: false,
            health: Health::Unconfigured,
            checked_at: None,
            last_error: None,
            auth_secret_ref: None,
            environment: BTreeMap::new(),
            secret_environment: BTreeMap::new(),
            tools: vec![],
            tool_allowlist: vec![],
            read_only: true,
            timeout_seconds: 30,
        }
    }
    pub fn validate(&self) -> Result<()> {
        require(
            !self.name.trim().is_empty() && self.name.len() <= 200,
            "Название подключения обязательно",
        )?;
        require(
            (1..=300).contains(&self.timeout_seconds),
            "Таймаут MCP: 1–300 секунд",
        )?;
        match &self.transport {
            ConnectionTransport::Files { roots } => {
                require(
                    !roots.is_empty() && roots.len() <= 32,
                    "Выберите от 1 до 32 разрешённых папок",
                )?;
                for root in roots {
                    require(
                        root.is_absolute() && root.is_dir(),
                        "Разрешённая папка не существует",
                    )?;
                }
                require(
                    self.read_only,
                    "Встроенный файловый источник поддерживает только чтение",
                )?;
            }
            ConnectionTransport::Stdio { command, args } => {
                require(
                    command.is_absolute() && command.is_file(),
                    "Для STDIO нужен абсолютный путь установленного executable",
                )?;
                let filename = command
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                require(
                    ![
                        "powershell.exe",
                        "pwsh.exe",
                        "cmd.exe",
                        "bash.exe",
                        "sh.exe",
                        "bash",
                        "sh",
                        "pwsh",
                    ]
                    .contains(&filename.as_str())
                        && !matches!(
                            command.extension().and_then(|s| s.to_str()),
                            Some("ps1" | "cmd" | "bat")
                        ),
                    "Укажите MCP executable или node/python, не shell-команду",
                )?;
                require(
                    args.len() <= 64 && args.iter().all(|arg| arg.len() <= 16_384),
                    "Слишком много аргументов MCP",
                )?;
            }
            ConnectionTransport::Http { url } => {
                let parsed = url::Url::parse(url)
                    .map_err(|_| crate::error::Error::Validation("Некорректный MCP URL".into()))?;
                require(
                    parsed.username().is_empty()
                        && parsed.password().is_none()
                        && parsed.fragment().is_none(),
                    "Credentials и fragment запрещены в URL; используйте поле секрета",
                )?;
                let local = matches!(
                    parsed.host_str(),
                    Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
                );
                require(
                    parsed.scheme() == "https" || (parsed.scheme() == "http" && local),
                    "Удалённый MCP требует HTTPS",
                )?;
                require(
                    parsed.query_pairs().all(|(k, _)| {
                        !k.to_lowercase().contains("token")
                            && !k.to_lowercase().contains("key")
                            && !k.to_lowercase().contains("password")
                    }),
                    "Секреты в query-параметрах запрещены",
                )?;
            }
        }
        for key in self
            .environment
            .keys()
            .chain(self.secret_environment.keys())
        {
            require(
                !key.is_empty()
                    && key.len() <= 100
                    && key
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                "Некорректное имя переменной MCP",
            )?;
        }
        for (key, value) in &self.environment {
            require(
                !["PASSWORD", "TOKEN", "KEY", "SECRET"]
                    .iter()
                    .any(|s| key.contains(s))
                    && value.len() <= 16_384,
                "Чувствительные переменные задаются через ссылки на секреты",
            )?;
        }
        for reference in self
            .secret_environment
            .values()
            .chain(self.auth_secret_ref.iter())
        {
            require(
                reference.starts_with("secret_") && reference.len() < 100,
                "Некорректная ссылка на секрет",
            )?;
        }
        Ok(())
    }
    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        if enabled {
            self.validate()?;
            require(
                self.health == Health::Ready,
                "Сначала проверьте подключение в мастере",
            )?;
        }
        self.enabled = enabled;
        Ok(())
    }
    pub fn allows_tool(&self, name: &str) -> bool {
        self.enabled
            && self.health == Health::Ready
            && self.tool_allowlist.iter().any(|tool| tool == name)
            && self
                .tools
                .iter()
                .any(|tool| tool.name == name && (!self.read_only || tool.read_only))
    }
}

pub fn files_tools() -> Vec<ToolDescription> {
    [("read_text","Читать UTF-8 файл из разрешённой папки"),("list_files","Список файлов разрешённой папки")].into_iter().map(|(name,description)|ToolDescription {name:name.into(),description:description.into(),input_schema:json!({"type":"object","properties":{"root":{"type":"integer","minimum":0},"path":{"type":"string"}},"required":["root","path"],"additionalProperties":false}),read_only:true}).collect()
}
fn relative_path(path: &str) -> Result<&Path> {
    let relative = Path::new(path);
    require(
        !relative.is_absolute()
            && !path.contains(':')
            && !path.contains('\\')
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "Используйте относительный путь без перехода к родителю",
    )?;
    Ok(relative)
}
fn root_dir(roots: &[PathBuf], index: usize) -> Result<cap_std::fs::Dir> {
    let root = roots
        .get(index)
        .ok_or_else(|| crate::error::Error::Denied("Папка не разрешена".into()))?;
    Ok(cap_std::fs::Dir::open_ambient_dir(
        root,
        cap_std::ambient_authority(),
    )?)
}
pub fn read_file(roots: &[PathBuf], index: usize, path: &str) -> Result<String> {
    let path = relative_path(path)?;
    let file = root_dir(roots, index)?.open(path)?;
    require(
        file.metadata()?.is_file(),
        "Источник должен быть обычным файлом",
    )?;
    let mut bytes = vec![];
    file.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    require(
        bytes.len() <= 4 * 1024 * 1024,
        "Файл превышает 4 МиБ; разбейте источник на части",
    )?;
    String::from_utf8(bytes).map_err(|_| {
        crate::error::Error::Validation(
            "Формат не поддерживается этим инструментом: нужен UTF-8 текст".into(),
        )
    })
}
pub fn list_files(roots: &[PathBuf], index: usize, path: &str) -> Result<Value> {
    let path = relative_path(if path.is_empty() { "." } else { path })?;
    let directory = root_dir(roots, index)?.open_dir(path)?;
    let mut entries = vec![];
    let mut truncated = false;
    for entry in directory.entries()? {
        let entry = entry?;
        if entries.len() == 1000 {
            truncated = true;
            break;
        }
        entries.push(json!({"name":entry.file_name().to_string_lossy(),"directory":entry.file_type()?.is_dir()}));
    }
    Ok(json!({"entries":entries,"truncated":truncated}))
}
