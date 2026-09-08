//! Owner-authored configuration only; transport values and runtime identity never cross this boundary.
use crate::{
    connections::{Connection, ConnectionTransport},
    error::{Result, require},
    policy::Binding,
    registry::{AgentStatus, RoleDraft},
    topology::Architecture,
    workspace::Snapshot,
};
use serde::Serialize;
use std::io::Write;

pub const MAX_EXPORT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EXPORT_CONNECTIONS: usize = 1000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Export<'a> {
    format: &'static str,
    schema_version: &'static str,
    migration_version: u32,
    source_revision: i64,
    name: &'a str,
    runtime: Runtime,
    role_versions: Vec<Role<'a>>,
    instances: Vec<Instance<'a>>,
    architecture: Architecture,
    connections: Vec<Placeholder<'a>>,
    bindings: &'a [Binding],
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    adapter: &'static str,
    setup_required: bool,
}
#[derive(Serialize)]
struct Role<'a> {
    id: &'a str,
    version: u32,
    #[serde(flatten)]
    draft: &'a RoleDraft,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Instance<'a> {
    id: &'a str,
    name: &'a str,
    role_id: &'a str,
    role_version: u32,
    archived: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Placeholder<'a> {
    id: &'a str,
    name: &'a str,
    recipe: &'a str,
    transport: &'static str,
    read_only: bool,
    timeout_seconds: u64,
    tool_allowlist: &'a [String],
    desired_enabled: bool,
    setup_required: bool,
    setup_fields: Vec<&'static str>,
    root_count: usize,
}
impl<'a> From<&'a Connection> for Placeholder<'a> {
    fn from(connection: &'a Connection) -> Self {
        let (transport, mut setup_fields, root_count) = match &connection.transport {
            ConnectionTransport::Files { roots } => ("files", vec!["roots"], roots.len()),
            ConnectionTransport::Http { .. } => ("http", vec!["url"], 0),
            ConnectionTransport::Stdio { .. } => ("stdio", vec!["command", "args"], 0),
        };
        if connection.auth_secret_ref.is_some() {
            setup_fields.push("authentication");
        }
        if !connection.environment.is_empty() {
            setup_fields.push("environment");
        }
        if !connection.secret_environment.is_empty() {
            setup_fields.push("secret_environment");
        }
        Self {
            id: &connection.id,
            name: &connection.name,
            recipe: &connection.recipe,
            transport,
            read_only: connection.read_only,
            timeout_seconds: connection.timeout_seconds,
            tool_allowlist: &connection.tool_allowlist,
            desired_enabled: connection.enabled,
            setup_required: true,
            setup_fields,
            root_count,
        }
    }
}

/// Serialize into a bounded buffer; no partial document is returned on any error.
/// Callers must supply a complete, consistently read connection list, never a UI page.
pub fn configuration_export(snapshot: &Snapshot) -> Result<Vec<u8>> {
    let config = &snapshot.config.value;
    require(
        config.schema_version == 1 && snapshot.config.revision > 0,
        "Несовместимая версия конфигурации",
    )?;
    require(
        snapshot.connections.len() <= MAX_EXPORT_CONNECTIONS,
        "Экспорт поддерживает до 1000 подключений; файл не создан",
    )?;
    require(
        config.registry.agents.len() <= 100_000 && config.registry.roles.len() <= 100_000,
        "Экспорт поддерживает до 100000 определений каждого типа; файл не создан",
    )?;
    let mut architecture = config.architecture.clone();
    architecture.paused = true;
    let document = Export {
        format: "polymas-configuration",
        schema_version: "0.1.0",
        migration_version: 1,
        source_revision: snapshot.config.revision,
        name: &config.name,
        runtime: Runtime {
            adapter: "codex-exec",
            setup_required: true,
        },
        role_versions: config
            .registry
            .roles
            .iter()
            .map(|role| Role {
                id: &role.id,
                version: role.version,
                draft: &role.draft,
            })
            .collect(),
        instances: config
            .registry
            .agents
            .iter()
            .map(|agent| Instance {
                id: &agent.id,
                name: &agent.name,
                role_id: &agent.role_id,
                role_version: agent.role_version,
                archived: agent.status == AgentStatus::Archived,
            })
            .collect(),
        architecture,
        connections: snapshot
            .connections
            .iter()
            .map(|connection| Placeholder::from(&connection.value))
            .collect(),
        bindings: &config.bindings,
    };
    let mut output = BoundedBuffer(Vec::new());
    serde_json::to_writer_pretty(&mut output, &document)?;
    Ok(output.0)
}
struct BoundedBuffer(Vec<u8>);
impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_EXPORT_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other(
                "Экспорт превышает 16 МиБ; файл не создан",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
