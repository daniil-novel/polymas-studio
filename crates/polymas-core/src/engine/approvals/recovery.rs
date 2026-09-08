use super::*;
use crate::policy::{ApprovalQuery, ApprovalState};

impl Engine {
    /// Keeps completed/cancelled attempts out of the owner's actionable inbox.
    /// The state index and metadata projection avoid loading active arguments.
    pub fn revoke_unused_approvals(&mut self) -> Result<Vec<String>> {
        let mut revoked = vec![];
        for state in [ApprovalState::Pending, ApprovalState::Approved] {
            let mut after = String::new();
            loop {
                let page = self.workspace.db.approval_page(&ApprovalQuery {
                    after,
                    state: Some(state),
                    limit: Some(100),
                    ..Default::default()
                })?;
                for item in page.items {
                    let reachable = self
                        .scheduler
                        .task(item["taskId"].as_str().unwrap_or_default())
                        .is_ok_and(|task| {
                            let reserved = task.continuation.as_ref().is_some_and(|ticket| {
                                Some(ticket.origin_epoch) == item["originEpoch"].as_u64()
                                    && Some(ticket.epoch) == item["epoch"].as_u64()
                            });
                            let origin_running = task.state == TaskState::Running
                                && task.lease.as_ref().is_some_and(|lease| {
                                    Some(lease.epoch) == item["originEpoch"].as_u64()
                                });
                            let reserved_wait = task.state == TaskState::Queued
                                || (task.state == TaskState::WaitingUser
                                    && task.approval_wait.as_ref().is_some_and(|wait| {
                                        Some(wait.approval_id.as_str()) == item["id"].as_str()
                                    }));
                            let executing_turn = task.state == TaskState::Running
                                && task.lease.as_ref().is_some_and(|lease| {
                                    Some(lease.epoch) == item["epoch"].as_u64()
                                });
                            Some(task.assignee.as_str()) == item["agentId"].as_str()
                                && Some(task.run_id.as_str()) == item["runId"].as_str()
                                && ((reserved && (origin_running || reserved_wait))
                                    || executing_turn)
                        });
                    if !reachable {
                        let id = item["id"]
                            .as_str()
                            .ok_or_else(|| Error::Storage("У операции отсутствует ID".into()))?;
                        ApprovalLedger::revoke_unused(&mut self.workspace.db, id)?;
                        revoked.push(id.to_owned());
                    }
                }
                match page.next_cursor {
                    Some(cursor) => after = cursor,
                    None => break,
                }
            }
        }
        Ok(revoked)
    }
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
