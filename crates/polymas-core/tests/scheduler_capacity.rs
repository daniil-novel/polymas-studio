use polymas_core::{
    registry::{BatchRequest, Registry, RoleDraft},
    scheduler::{Scheduler, TaskInput, TaskState},
    topology::{Architecture, Target},
};

#[test]
fn a_batch_cannot_overfill_the_working_set_and_rejection_does_not_consume_its_key() {
    let mut registry = Registry::default();
    let role = registry
        .create_role(RoleDraft {
            name: "Capacity fixture".into(),
            instruction: "Work".into(),
            ..Default::default()
        })
        .unwrap();
    let ids = registry
        .create_batch(
            "agents",
            BatchRequest {
                role_id: role.id,
                role_version: 1,
                count: 2,
                name_pattern: "Agent {n}".into(),
            },
        )
        .unwrap();
    let architecture = Architecture::default();
    let mut scheduler = Scheduler::default();
    scheduler
        .submit(
            "seed",
            &registry,
            &architecture,
            1,
            TaskInput {
                prompt: "Fixture seed".into(),
                target: Target::Agent { id: ids[0].clone() },
                ..Default::default()
            },
            1,
        )
        .unwrap();
    // Authored working-set pressure only: these definitions do not represent provider executions.
    let mut historical = scheduler.tasks[0].clone();
    historical.state = TaskState::Accepted;
    historical.completed_at = Some(historical.created_at.clone());
    scheduler.tasks = (0..99_999)
        .map(|i| {
            let mut task = historical.clone();
            task.id = format!("historical-{i}");
            task
        })
        .collect();
    let input = TaskInput {
        prompt: "Two recipients".into(),
        target: Target::Agents { ids },
        ..Default::default()
    };
    let before_runs = scheduler.runs.clone();
    let error = scheduler
        .submit("boundary", &registry, &architecture, 1, input.clone(), 1)
        .expect_err("99999 + 2 must be rejected as one batch");
    assert!(error.to_string().contains("100 000"));
    assert_eq!(scheduler.tasks.len(), 99_999);
    assert_eq!(scheduler.runs, before_runs);
    assert_eq!(scheduler.tasks[0].id, "historical-0");
    assert_eq!(scheduler.tasks.last().unwrap().id, "historical-99998");
    scheduler.tasks.pop();
    let mut corrected = input;
    corrected.title = "Admitted after space became available".into();
    let run = scheduler
        .submit(
            "boundary",
            &registry,
            &architecture,
            1,
            corrected.clone(),
            1,
        )
        .unwrap();
    assert_eq!(run.task_ids.len(), 2);
    assert_eq!(scheduler.tasks.len(), 100_000);
    assert_eq!(scheduler.runs.len(), before_runs.len() + 1);
    // A receipt read still works at the hard limit without allocating another batch.
    assert_eq!(
        scheduler
            .submit(
                "boundary",
                &registry,
                &architecture,
                1,
                corrected.clone(),
                1
            )
            .unwrap(),
        run
    );
    assert_eq!(scheduler.tasks.len(), 100_000);
    assert!(
        scheduler
            .submit("full", &registry, &architecture, 1, corrected, 1)
            .is_err()
    );
    assert_eq!(scheduler.runs.len(), before_runs.len() + 1);
    assert_eq!(scheduler.tasks.len(), 100_000);
}
