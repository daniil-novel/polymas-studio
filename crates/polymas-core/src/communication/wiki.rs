use super::*;
use crate::storage::WriteTransaction;
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Head {
    run_id: String,
    entry_id: String,
}

pub(super) fn validate(input: &PublicationInput) -> Result<()> {
    let slug = input.slug.as_deref().unwrap_or_default();
    require(
        !slug.is_empty()
            && slug.len() <= 80
            && slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
            && slug.as_bytes()[0].is_ascii_alphanumeric(),
        "Slug страницы: 1–80 символов a-z, 0-9, _ или -, начинается с буквы/цифры",
    )
}
pub(super) fn revise(
    tx: &WriteTransaction<'_>,
    scope: &ExchangeScope,
    input: &PublicationInput,
    entry: &mut Publication,
) -> Result<()> {
    let key = hash(&serde_json::to_vec(&(&scope.run_id, "wiki", &input.slug))?);
    let head = tx.get::<Head>("publication_heads", &key)?;
    let revision = head.as_ref().map_or(0, |h| h.revision);
    if let Some(head) = head {
        let previous = tx
            .get::<Publication>("publications", &head.value.entry_id)?
            .ok_or_else(|| Error::Storage("Версия страницы не найдена".into()))?
            .value;
        require(
            visible(&previous, &scope.readable_authors),
            "Страница недоступна этой области",
        )?;
        entry.previous_id = Some(previous.id);
        entry.contributors = previous
            .contributors
            .into_iter()
            .chain([scope.author.clone()])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    if revision != i64::from(input.expected_revision) {
        return Err(Error::Conflict(
            "Версия страницы изменилась; прочитайте текущую и повторите с новым ключом".into(),
        ));
    }
    require(
        entry.contributors.len() <= 32,
        "Не более 32 участников одной страницы",
    )?;
    entry.revision = input
        .expected_revision
        .checked_add(1)
        .ok_or_else(|| Error::Capacity("Исчерпаны версии страницы".into()))?;
    tx.put(
        "publication_heads",
        &key,
        revision,
        &Head {
            run_id: scope.run_id.clone(),
            entry_id: entry.id.clone(),
        },
    )?;
    Ok(())
}
