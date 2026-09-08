use crate::connections::{
    Connection, ConnectionTransport, ToolDescription, files_tools, list_files, read_file,
};
use crate::error::{Error, Result, require};
use crate::process::{ProcessSpec, command_for, redact};
use rmcp::{
    RoleClient, ServiceExt,
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, TokioChildProcess,
        streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub tools: Vec<ToolDescription>,
    pub protocol: String,
    pub server: Value,
}
type Client = RunningService<RoleClient, ()>;

async fn connect(
    connection: &Connection,
    bearer: Option<String>,
    environment: BTreeMap<String, String>,
) -> Result<Client> {
    connection.validate()?;
    let deadline = Duration::from_secs(connection.timeout_seconds);
    let client = match &connection.transport {
        ConnectionTransport::Stdio { command, args } => {
            let mut env = connection.environment.clone();
            env.extend(environment);
            let mut process = command_for(&ProcessSpec {
                executable: command.clone(),
                args: args.clone(),
                workspace: std::env::temp_dir(),
                stdin: String::new(),
                timeout_seconds: connection.timeout_seconds,
                environment: env,
            });
            process.stderr(std::process::Stdio::null());
            let transport = TokioChildProcess::builder(process)
                .stderr(std::process::Stdio::null())
                .spawn()?
                .0;
            tokio::time::timeout(deadline, ().serve(transport))
                .await
                .map_err(|_| Error::Timeout("MCP initialize timeout".into()))?
        }
        ConnectionTransport::Http { url } => {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                // Audited write operations must not be retransmitted by HTTP policy.
                .retry(reqwest::retry::never())
                .connect_timeout(Duration::from_secs(10))
                .timeout(deadline)
                .build()
                .map_err(|e| Error::Runtime(redact(&e.to_string())))?;
            let mut config = StreamableHttpClientTransportConfig::with_uri(url.clone())
                .max_concurrent_requests(1);
            config.reinit_on_expired_session = false;
            config.max_sse_event_size = 1024 * 1024;
            if let Some(token) = bearer {
                config = config.auth_header(token);
            }
            let transport = StreamableHttpClientTransport::with_client(client, config);
            tokio::time::timeout(deadline, ().serve(transport))
                .await
                .map_err(|_| Error::Timeout("MCP initialize timeout".into()))?
        }
        ConnectionTransport::Files { .. } => {
            return Err(Error::Validation(
                "Файлы обслуживаются локальным gateway".into(),
            ));
        }
    }
    .map_err(|e| Error::Runtime(redact(&e.to_string())))?;
    // Default SDK stale-response caching is inappropriate for an audited tool gateway.
    client
        .set_response_cache_config(rmcp::service::ClientCacheConfig::disabled())
        .await;
    Ok(client)
}

pub async fn probe_connection(
    connection: &Connection,
    bearer: Option<String>,
    environment: BTreeMap<String, String>,
) -> Result<ProbeResult> {
    connection.validate()?;
    if let ConnectionTransport::Files { roots } = &connection.transport {
        for index in 0..roots.len() {
            list_files(roots, index, ".")?;
        }
        return Ok(ProbeResult {
            tools: files_tools(),
            protocol: "builtin MCP gateway".into(),
            server: json!({"name":"PolyMAS Files","version":"1"}),
        });
    }
    let mut client = connect(connection, bearer, environment).await?;
    let result = tokio::time::timeout(
        Duration::from_secs(connection.timeout_seconds),
        client.list_all_tools(),
    )
    .await;
    let server = client
        .peer_info()
        .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
        .unwrap_or(Value::Null);
    let _ = client.close_with_timeout(Duration::from_secs(3)).await;
    let tools = result
        .map_err(|_| Error::Timeout("MCP tools/list timeout".into()))?
        .map_err(|e| Error::Runtime(redact(&e.to_string())))?;
    require(tools.len() <= 1000, "MCP вернул слишком много инструментов")?;
    let tools = tools
        .into_iter()
        .map(|tool| ToolDescription {
            name: tool.name.to_string(),
            description: tool.description.map(|s| s.to_string()).unwrap_or_default(),
            input_schema: Value::Object((*tool.input_schema).clone()),
            // Metadata is descriptive; only vetted read operations get this policy flag.
            read_only: connection.recipe == "neo4j"
                && ["read_neo4j_cypher", "get_neo4j_schema"].contains(&tool.name.as_ref()),
        })
        .collect();
    Ok(ProbeResult {
        tools,
        protocol: server["protocolVersion"]
            .as_str()
            .unwrap_or("negotiated")
            .into(),
        server,
    })
}

pub async fn call_tool(
    connection: &Connection,
    name: &str,
    arguments: Value,
    bearer: Option<String>,
    environment: BTreeMap<String, String>,
) -> Result<Value> {
    require(
        connection.allows_tool(name),
        "Инструмент выключен или не входит в разрешённый список",
    )?;
    require(
        arguments.is_object() && serde_json::to_vec(&arguments)?.len() <= 64 * 1024,
        "Некорректные аргументы инструмента",
    )?;
    if let ConnectionTransport::Files { roots } = &connection.transport {
        let index = arguments["root"]
            .as_u64()
            .ok_or_else(|| Error::Validation("Выберите номер разрешённой папки".into()))?
            as usize;
        let path = arguments["path"]
            .as_str()
            .ok_or_else(|| Error::Validation("Укажите относительный путь".into()))?;
        let text = match name {
            "read_text" => read_file(roots, index, path)?,
            "list_files" => serde_json::to_string(&list_files(roots, index, path)?)?,
            _ => return Err(Error::Denied("Неизвестный файловый инструмент".into())),
        };
        return Ok(json!({"content":[{"type":"text","text":text}],"isError":false}));
    }
    let mut client = connect(connection, bearer, environment).await?;
    let request = rmcp::model::CallToolRequestParams::new(name.to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    let result = tokio::time::timeout(
        Duration::from_secs(connection.timeout_seconds),
        client.call_tool(request),
    )
    .await;
    let _ = client.close_with_timeout(Duration::from_secs(3)).await;
    let result = result
        .map_err(|_| {
            Error::Timeout("MCP call timeout: внешний результат может быть неизвестен".into())
        })?
        .map_err(|e| Error::Runtime(redact(&e.to_string())))?;
    let value = serde_json::to_value(result)?;
    require(
        serde_json::to_vec(&value)?.len() <= 4 * 1024 * 1024,
        "Ответ MCP превышает 4 МиБ",
    )?;
    Ok(value)
}
