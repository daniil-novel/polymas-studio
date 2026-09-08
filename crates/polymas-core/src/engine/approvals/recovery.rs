use super::*;
use crate::policy::{ApprovalQuery, ApprovalState};

impl Engine {
    /// The state index excludes historical operations. Active arguments are not
    /// deserialized during routine sweeps; only abandoned entries are changed.
    pub fn recover_inactive_operations(&mut self) -> Result<Vec<String>> {
        let mut after = String::new();
        let mut recovered = vec![];
        loop {
            let page = self.workspace.db.approval_page(&ApprovalQuery {
                after,
                limit: Some(100),
                state: Some(ApprovalState::Executing),
                ..Default::default()
            })?;
            for item in page.items {
                let task_id = item["taskId"].as_str().unwrap_or_default();
                let active = self.scheduler.task(task_id).is_ok_and(|task| {
                    task.state == TaskState::Running
                        && task
                            .lease
                            .as_ref()
                            .is_some_and(|l| Some(l.epoch) == item["epoch"].as_u64())
                        && Some(task.assignee.as_str()) == item["agentId"].as_str()
                        && Some(task.run_id.as_str()) == item["runId"].as_str()
                });
                if !active {
                    let id = item["id"]
                        .as_str()
                        .ok_or_else(|| Error::Storage("У операции отсутствует ID".into()))?;
                    ApprovalLedger::complete(&mut self.workspace.db, id, ApprovalState::Unknown)?;
                    recovered.push(id.to_owned());
                }
            }
            match page.next_cursor {
                Some(cursor) => after = cursor,
                None => break,
            }
        }
        Ok(recovered)
    }
    pub(crate) fn require_resolved_operations(&self, task_id: &str) -> Result<()> {
        for state in [ApprovalState::Executing, ApprovalState::Unknown] {
            let page = self.workspace.db.approval_page(&ApprovalQuery {
                limit: Some(1),
                task_id: Some(task_id.to_owned()),
                state: Some(state),
                ..Default::default()
            })?;
            require(
                page.items.is_empty(),
                "Сначала сверьте неизвестный исход внешней операции в разделе «Подтверждения»",
            )?;
        }
        Ok(())
    }
}
