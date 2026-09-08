use polymas_core::registry::{BatchRequest, Registry, RoleDraft};
use std::collections::HashSet;

fn role(name: &str) -> RoleDraft {
    RoleDraft {
        name: name.into(),
        instruction: "Use only allowed tools. Cite evidence.".into(),
        ..Default::default()
    }
}

#[test]
fn numbered_names_support_zero_padding_and_reject_unknown_template_syntax_atomically() {
    let mut registry = Registry::default();
    let role = registry.create_role(role("Analyst")).unwrap();
    registry
        .create_batch(
            "padded",
            BatchRequest {
                role_id: role.id.clone(),
                role_version: 1,
                count: 2,
                name_pattern: "Аналитик · {n:02}".into(),
            },
        )
        .unwrap();
    assert_eq!(registry.agents[0].name, "Аналитик · 01");
    assert_eq!(registry.agents[1].name, "Аналитик · 02");
    assert!(
        registry
            .create_batch(
                "bad",
                BatchRequest {
                    role_id: role.id,
                    role_version: 1,
                    count: 2,
                    name_pattern: "Аналитик · {n:99}".into()
                }
            )
            .is_err()
    );
    assert_eq!(registry.agents.len(), 2);
}

#[test]
fn thousand_instances_have_distinct_identity_and_private_workspace() {
    let mut registry = Registry::default();
    let role = registry.create_role(role("Researcher")).unwrap();
    let request = BatchRequest {
        role_id: role.id,
        role_version: 1,
        count: 1000,
        name_pattern: "Researcher {n}".into(),
    };
    let ids = registry.create_batch("batch-1", request.clone()).unwrap();
    assert_eq!(ids.len(), 1000);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 1000);
    assert_eq!(
        registry
            .agents
            .iter()
            .map(|a| &a.workspace_key)
            .collect::<HashSet<_>>()
            .len(),
        1000
    );
    assert!(registry.agents.iter().all(|a| a.session_id.is_none()));
    assert_eq!(registry.create_batch("batch-1", request).unwrap(), ids);
    assert_eq!(registry.agents.len(), 1000);
}

#[test]
fn same_idempotency_key_cannot_mean_a_different_operation() {
    let mut r = Registry::default();
    let role = r.create_role(role("Analyst")).unwrap();
    let mut b = BatchRequest {
        role_id: role.id,
        role_version: 1,
        count: 2,
        name_pattern: "Agent {n}".into(),
    };
    r.create_batch("request", b.clone()).unwrap();
    b.count = 3;
    assert!(r.create_batch("request", b).is_err());
    assert_eq!(r.agents.len(), 2);
}

#[test]
fn editing_a_role_never_rewrites_pinned_agents_or_earlier_versions() {
    let mut r = Registry::default();
    let first = r.create_role(role("Writer")).unwrap();
    r.create_batch(
        "first",
        BatchRequest {
            role_id: first.id.clone(),
            role_version: 1,
            count: 1,
            name_pattern: "Writer {n}".into(),
        },
    )
    .unwrap();
    let v2 = r.revise_role(&first.id, 1, role("Editor")).unwrap();
    assert_eq!(v2.version, 2);
    assert_eq!(r.role(&first.id, 1).unwrap().draft.name, "Writer");
    assert_eq!(r.agents[0].role_version, 1);
    assert!(r.revise_role(&first.id, 1, role("Stale editor")).is_err());
}

#[test]
fn invalid_batch_is_atomic_and_role_requires_meaningful_instructions() {
    let mut r = Registry::default();
    assert!(r.create_role(RoleDraft::default()).is_err());
    let role = r.create_role(role("A")).unwrap();
    for count in [0, 5001] {
        assert!(
            r.create_batch(
                "invalid",
                BatchRequest {
                    role_id: role.id.clone(),
                    role_version: 1,
                    count,
                    name_pattern: "A {n}".into()
                }
            )
            .is_err()
        );
    }
    assert!(r.agents.is_empty());
}
