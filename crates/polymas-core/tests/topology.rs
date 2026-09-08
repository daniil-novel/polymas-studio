use polymas_core::registry::{BatchRequest, Registry, RoleDraft};
use polymas_core::topology::{Architecture, Holon, Preset, Target};

fn registry(n: usize) -> Registry {
    let mut r = Registry::default();
    let role = r
        .create_role(RoleDraft {
            name: "Analyst".into(),
            instruction: "Analyze supplied evidence".into(),
            ..Default::default()
        })
        .unwrap();
    r.create_batch(
        "create",
        BatchRequest {
            role_id: role.id,
            role_version: 1,
            count: n,
            name_pattern: "Agent {n}".into(),
        },
    )
    .unwrap();
    r
}

#[test]
fn coordinator_and_memory_are_independent_without_hidden_agents() {
    let r = registry(10);
    for coordinator in [false, true] {
        for memory in [false, true] {
            let a = Architecture {
                coordinator_agent_id: coordinator.then(|| r.agents[0].id.clone()),
                memory_enabled: memory,
                ..Default::default()
            };
            a.validate(&r).unwrap();
            let recipients = a.resolve(&r, &Target::System { broadcast: false }).unwrap();
            assert_eq!(recipients.len(), 1);
            assert_eq!(r.agents.len(), 10);
        }
    }
}

#[test]
fn single_mode_is_exactly_one_instance_without_a_coordinator() {
    let r = registry(1);
    let mut a = Architecture {
        preset: Preset::Single,
        ..Default::default()
    };
    a.validate(&r).unwrap();
    a.coordinator_agent_id = Some(r.agents[0].id.clone());
    assert!(a.validate(&r).is_err());
}

#[test]
fn holon_ownership_cycles_and_duplicate_members_are_rejected() {
    let r = registry(2);
    let h1 = Holon {
        id: "h1".into(),
        name: "Root".into(),
        parent_id: Some("h2".into()),
        members: vec![r.agents[0].id.clone()],
        ..Default::default()
    };
    let h2 = Holon {
        id: "h2".into(),
        name: "Child".into(),
        parent_id: Some("h1".into()),
        ..Default::default()
    };
    let mut a = Architecture {
        holons: vec![h1, h2],
        ..Default::default()
    };
    assert!(a.validate(&r).is_err());
    a.holons[0].parent_id = None;
    a.validate(&r).unwrap();
    a.holons[1].members.push(r.agents[0].id.clone());
    assert!(a.validate(&r).is_err());
}

#[test]
fn recursion_and_broadcast_require_explicit_dispatch_choice() {
    let r = registry(2);
    let a = Architecture {
        holons: vec![
            Holon {
                id: "root".into(),
                name: "Root".into(),
                members: vec![r.agents[0].id.clone()],
                ..Default::default()
            },
            Holon {
                id: "child".into(),
                name: "Child".into(),
                parent_id: Some("root".into()),
                members: vec![r.agents[1].id.clone()],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_eq!(
        a.resolve(
            &r,
            &Target::Holon {
                id: "root".into(),
                recursive: false
            }
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        a.resolve(
            &r,
            &Target::Holon {
                id: "root".into(),
                recursive: true
            }
        )
        .unwrap()
        .len(),
        2
    );
}
