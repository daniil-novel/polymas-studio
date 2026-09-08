use crate::error::{Error, Result, require};
use crate::registry::{AgentStatus, Registry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Single,
    Independent,
    #[default]
    Network,
    Star,
    Holonic,
    Custom,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    #[default]
    QueueAllowed,
    StartAll,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommunicationMode {
    #[default]
    Direct,
    Blackboard,
    Wiki,
    Graph,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Architecture {
    pub communication_mode: CommunicationMode,
    pub preset: Preset,
    pub max_concurrency: usize,
    pub admission: Admission,
    pub coordinator_agent_id: Option<String>,
    pub memory_enabled: bool,
    pub memory_connection_id: Option<String>,
    pub holons: Vec<Holon>,
    pub edges: Vec<Edge>,
    pub positions: BTreeMap<String, [f32; 2]>,
    pub message_hop_limit: u32,
    pub max_messages_per_run: usize,
    pub paused: bool,
}
impl Default for Architecture {
    fn default() -> Self {
        Self {
            communication_mode: CommunicationMode::Direct,
            preset: Preset::Network,
            max_concurrency: 1,
            admission: Admission::QueueAllowed,
            coordinator_agent_id: None,
            memory_enabled: false,
            memory_connection_id: None,
            holons: vec![],
            edges: vec![],
            positions: BTreeMap::new(),
            message_hop_limit: 8,
            max_messages_per_run: 1000,
            paused: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Holon {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub parent_id: Option<String>,
    pub members: Vec<String>,
    pub coordinator_agent_id: Option<String>,
    pub input_contract: String,
    pub output_contract: String,
    pub max_tasks: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Agent { id: String },
    Agents { ids: Vec<String> },
    Holon { id: String, recursive: bool },
    System { broadcast: bool },
}

impl Architecture {
    pub fn validate(&self, registry: &Registry) -> Result<()> {
        let active: HashSet<_> = registry
            .agents
            .iter()
            .filter(|a| a.status != AgentStatus::Archived)
            .map(|a| a.id.as_str())
            .collect();
        let n = active.len();
        require(n <= 5000, "Максимум 5000 активных определений")?;
        require(
            self.max_concurrency >= 1 && self.max_concurrency <= n.max(1),
            "Параллелизм C должен быть от 1 до N",
        )?;
        require(
            (1..=32).contains(&self.message_hop_limit)
                && (1..=100_000).contains(&self.max_messages_per_run),
            "Некорректный бюджет сообщений",
        )?;
        if let Some(id) = &self.coordinator_agent_id {
            require(
                active.contains(id.as_str()) && n > 1,
                "Координатор должен быть существующим агентом системы из двух и более экземпляров",
            )?;
        }
        if self.preset == Preset::Single {
            require(
                n == 1 && self.coordinator_agent_id.is_none(),
                "Один агент: ровно один экземпляр без координатора",
            )?;
        }
        if self.preset == Preset::Star {
            require(
                self.coordinator_agent_id.is_some(),
                "Звезде необходим координатор",
            )?;
        }
        require(
            self.holons.len() <= 5000 && self.edges.len() <= 25_000,
            "Превышен лимит групп или связей",
        )?;
        let holons: BTreeMap<_, _> = self.holons.iter().map(|h| (h.id.as_str(), h)).collect();
        require(holons.len() == self.holons.len(), "Повторный ID холона")?;
        let mut owned = HashSet::new();
        for holon in &self.holons {
            require(
                !holon.id.is_empty() && !holon.name.trim().is_empty() && holon.name.len() <= 200,
                "ID и название холона обязательны",
            )?;
            let mut seen = HashSet::from([holon.id.as_str()]);
            let mut parent = holon.parent_id.as_deref();
            while let Some(p) = parent {
                require(seen.insert(p), "Цикл владения холонов")?;
                require(seen.len() <= 16, "Глубина холонов не больше 16")?;
                parent = holons
                    .get(p)
                    .ok_or_else(|| Error::Validation("Родительский холон не найден".into()))?
                    .parent_id
                    .as_deref();
            }
            for agent in &holon.members {
                require(
                    active.contains(agent.as_str()) && owned.insert(agent),
                    "Неизвестный агент или несколько владельцев",
                )?;
            }
            if let Some(coordinator) = &holon.coordinator_agent_id {
                require(
                    holon.members.contains(coordinator),
                    "Координатор холона должен быть его непосредственным участником",
                )?;
            }
        }
        let mut edges = HashSet::new();
        for edge in &self.edges {
            require(
                active.contains(edge.from.as_str())
                    && active.contains(edge.to.as_str())
                    && edge.from != edge.to,
                "Связь должна соединять двух существующих агентов",
            )?;
            require(edges.insert((&edge.from, &edge.to)), "Повторная связь")?;
        }
        require(
            self.positions.len() <= 10_000
                && self
                    .positions
                    .values()
                    .all(|xy| xy.iter().all(|x| x.is_finite() && x.abs() < 1_000_000.0)),
            "Некорректные координаты",
        )?;
        Ok(())
    }
    pub fn resolve(&self, registry: &Registry, target: &Target) -> Result<Vec<String>> {
        self.validate(registry)?;
        let ids = match target {
            Target::Agent { id } => vec![id.clone()],
            Target::Agents { ids } => ids.clone(),
            Target::System { broadcast } => {
                if let Some(coordinator) = &self.coordinator_agent_id {
                    vec![coordinator.clone()]
                } else {
                    registry
                        .agents
                        .iter()
                        .filter(|a| a.status != AgentStatus::Archived)
                        .take(if *broadcast { 5000 } else { 1 })
                        .map(|a| a.id.clone())
                        .collect()
                }
            }
            Target::Holon { id, recursive } => {
                let holon = self
                    .holons
                    .iter()
                    .find(|h| &h.id == id)
                    .ok_or_else(|| Error::NotFound("Холон не найден".into()))?;
                if !recursive && holon.coordinator_agent_id.is_some() {
                    vec![holon.coordinator_agent_id.clone().unwrap_or_default()]
                } else {
                    let mut included = HashSet::from([id.clone()]);
                    if *recursive {
                        loop {
                            let old = included.len();
                            for h in &self.holons {
                                if h.parent_id.as_ref().is_some_and(|p| included.contains(p)) {
                                    included.insert(h.id.clone());
                                }
                            }
                            if included.len() == old {
                                break;
                            }
                        }
                    }
                    self.holons
                        .iter()
                        .filter(|h| included.contains(&h.id))
                        .flat_map(|h| h.members.clone())
                        .collect()
                }
            }
        };
        require(
            !ids.is_empty() && ids.len() <= 5000,
            "Выберите хотя бы одного получателя",
        )?;
        let mut unique = HashSet::new();
        for id in &ids {
            require(
                unique.insert(id) && registry.agent(id)?.status != AgentStatus::Archived,
                "Повторный или архивный получатель",
            )?;
        }
        Ok(ids)
    }
    pub fn permits_message(&self, from: &str, to: &str) -> bool {
        if from == to || matches!(self.preset, Preset::Single | Preset::Independent) {
            return false;
        }
        if self
            .coordinator_agent_id
            .as_deref()
            .is_some_and(|c| c == from || c == to)
        {
            return true;
        }
        self.edges.iter().any(|e| e.from == from && e.to == to)
            || self.holons.iter().any(|h| {
                h.coordinator_agent_id
                    .as_deref()
                    .is_some_and(|c| c == from || c == to)
                    && h.members.iter().any(|m| m == from)
                    && h.members.iter().any(|m| m == to)
            })
    }
}
