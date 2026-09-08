use super::*;

impl ApprovalLedger {
    /// Records the owner's observed outcome. It never sends another external call.
    pub fn reconcile_unknown(
        db: &mut crate::storage::Store,
        id: &str,
        expected_revision: i64,
        outcome: ApprovalState,
        note: &str,
    ) -> Result<Approval> {
        require(
            matches!(outcome, ApprovalState::Succeeded | ApprovalState::Failed)
                && note.trim().len() >= 10
                && note.len() <= 2000,
            "Укажите проверенный исход и описание сверки (10–2000 байт)",
        )?;
        db.transaction(|tx| {
            let mut stored = tx.get::<Approval>("approvals", id)?
                .ok_or_else(|| Error::NotFound("Разрешение не найдено".into()))?;
            if stored.value.state == outcome && Some(stored.revision) == expected_revision.checked_add(1)
                && tx.get::<Value>("approval_reconciliations", id)?.is_some_and(|s| s.value["note"] == note) {
                return Ok(stored.value);
            }
            require(stored.revision == expected_revision && stored.value.state == ApprovalState::Unknown,
                "Исход уже изменён или не требует ручной сверки")?;
            stored.value.state = outcome;
            tx.put("approvals", id, stored.revision, &stored.value)?;
            tx.put("approval_reconciliations", id, 0,
                &json!({"outcome":outcome,"note":note,"author":"local:owner","observedAt":crate::registry::now()}))?;
            tx.emit(EventInput::new("approval.reconciled", json!({"approvalId":id,"taskId":stored.value.operation.task_id,"state":outcome,"author":"local:owner"}))
                .agent(&stored.value.operation.agent_id).run(&stored.value.operation.run_id))?;
            Ok(stored.value)
        })
    }
}
