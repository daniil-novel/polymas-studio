use super::*;
use crate::storage::WriteTransaction;
use std::collections::BTreeSet;

pub(super) fn link(
    tx: &WriteTransaction<'_>,
    scope: &ExchangeScope,
    entry: &mut Publication,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut contributors = BTreeSet::from([scope.author.clone()]);
    for link in &entry.links {
        bounded(&link.entry_id, 200, "ID связанного утверждения")?;
        require(ids.insert(&link.entry_id), "Повторная связь с утверждением")?;
        let parent = tx
            .get::<Publication>("publications", &link.entry_id)?
            .ok_or_else(|| Error::Denied("Связанное утверждение недоступно".into()))?
            .value;
        require(
            parent.run_id == scope.run_id
                && parent.mode == CommunicationMode::Graph
                && visible(&parent, &scope.readable_authors),
            "Связанное утверждение недоступно",
        )?;
        entry.depth = entry.depth.max(
            parent
                .depth
                .checked_add(1)
                .ok_or_else(|| Error::Capacity("Переполнение глубины графа".into()))?,
        );
        contributors.extend(parent.contributors);
    }
    require(
        entry.depth <= scope.hop_limit && contributors.len() <= 32,
        "Достигнут предел глубины или 32 участников; нужна эскалация",
    )?;
    entry.contributors = contributors.into_iter().collect();
    Ok(())
}
