use polymas_core::{
    engine::Engine,
    registry::AgentStatus,
    scheduler::{Lease, Task, TaskState},
};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentActivity<'a> {
    id: &'a str,
    assignee: &'a str,
    state: TaskState,
    lease: Option<&'a Lease>,
}

fn priority(state: TaskState) -> u8 {
    match state {
        TaskState::Running | TaskState::Stopping | TaskState::Unknown => 4,
        TaskState::WaitingUser => 3,
        TaskState::Queued => 2,
        TaskState::Waiting => 1,
        _ => 0,
    }
}

// Recent task pages are history, not a complete source of live agent state.
// Keep one observation per active registry identity, independent of history size.
pub(crate) fn project(engine: &Engine) -> Vec<AgentActivity<'_>> {
    let active: HashSet<_> = engine
        .workspace
        .config()
        .value
        .registry
        .agents
        .iter()
        .filter(|agent| agent.status != AgentStatus::Archived)
        .map(|agent| agent.id.as_str())
        .collect();
    let mut selected: BTreeMap<&str, &Task> = BTreeMap::new();
    for task in engine.scheduler.tasks.iter().rev() {
        if priority(task.state) == 0 || !active.contains(task.assignee.as_str()) {
            continue;
        }
        if selected
            .get(task.assignee.as_str())
            .is_none_or(|previous| priority(task.state) > priority(previous.state))
        {
            selected.insert(&task.assignee, task);
        }
    }
    selected
        .values()
        .map(|task| AgentActivity {
            id: &task.id,
            assignee: &task.assignee,
            state: task.state,
            lease: task.lease.as_ref(),
        })
        .collect()
}
