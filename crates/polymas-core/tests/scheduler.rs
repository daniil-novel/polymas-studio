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
fn a_reserved_approval_turn_keeps_agent_order_without_reserving_a_process_slot() {
    let (r, a) = setup(2, 1);
    let mut s = Scheduler::default();
    s.submit("approval", &r, &a, 1, input(), 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let origin = first.lease.unwrap().epoch;
    let reserved = s.reserve_approval_continuation(&first.id, origin).unwrap();
    assert!(reserved.epoch > origin);
    assert_eq!(
        s.reserve_approval_continuation(&first.id, origin).unwrap(),
        reserved
    );
    let mut overtaking = input();
    overtaking.target = Target::Agent {
        id: first.assignee.clone(),
    };
    overtaking.priority = 50;
    let other_run = s.submit("same-agent", &r, &a, 1, overtaking, 1).unwrap();
    s.park_approval(&first.id, origin, "approval-1", "Wait".into())
        .unwrap();
    let mut db = polymas_core::storage::Store::memory().unwrap();
    db.transaction(|tx| s.persist_delta(&Scheduler::default(), tx))
        .unwrap();
    s = Scheduler::load(&db).unwrap();
    s.recover_after_crash();
    assert_eq!(
        s.task(&first.id).unwrap().continuation,
        Some(reserved.clone())
    );
    let second = s.claim_ready(1, "local", 101).unwrap().remove(0);
    assert_ne!(second.assignee, first.assignee);
    assert!(second.lease.as_ref().unwrap().epoch > reserved.epoch);
    s.wake_approval(&first.id, "approval-1", "Decision received".into())
        .unwrap();
    s.finish(&second.id, second.lease.unwrap().epoch, Ok("done".into()))
        .unwrap();
    let resumed = s.claim_ready(1, "local", 102).unwrap().remove(0);
    assert_eq!(resumed.id, first.id);
    assert_eq!(resumed.lease.as_ref().unwrap().epoch, reserved.epoch);
    assert!(resumed.continuation.is_none());
    s.finish(&resumed.id, reserved.epoch, Ok("done".into()))
        .unwrap();
    let after = s.claim_ready(1, "local", 103).unwrap().remove(0);
    assert_eq!(after.run_id, other_run.id);
    assert!(after.lease.as_ref().unwrap().epoch > reserved.epoch);
    let obsolete = s
        .reserve_approval_continuation(&after.id, after.lease.unwrap().epoch)
        .unwrap();
    s.recover_after_crash();
    assert!(s.task(&after.id).unwrap().continuation.is_none());
    assert_eq!(s.task(&after.id).unwrap().state, TaskState::Unknown);
    assert!(obsolete.epoch > reserved.epoch);
}
#[test]
fn cancelling_a_reserved_turn_never_reuses_its_epoch() {
    let (r, a) = setup(1, 1);
    let mut s = Scheduler::default();
    s.submit("approval", &r, &a, 1, input(), 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let origin = first.lease.unwrap().epoch;
    let reserved = s.reserve_approval_continuation(&first.id, origin).unwrap();
    s.park_approval(&first.id, origin, "approval-1", "Wait".into())
        .unwrap();
    s.cancel_task(&first.id).unwrap();
    assert!(s.task(&first.id).unwrap().continuation.is_none());
    assert!(
        s.wake_approval(&first.id, "approval-1", "Late".into())
            .is_err()
    );
    s.submit("next", &r, &a, 1, input(), 1).unwrap();
    let next = s.claim_ready(1, "local", 101).unwrap().remove(0);
    assert!(next.lease.unwrap().epoch > reserved.epoch);
}
#[test]
fn waiting_for_owner_frees_capacity_and_only_the_matching_operation_can_resume() {
    let (r, a) = setup(2, 1);
    let mut s = Scheduler::default();
    s.submit("approval", &r, &a, 1, input(), 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let epoch = first.lease.unwrap().epoch;
    s.park_approval(&first.id, epoch, "approval-1", "Waiting for owner".into())
        .unwrap();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::WaitingUser);
    assert!(s.task(&first.id).unwrap().lease.is_none());
    assert_eq!(
        s.task(&first.id)
            .unwrap()
            .approval_wait
            .as_ref()
            .unwrap()
            .origin_epoch,
        epoch
    );
    assert!(s.verify_lease(&first.id, epoch).is_err());
    assert!(s.accept(&first.id, true).is_err());
    let mut db = polymas_core::storage::Store::memory().unwrap();
    db.transaction(|tx| s.persist_delta(&Scheduler::default(), tx))
        .unwrap();
    let reloaded = Scheduler::load(&db).unwrap();
    assert_eq!(
        reloaded.task(&first.id).unwrap().approval_wait,
        s.task(&first.id).unwrap().approval_wait
    );
    s.recover_after_crash();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::WaitingUser);
    assert!(
        s.wake_approval(&first.id, "other-approval", "Forged decision".into())
            .is_err()
    );
    assert!(
        s.wake(&first.id, "A message cannot wake owner approval".into())
            .is_err()
    );
    let second = s.claim_ready(1, "local", 101).unwrap().remove(0);
    assert_ne!(second.assignee, first.assignee);
    s.wake_approval(&first.id, "approval-1", "Operation completed".into())
        .unwrap();
    assert!(s.task(&first.id).unwrap().approval_wait.is_none());
    s.finish(&second.id, second.lease.unwrap().epoch, Ok("done".into()))
        .unwrap();
    let resumed = s.claim_ready(1, "local", 102).unwrap().remove(0);
    assert_eq!(resumed.id, first.id);
    assert_eq!(resumed.attempt, 2);
    assert!(resumed.lease.as_ref().unwrap().epoch > epoch);
    assert!(s.finish(&first.id, epoch, Ok("stale".into())).is_err());
    s.park_approval(
        &first.id,
        resumed.lease.unwrap().epoch,
        "approval-2",
        "Next approval".into(),
    )
    .unwrap();
    s.cancel_task(&first.id).unwrap();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::Cancelled);
    assert!(s.task(&first.id).unwrap().approval_wait.is_none());
    assert!(
        s.wake_approval(&first.id, "approval-2", "Late owner response".into())
            .is_err()
    );
}
#[test]
fn stopped_exhausted_and_first_accepted_runs_cannot_resurrect_an_owner_wait() {
    let (r, a) = setup(2, 1);
    let mut s = Scheduler::default();
    let mut request = input();
    request.completion = CompletionRule::FirstAccepted;
    s.submit("approval", &r, &a, 1, request, 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let epoch = first.lease.unwrap().epoch;
    s.task_mut(&first.id).unwrap().role.draft.max_turns = 1;
    assert!(
        s.park_approval(&first.id, epoch, "approval-1", "No budget".into())
            .is_err()
    );
    assert!(s.task(&first.id).unwrap().approval_wait.is_none());
    assert!(s.verify_lease(&first.id, epoch).is_ok());
    s.task_mut(&first.id).unwrap().role.draft.max_turns = 4;
    s.park_approval(&first.id, epoch, "approval-1", "Waiting".into())
        .unwrap();
    let second = s.claim_ready(1, "local", 101).unwrap().remove(0);
    s.finish(
        &second.id,
        second.lease.unwrap().epoch,
        Ok("Accepted winner".into()),
    )
    .unwrap();
    s.accept(&second.id, true).unwrap();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::Cancelled);
    assert!(s.task(&first.id).unwrap().approval_wait.is_none());
    assert!(
        s.wake_approval(&first.id, "approval-1", "Late".into())
            .is_err()
    );
    assert!(
        s.park_approval(&first.id, epoch, "approval-1", "Late".into())
            .is_err()
    );
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
fn waiting_releases_capacity_preserves_identity_and_requires_a_new_fenced_attempt() {
    use polymas_core::scheduler::WaitCondition;
    let (r, a) = setup(2, 1);
    let mut s = Scheduler::default();
    s.submit("wait", &r, &a, 1, input(), 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let epoch = first.lease.as_ref().unwrap().epoch;
    s.park(
        &first.id,
        epoch,
        WaitCondition {
            sender: Some(r.agents[1].id.clone()),
            correlation_id: None,
            deadline_at: 200,
        },
        "Waiting for evidence".into(),
        100,
    )
    .unwrap();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::Waiting);
    assert!(s.verify_lease(&first.id, epoch).is_err());
    assert!(s.accept(&first.id, true).is_err());
    s.recover_after_crash();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::Waiting);
    let second = s.claim_ready(1, "local", 101).unwrap().remove(0);
    assert_ne!(second.assignee, first.assignee);
    s.wake(&first.id, "Message became available".into())
        .unwrap();
    assert!(s.claim_ready(1, "local", 102).unwrap().is_empty());
    s.finish(&second.id, second.lease.unwrap().epoch, Ok("done".into()))
        .unwrap();
    let resumed = s.claim_ready(1, "local", 103).unwrap().remove(0);
    assert_eq!(resumed.assignee, first.assignee);
    assert_eq!(resumed.attempt, 2);
    assert!(resumed.lease.unwrap().epoch > epoch);
    assert!(
        s.finish(&first.id, epoch, Ok("stale result".into()))
            .is_err()
    );
}

#[test]
fn stopped_or_turn_exhausted_waits_cannot_restart_the_agent() {
    use polymas_core::scheduler::WaitCondition;
    let (r, a) = setup(1, 1);
    let mut s = Scheduler::default();
    s.submit("wait", &r, &a, 1, input(), 1).unwrap();
    let first = s.claim_ready(1, "local", 100).unwrap().remove(0);
    let epoch = first.lease.unwrap().epoch;
    assert!(
        s.park(
            &first.id,
            epoch,
            WaitCondition {
                sender: None,
                correlation_id: None,
                deadline_at: 100
            },
            "Wait".into(),
            100
        )
        .is_err()
    );
    s.task_mut(&first.id).unwrap().role.draft.max_turns = 1;
    assert!(
        s.park(
            &first.id,
            epoch,
            WaitCondition {
                sender: None,
                correlation_id: None,
                deadline_at: 200
            },
            "Wait".into(),
            100
        )
        .is_err()
    );
    s.task_mut(&first.id).unwrap().role.draft.max_turns = 2;
    s.park(
        &first.id,
        epoch,
        WaitCondition {
            sender: None,
            correlation_id: None,
            deadline_at: 200,
        },
        "Wait".into(),
        100,
    )
    .unwrap();
    s.cancel_task(&first.id).unwrap();
    assert_eq!(s.task(&first.id).unwrap().state, TaskState::Cancelled);
    assert!(s.wake(&first.id, "message".into()).is_err());
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
