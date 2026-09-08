use polymas_core::registry::{BatchRequest, Registry, RoleDraft};
use polymas_core::scheduler::{CompletionRule, Scheduler, TaskInput, TaskState};
use polymas_core::topology::{Admission, Architecture, Target};
use std::collections::HashSet;

fn setup(n: usize, c: usize) -> (Registry, Architecture) {
    let mut r = Registry::default();
    let v = r
        .create_role(RoleDraft {
            name: "Worker".into(),
            instruction: "Complete the assigned task".into(),
            ..Default::default()
        })
        .unwrap();
    r.create_batch(
        "batch",
        BatchRequest {
            role_id: v.id,
            role_version: 1,
            count: n,
            name_pattern: "Worker {n}".into(),
        },
    )
    .unwrap();
    (
        r,
        Architecture {
            max_concurrency: c,
            ..Default::default()
        },
    )
}
fn input() -> TaskInput {
    TaskInput {
        prompt: "Compute an evidence-backed result".into(),
        target: Target::System { broadcast: true },
        ..Default::default()
    }
}

#[test]
fn one_thousand_ids_are_queued_but_only_ten_distinct_leases_are_claimed() {
    let (r, a) = setup(1000, 10);
    let mut s = Scheduler::default();
    s.submit("request", &r, &a, 1, input(), 10).unwrap();
    let claims = s.claim_ready(10, "worker-local", 100).unwrap();
    assert_eq!(claims.len(), 10);
    assert_eq!(
        claims
            .iter()
            .map(|t| &t.assignee)
            .collect::<HashSet<_>>()
            .len(),
        10
    );
    assert!(s.claim_ready(10, "worker-local", 101).unwrap().is_empty());
    assert_eq!(
        s.tasks
            .iter()
            .filter(|t| t.state == TaskState::Queued)
            .count(),
        990
    );
}
#[test]
fn start_all_never_silently_falls_back_to_ten_slots() {
    let (r, mut a) = setup(1000, 1000);
    a.admission = Admission::StartAll;
    let mut s = Scheduler::default();
    assert!(s.submit("request", &r, &a, 1, input(), 10).is_err());
    assert!(s.tasks.is_empty() && s.runs.is_empty());
}
#[test]
fn completion_is_not_acceptance_and_old_fencing_token_cannot_complete() {
    let (r, a) = setup(1, 1);
    let mut s = Scheduler::default();
    s.submit("request", &r, &a, 1, input(), 1).unwrap();
    let t = s.claim_ready(1, "local", 1).unwrap().remove(0);
    let epoch = t.lease.as_ref().unwrap().epoch;
    assert!(s.finish(&t.id, epoch + 1, Ok("forged".into())).is_err());
    s.finish(&t.id, epoch, Ok("real output".into())).unwrap();
    assert_eq!(s.task(&t.id).unwrap().state, TaskState::AwaitingAcceptance);
    assert!(!s.runs[0].accepted);
    s.accept(&t.id, true).unwrap();
    assert!(s.runs[0].accepted);
}
#[test]
fn a_running_agent_never_gets_a_second_turn_and_crash_outcome_is_unknown() {
    let (r, a) = setup(1, 1);
    let mut s = Scheduler::default();
    s.submit("one", &r, &a, 1, input(), 1).unwrap();
    s.submit("two", &r, &a, 1, input(), 1).unwrap();
    assert_eq!(s.claim_ready(10, "local", 1).unwrap().len(), 1);
    assert!(s.claim_ready(10, "local", 2).unwrap().is_empty());
    s.recover_after_crash();
    assert_eq!(
        s.tasks
            .iter()
            .filter(|t| t.state == TaskState::Unknown)
            .count(),
        1
    );
    assert!(s.claim_ready(10, "local", 3).unwrap().is_empty());
}
#[test]
fn quorum_counts_only_accepted_results_and_role_snapshot_is_pinned() {
    let (mut r, a) = setup(3, 3);
    let mut s = Scheduler::default();
    let mut i = input();
    i.completion = CompletionRule::Quorum { required: 2 };
    s.submit("one", &r, &a, 1, i, 3).unwrap();
    r.revise_role(
        &r.roles[0].id.clone(),
        1,
        RoleDraft {
            name: "Changed".into(),
            instruction: "Different".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let claims = s.claim_ready(3, "local", 1).unwrap();
    for t in &claims {
        assert_eq!(t.role.version, 1);
        s.finish(&t.id, t.lease.as_ref().unwrap().epoch, Ok("output".into()))
            .unwrap();
    }
    s.accept(&claims[0].id, true).unwrap();
    assert!(!s.runs[0].accepted);
    s.accept(&claims[1].id, true).unwrap();
    assert!(s.runs[0].accepted);
}
