use crate::error::{Error, Result, require};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const MAX_AGENTS: usize = 5000;
pub fn id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4())
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct RoleDraft {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub output_contract: String,
    pub acceptance_criteria: String,
    pub escalation_rule: String,
    pub model: Option<String>,
    pub sandbox: Sandbox,
    pub timeout_seconds: u64,
    pub max_turns: u32,
    pub max_output_tokens: Option<u64>,
}

impl Default for RoleDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            instruction: String::new(),
            output_contract: "markdown".into(),
            acceptance_criteria: String::new(),
            escalation_rule: "При нехватке данных сообщи об ограничениях оператору.".into(),
            model: None,
            sandbox: Sandbox::ReadOnly,
            timeout_seconds: 600,
            max_turns: 20,
            max_output_tokens: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Sandbox {
    #[default]
    ReadOnly,
    WorkspaceWrite,
}

impl RoleDraft {
    pub fn validate(&self) -> Result<()> {
        require(
            !self.name.trim().is_empty() && self.name.len() <= 200,
            "Название роли: от 1 до 200 байт",
        )?;
        require(
            !self.instruction.trim().is_empty() && self.instruction.len() <= 100_000,
            "Инструкция роли обязательна (до 100 КБ)",
        )?;
        require(
            (5..=86_400).contains(&self.timeout_seconds),
            "Таймаут: 5–86400 секунд",
        )?;
        require((1..=1000).contains(&self.max_turns), "Число turn: 1–1000")?;
        require(
            self.description.len() <= 10_000
                && self.acceptance_criteria.len() <= 20_000
                && self.escalation_rule.len() <= 10_000,
            "Слишком большой текст роли",
        )?;
        if let Some(model) = &self.model {
            require(
                !model.is_empty() && model.len() <= 200 && !model.contains(['\n', '\r']),
                "Некорректная модель",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoleVersion {
    pub id: String,
    pub version: u32,
    pub created_at: String,
    #[serde(flatten)]
    pub draft: RoleDraft,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    #[default]
    Idle,
    Queued,
    Running,
    Waiting,
    Stopping,
    Failed,
    Stale,
    Archived,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInstance {
    pub id: String,
    pub name: String,
    pub role_id: String,
    pub role_version: u32,
    pub workspace_key: String,
    pub session_id: Option<String>,
    pub owning_holon_id: Option<String>,
    pub status: AgentStatus,
    pub created_at: String,
    pub forked_from: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRequest {
    pub role_id: String,
    pub role_version: u32,
    pub count: usize,
    pub name_pattern: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub roles: Vec<RoleVersion>,
    pub agents: Vec<AgentInstance>,
    #[serde(default)]
    batch_receipts: BTreeMap<String, (BatchRequest, Vec<String>)>,
}

impl Registry {
    pub fn create_role(&mut self, draft: RoleDraft) -> Result<RoleVersion> {
        draft.validate()?;
        let version = RoleVersion {
            id: id("role"),
            version: 1,
            created_at: now(),
            draft,
        };
        self.roles.push(version.clone());
        Ok(version)
    }
    pub fn role(&self, id: &str, version: u32) -> Result<&RoleVersion> {
        self.roles
            .iter()
            .find(|r| r.id == id && r.version == version)
            .ok_or_else(|| Error::NotFound("Версия роли не найдена".into()))
    }
    pub fn agent(&self, id: &str) -> Result<&AgentInstance> {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::NotFound("Агент не найден".into()))
    }
    pub fn agent_mut(&mut self, id: &str) -> Result<&mut AgentInstance> {
        self.agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::NotFound("Агент не найден".into()))
    }
    pub fn revise_role(
        &mut self,
        role_id: &str,
        expected_version: u32,
        draft: RoleDraft,
    ) -> Result<RoleVersion> {
        draft.validate()?;
        let latest = self
            .roles
            .iter()
            .filter(|r| r.id == role_id)
            .map(|r| r.version)
            .max()
            .ok_or_else(|| Error::NotFound("Роль не найдена".into()))?;
        if expected_version != latest {
            return Err(Error::Conflict("Роль изменилась; обновите редактор".into()));
        }
        let version = RoleVersion {
            id: role_id.into(),
            version: latest + 1,
            created_at: now(),
            draft,
        };
        self.roles.push(version.clone());
        Ok(version)
    }
    pub fn create_batch(&mut self, key: &str, request: BatchRequest) -> Result<Vec<String>> {
        require(
            !key.is_empty() && key.len() <= 128,
            "Некорректный ключ операции",
        )?;
        if let Some((old, ids)) = self.batch_receipts.get(key) {
            return if old == &request {
                Ok(ids.clone())
            } else {
                Err(Error::Conflict(
                    "Ключ уже использован для другой операции".into(),
                ))
            };
        }
        self.role(&request.role_id, request.role_version)?;
        let existing = self
            .agents
            .iter()
            .filter(|a| a.status != AgentStatus::Archived)
            .count();
        require(
            request.count > 0 && request.count <= MAX_AGENTS.saturating_sub(existing),
            "Допускается от 1 до 5000 незархивированных экземпляров",
        )?;
        require(
            !request.name_pattern.trim().is_empty() && request.name_pattern.len() <= 200,
            "Шаблон имени обязателен (до 200 байт)",
        )?;
        let created: Vec<_> = (1..=request.count)
            .map(|n| {
                let id = id("agent");
                AgentInstance {
                    workspace_key: id.clone(),
                    id,
                    name: request.name_pattern.replace("{n}", &n.to_string()),
                    role_id: request.role_id.clone(),
                    role_version: request.role_version,
                    session_id: None,
                    owning_holon_id: None,
                    status: AgentStatus::Idle,
                    created_at: now(),
                    forked_from: None,
                }
            })
            .collect();
        let ids = created.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        self.agents.extend(created);
        self.batch_receipts
            .insert(key.into(), (request, ids.clone()));
        Ok(ids)
    }
    pub fn pin_role(&mut self, agent_ids: &[String], role_id: &str, version: u32) -> Result<()> {
        self.role(role_id, version)?;
        for id in agent_ids {
            self.agent(id)?;
        }
        for id in agent_ids {
            let a = self.agent_mut(id)?;
            a.role_id = role_id.into();
            a.role_version = version;
        }
        Ok(())
    }
    pub fn archive(&mut self, id: &str) -> Result<()> {
        let a = self.agent_mut(id)?;
        if matches!(
            a.status,
            AgentStatus::Running | AgentStatus::Stopping | AgentStatus::Queued
        ) {
            return Err(Error::Conflict("Сначала остановите задачи агента".into()));
        }
        a.status = AgentStatus::Archived;
        Ok(())
    }
}
