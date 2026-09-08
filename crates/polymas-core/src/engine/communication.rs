use super::*;
use crate::{communication::ExchangeScope, registry::AgentStatus};

impl Engine {
    pub fn communication_scope(&self, run_id: &str, author: &str) -> Result<ExchangeScope> {
        let snapshot = self.run_configuration(run_id)?;
        let live = &self.workspace.config().value;
        let active = |config: &crate::workspace::Configuration, id: &str| {
            config
                .registry
                .agent(id)
                .is_ok_and(|a| a.status != AgentStatus::Archived)
        };
        require(
            active(&snapshot, author) && active(live, author),
            "Участник обмена недоступен",
        )?;
        require(
            snapshot.architecture.communication_mode == live.architecture.communication_mode,
            "Способ общения изменён; для нового режима нужен новый запуск",
        )?;
        let readable_authors = snapshot
            .registry
            .agents
            .iter()
            .filter(|candidate| {
                active(&snapshot, &candidate.id)
                    && active(live, &candidate.id)
                    && (candidate.id == author
                        || (snapshot.architecture.permits_message(&candidate.id, author)
                            && live.architecture.permits_message(&candidate.id, author)))
            })
            .map(|a| a.id.clone())
            .collect();
        Ok(ExchangeScope {
            run_id: run_id.into(),
            author: author.into(),
            mode: snapshot.architecture.communication_mode,
            readable_authors,
            max_publications: snapshot
                .architecture
                .max_messages_per_run
                .min(live.architecture.max_messages_per_run),
            hop_limit: snapshot
                .architecture
                .message_hop_limit
                .min(live.architecture.message_hop_limit),
        })
    }
}
