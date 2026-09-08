use crate::connections::{Connection, Health};
use crate::error::{Error, Result, require};
use crate::policy::{Binding, Subject};
use crate::registry::{AgentStatus, BatchRequest, Registry, RoleDraft, id, now};
use crate::storage::{EventInput, Store, Stored};
use crate::topology::{Architecture, Preset};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub registry: Registry,
    pub architecture: Architecture,
    pub bindings: Vec<Binding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConfigCommand {
    RenameWorkspace {
        name: String,
    },
    CreateRole {
        draft: RoleDraft,
    },
    ReviseRole {
        role_id: String,
        expected_version: u32,
        draft: RoleDraft,
    },
    CreateAgents {
        key: String,
        request: BatchRequest,
    },
    PinRole {
        agent_ids: Vec<String>,
        role_id: String,
        version: u32,
    },
    RenameAgent {
        agent_id: String,
        name: String,
    },
    ArchiveAgent {
        agent_id: String,
    },
    ArchiveAgents {
        agent_ids: Vec<String>,
    },
    SetArchitecture {
        architecture: Architecture,
    },
    SetNodePosition {
        node_id: String,
        position: Option<[f32; 2]>,
    },
    SetBindings {
        bindings: Vec<Binding>,
    },
}
impl ConfigCommand {
    fn kind(&self) -> &'static str {
        match self {
            Self::RenameWorkspace { .. } => "workspace.renamed",
            Self::CreateRole { .. } => "role.created",
            Self::ReviseRole { .. } => "role.revised",
            Self::CreateAgents { .. } => "agents.created",
            Self::PinRole { .. } => "agents.role_pinned",
            Self::RenameAgent { .. } => "agent.renamed",
            Self::ArchiveAgent { .. } => "agent.archived",
            Self::ArchiveAgents { .. } => "agents.archived",
            Self::SetArchitecture { .. } => "architecture.changed",
            Self::SetNodePosition { .. } => "graph.position_changed",
            Self::SetBindings { .. } => "bindings.changed",
        }
    }
    pub(crate) fn archived_agent_ids(&self) -> Option<&[String]> {
        match self {
            Self::ArchiveAgent { agent_id } => Some(std::slice::from_ref(agent_id)),
            Self::ArchiveAgents { agent_ids } => Some(agent_ids),
            _ => None,
        }
    }
}

impl Configuration {
    fn archive_agents(&mut self, agent_ids: &[String]) -> Result<()> {
        self.registry.archive_many(agent_ids)?;
        let ids: HashSet<_> = agent_ids.iter().collect();
        self.architecture
            .edges
            .retain(|edge| !ids.contains(&edge.from) && !ids.contains(&edge.to));
        self.architecture
            .positions
            .retain(|id, _| !ids.contains(id));
        for holon in &mut self.architecture.holons {
            holon.members.retain(|id| !ids.contains(id));
            if holon
                .coordinator_agent_id
                .as_ref()
                .is_some_and(|id| ids.contains(id))
            {
                holon.coordinator_agent_id = None;
            }
        }
        if self
            .architecture
            .coordinator_agent_id
            .as_ref()
            .is_some_and(|id| ids.contains(id))
        {
            self.architecture.coordinator_agent_id = None;
            self.architecture.preset = Preset::Custom;
        }
        self.bindings
            .retain(|binding| !matches!(&binding.subject, Subject::Agent {id} if ids.contains(id)));
        let remaining = self
            .registry
            .agents
            .iter()
            .filter(|agent| agent.status != AgentStatus::Archived)
            .count();
        self.architecture.max_concurrency = self.architecture.max_concurrency.min(remaining.max(1));
        if remaining == 0 {
            self.architecture.preset = Preset::Network;
        }
        if remaining == 1 {
            self.architecture.coordinator_agent_id = None;
            if self.architecture.preset == Preset::Star {
                self.architecture.preset = Preset::Custom;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub revision: i64,
    pub previous_revision: i64,
    pub author: String,
    pub timestamp: String,
    pub reason: String,
    pub configuration: Configuration,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Stored<Configuration>,
    pub connections: Vec<Stored<Connection>>,
    pub cursor: i64,
}
pub struct Workspace {
    pub db: Store,
    pub root: PathBuf,
    config: Stored<Configuration>,
    _lock: std::fs::File,
}
impl Workspace {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("workspace.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|_| Error::Conflict("Workspace уже открыт другой службой".into()))?;
        let mut db = Store::open(&root.join("state.sqlite3"))?;
        let config =
            if let Some(existing) = db.get::<Configuration>("workspace", "configuration")? {
                existing
            } else {
                let configuration = Configuration {
                    schema_version: 1,
                    id: id("workspace"),
                    name: "Моя система".into(),
                    created_at: now(),
                    registry: Registry::default(),
                    architecture: Architecture::default(),
                    bindings: vec![],
                };
                let revision = db.transaction(|tx| {
                    let revision = tx.put("workspace", "configuration", 0, &configuration)?;
                    tx.put(
                        "configuration_history",
                        "1",
                        0,
                        &HistoryEntry {
                            revision,
                            previous_revision: 0,
                            author: "local:owner".into(),
                            timestamp: now(),
                            reason: "workspace.created".into(),
                            configuration: configuration.clone(),
                        },
                    )?;
                    tx.emit(EventInput::new(
                        "workspace.created",
                        json!({"workspaceId":configuration.id,"revision":revision}),
                    ))?;
                    Ok(revision)
                })?;
                Stored {
                    revision,
                    value: configuration,
                }
            };
        require(
            config.value.schema_version == 1,
            "Workspace создан более новой версией",
        )?;
        config.value.architecture.validate(&config.value.registry)?;
        std::fs::create_dir_all(root.join("agents"))?;
        std::fs::create_dir_all(root.join("artifacts"))?;
        Ok(Self {
            db,
            root,
            config,
            _lock: lock,
        })
    }
    pub fn config(&self) -> &Stored<Configuration> {
        &self.config
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        Ok(Snapshot {
            config: self.config.clone(),
            connections: self.db.list("connections", 1000)?,
            cursor: self.db.last_sequence()?,
        })
    }
    pub fn history(&self) -> Result<Vec<Stored<HistoryEntry>>> {
        self.db.list("configuration_history", 1000)
    }
    pub fn configure(&mut self, expected: i64, command: ConfigCommand) -> Result<Value> {
        if expected != self.config.revision {
            return Err(Error::Conflict(
                "Конфигурация изменилась; обновите редактор".into(),
            ));
        }
        let mut candidate = self.config.value.clone();
        let reason = command.kind();
        let result = match command {
            ConfigCommand::RenameWorkspace { name } => {
                require(
                    !name.trim().is_empty() && name.len() <= 200,
                    "Название workspace обязательно",
                )?;
                candidate.name = name;
                json!({})
            }
            ConfigCommand::CreateRole { draft } => {
                serde_json::to_value(candidate.registry.create_role(draft)?)?
            }
            ConfigCommand::ReviseRole {
                role_id,
                expected_version,
                draft,
            } => serde_json::to_value(candidate.registry.revise_role(
                &role_id,
                expected_version,
                draft,
            )?)?,
            ConfigCommand::CreateAgents { key, request } => {
                json!({"ids":candidate.registry.create_batch(&key,request)?})
            }
            ConfigCommand::PinRole {
                agent_ids,
                role_id,
                version,
            } => {
                candidate.registry.pin_role(&agent_ids, &role_id, version)?;
                json!({})
            }
            ConfigCommand::RenameAgent { agent_id, name } => {
                require(
                    !name.trim().is_empty() && name.len() <= 200,
                    "Название агента обязательно",
                )?;
                candidate.registry.agent_mut(&agent_id)?.name = name;
                json!({})
            }
            ConfigCommand::ArchiveAgent { agent_id } => {
                candidate.archive_agents(&[agent_id])?;
                json!({})
            }
            ConfigCommand::ArchiveAgents { agent_ids } => {
                candidate.archive_agents(&agent_ids)?;
                json!({"ids":agent_ids})
            }
            ConfigCommand::SetArchitecture { architecture } => {
                candidate.architecture = architecture;
                json!({})
            }
            ConfigCommand::SetNodePosition { node_id, position } => {
                let is_agent = candidate
                    .registry
                    .agents
                    .iter()
                    .any(|agent| agent.id == node_id && agent.status != AgentStatus::Archived);
                let is_memory = candidate.architecture.memory_enabled
                    && candidate
                        .architecture
                        .memory_connection_id
                        .as_ref()
                        .is_some_and(|connection| node_id == format!("memory:{connection}"));
                require(is_agent || is_memory, "Узел графа больше не существует")?;
                if let Some(xy) = position {
                    candidate.architecture.positions.insert(node_id.clone(), xy);
                } else {
                    candidate.architecture.positions.remove(&node_id);
                }
                json!({"nodeId":node_id,"position":position})
            }
            ConfigCommand::SetBindings { bindings } => {
                require(bindings.len() <= 25000, "Слишком много bindings")?;
                let mut seen = HashSet::new();
                for binding in &bindings {
                    binding.validate()?;
                    require(seen.insert(&binding.id), "Повторный binding ID")?;
                    let connection = self
                        .db
                        .get::<Connection>("connections", &binding.connection_id)?
                        .ok_or_else(|| Error::NotFound("MCP не найден".into()))?
                        .value;
                    require(
                        binding
                            .tools
                            .iter()
                            .all(|name| connection.tools.iter().any(|t| &t.name == name)),
                        "В binding есть неизвестный инструмент",
                    )?;
                    match &binding.subject {
                        Subject::Agent { id } => {
                            candidate.registry.agent(id)?;
                        }
                        Subject::Role { id } => {
                            require(
                                candidate.registry.roles.iter().any(|r| &r.id == id),
                                "Роль binding не найдена",
                            )?;
                        }
                    }
                }
                candidate.bindings = bindings;
                json!({})
            }
        };
        candidate.architecture.validate(&candidate.registry)?;
        if let Some(connection_id) = &candidate.architecture.memory_connection_id {
            require(
                self.db
                    .get::<Connection>("connections", connection_id)?
                    .is_some(),
                "Подключение общей памяти не найдено",
            )?;
        }
        // History is part of the same commit as the new revision and event.
        let revision = self.db.transaction(|tx| {
            let revision = tx.put("workspace", "configuration", expected, &candidate)?;
            tx.put(
                "configuration_history",
                &revision.to_string(),
                0,
                &HistoryEntry {
                    revision,
                    previous_revision: expected,
                    author: "local:owner".into(),
                    timestamp: now(),
                    reason: reason.into(),
                    configuration: candidate.clone(),
                },
            )?;
            tx.emit(EventInput::new(
                reason,
                json!({"revision":revision,"previousRevision":expected,"result":result}),
            ))?;
            Ok(revision)
        })?;
        self.config = Stored {
            revision,
            value: candidate,
        };
        Ok(result)
    }
    pub fn save_connection(
        &mut self,
        expected: i64,
        mut connection: Connection,
    ) -> Result<Stored<Connection>> {
        require(
            !connection.id.is_empty() && connection.id.len() <= 200,
            "Некорректный MCP ID",
        )?;
        connection.validate()?;
        // Only a server-side probe may populate health and discovered tools.
        connection.enabled = false;
        connection.health = Health::Unconfigured;
        connection.checked_at = None;
        connection.last_error = None;
        connection.tools.clear();
        let revision = self.db.transaction(|tx| {
            let revision = tx.put("connections", &connection.id, expected, &connection)?;
            tx.emit(EventInput::new(
                "mcp.configuration_saved",
                json!({"connectionId":connection.id,"revision":revision}),
            ))?;
            Ok(revision)
        })?;
        Ok(Stored {
            revision,
            value: connection,
        })
    }
    pub fn connection(&self, id: &str) -> Result<Stored<Connection>> {
        self.db
            .get("connections", id)?
            .ok_or_else(|| Error::NotFound("MCP не найден".into()))
    }
    pub fn agent_workspace(&self, agent_id: &str) -> Result<PathBuf> {
        let agent = self.config.value.registry.agent(agent_id)?;
        require(
            agent.workspace_key == agent.id
                && agent.id.starts_with("agent_")
                && uuid::Uuid::parse_str(&agent.id[6..]).is_ok(),
            "Некорректный workspace агента",
        )?;
        let parent = self.root.join("agents");
        require(
            parent.canonicalize()? == parent,
            "Папка агентов не должна быть перенаправлена",
        )?;
        let path = parent.join(&agent.workspace_key);
        if !path.exists() {
            std::fs::create_dir(&path)?;
        }
        let actual = path.canonicalize()?;
        require(
            actual == path,
            "Workspace агента не должен быть ссылкой на другую папку",
        )?;
        Ok(actual)
    }
}
