//! Run-local, mediated publications. Stored text is untrusted, never verified knowledge.
use crate::{
    error::{Error, Result, require},
    registry::{id, now},
    storage::{EventInput, Store},
    topology::CommunicationMode,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
mod graph;
mod wiki;

#[derive(Clone, Debug)]
pub struct ExchangeScope {
    pub run_id: String,
    pub author: String,
    pub mode: CommunicationMode,
    pub readable_authors: Vec<String>,
    pub max_publications: usize,
    pub hop_limit: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct PublicationInput {
    pub links: Vec<PublicationLink>,
    pub key: String,
    pub title: String,
    pub body: String,
    pub slug: Option<String>,
    pub expected_revision: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationLink {
    pub entry_id: String,
    pub relation: crate::knowledge::Relation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    #[serde(default)]
    pub links: Vec<PublicationLink>,
    #[serde(default)]
    pub depth: u32,
    pub id: String,
    pub run_id: String,
    pub mode: CommunicationMode,
    pub author: String,
    pub contributors: Vec<String>,
    pub title: String,
    pub body: String,
    pub slug: Option<String>,
    pub revision: u32,
    #[serde(default)]
    pub previous_id: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct PublicationQuery {
    pub latest_only: bool,
    pub after: i64,
    pub limit: usize,
    pub entry_id: Option<String>,
    pub slug: Option<String>,
}
impl Default for PublicationQuery {
    fn default() -> Self {
        Self {
            latest_only: false,
            after: 0,
            limit: 20,
            entry_id: None,
            slug: None,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationPage {
    pub items: Vec<Publication>,
    pub cursor: i64,
    pub has_more: bool,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    fingerprint: String,
    entry_id: String,
}
fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn pre_links_fingerprint(
    mode: CommunicationMode,
    input: &PublicationInput,
) -> Result<Option<String>> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Legacy<'a> {
        key: &'a str,
        title: &'a str,
        body: &'a str,
        slug: &'a Option<String>,
        expected_revision: u32,
    }
    if !input.links.is_empty() {
        return Ok(None);
    }
    Ok(Some(hash(&serde_json::to_vec(&(
        mode,
        Legacy {
            key: &input.key,
            title: &input.title,
            body: &input.body,
            slug: &input.slug,
            expected_revision: input.expected_revision,
        },
    ))?)))
}
fn bounded(value: &str, max: usize, label: &str) -> Result<()> {
    require(
        !value.trim().is_empty() && value.len() <= max,
        &format!("{label}: значение пустое или слишком длинное"),
    )
}
fn visible(entry: &Publication, authors: &[String]) -> bool {
    entry
        .contributors
        .iter()
        .all(|author| authors.contains(author))
}
pub struct Publications;
impl Publications {
    pub fn publish(
        db: &mut Store,
        scope: &ExchangeScope,
        input: PublicationInput,
    ) -> Result<Publication> {
        require(
            matches!(
                scope.mode,
                CommunicationMode::Blackboard | CommunicationMode::Wiki | CommunicationMode::Graph
            ),
            "Режим публикаций пока недоступен",
        )?;
        bounded(&scope.run_id, 200, "Запуск")?;
        bounded(&scope.author, 200, "Автор")?;
        require(
            scope.readable_authors.contains(&scope.author) && scope.readable_authors.len() <= 5000,
            "Некорректная область публикации",
        )?;
        require(
            (1..=100_000).contains(&scope.max_publications),
            "Некорректный бюджет публикаций",
        )?;
        bounded(&input.key, 128, "Ключ")?;
        bounded(&input.title, 200, "Заголовок")?;
        bounded(&input.body, 16384, "Markdown")?;
        require(
            (1..=32).contains(&scope.hop_limit),
            "Некорректная глубина графа",
        )?;
        require(
            input.links.len() <= 8
                && (scope.mode == CommunicationMode::Graph || input.links.is_empty()),
            "Связи доступны только графу, не более восьми",
        )?;
        if scope.mode == CommunicationMode::Wiki {
            wiki::validate(&input)?;
        } else {
            require(
                input.slug.is_none() && input.expected_revision == 0,
                "Запись доски неизменяема и не имеет slug",
            )?;
        }
        let receipt_key = hash(&serde_json::to_vec(&(
            &scope.run_id,
            &scope.author,
            &input.key,
        ))?);
        let fingerprint = hash(&serde_json::to_vec(&(scope.mode, &input))?);
        let legacy_fingerprint = pre_links_fingerprint(scope.mode, &input)?;
        db.transaction(|tx| {
            if let Some(old) = tx.get::<Receipt>("publication_receipts", &receipt_key)? {
                if old.value.fingerprint != fingerprint && Some(&old.value.fingerprint) != legacy_fingerprint.as_ref() { return Err(Error::Conflict("Ключ публикации уже использован иначе".into())); }
                let entry = tx.get::<Publication>("publications", &old.value.entry_id)?.ok_or_else(|| Error::Storage("Публикация receipt не найдена".into()))?.value;
                require(visible(&entry, &scope.readable_authors), "Доступ к публикации отозван")?;
                return Ok(entry);
            }
            let count: i64 = tx.connection().query_row("SELECT (SELECT COUNT(*) FROM entities WHERE collection='publications' AND json_extract(document,'$.runId')=?1) + (SELECT COUNT(*) FROM messages WHERE run_id=?1)", [&scope.run_id], |r| r.get(0))?;
            require(count < scope.max_publications as i64, "Исчерпан общий бюджет сообщений и публикаций")?;
            let mut entry = Publication { id: id("publication"), run_id: scope.run_id.clone(), mode: scope.mode, author: scope.author.clone(), contributors: vec![scope.author.clone()], title: input.title.clone(), body: input.body.clone(), slug: input.slug.clone(), revision: 1, previous_id: None, links: input.links.clone(), depth: 0, created_at: now() };
            if scope.mode == CommunicationMode::Wiki { wiki::revise(tx, scope, &input, &mut entry)?; }
            if scope.mode == CommunicationMode::Graph { graph::link(tx, scope, &mut entry)?; }
            tx.put("publications", &entry.id, 0, &entry)?;
            tx.put("publication_receipts", &receipt_key, 0, &Receipt { fingerprint, entry_id: entry.id.clone() })?;
            tx.emit(EventInput::new("communication.published", json!({"entryId":entry.id,"mode":entry.mode,"revision":entry.revision,"bytes":entry.body.len()})).agent(&scope.author).run(&scope.run_id))?;
            Ok(entry)
        })
    }
    /// None is owner access. An empty author set grants access to no agent records.
    pub fn page(
        db: &Store,
        run_id: &str,
        query: &PublicationQuery,
        authors: Option<&[String]>,
    ) -> Result<PublicationPage> {
        bounded(run_id, 200, "Запуск")?;
        require(
            query.after >= 0 && (1..=50).contains(&query.limit),
            "Некорректная страница публикаций",
        )?;
        require(
            authors.is_none_or(|a| a.len() <= 5000),
            "Слишком много авторов",
        )?;
        for filter in [query.entry_id.as_deref(), query.slug.as_deref()]
            .into_iter()
            .flatten()
        {
            bounded(filter, 200, "Фильтр")?;
        }
        let mut stmt = db.connection().prepare("SELECT rowid,document FROM entities e WHERE collection='publications' AND json_extract(document,'$.runId')=?1 AND rowid>?2 AND (?3 IS NULL OR id=?3) AND (?4 IS NULL OR json_extract(document,'$.slug')=?4) AND (?5 OR NOT EXISTS (SELECT 1 FROM json_each(e.document,'$.contributors') c WHERE c.value NOT IN (SELECT value FROM json_each(?6)))) AND (NOT ?8 OR json_extract(e.document,'$.mode')!='wiki' OR e.id IN (SELECT json_extract(h.document,'$.entryId') FROM entities h WHERE h.collection='publication_heads' AND json_extract(h.document,'$.runId')=?1)) ORDER BY rowid LIMIT ?7")?;
        let rows = stmt.query_map(
            params![
                run_id,
                query.after,
                query.entry_id,
                query.slug,
                authors.is_none(),
                serde_json::to_string(&authors.unwrap_or_default())?,
                (query.limit + 1) as i64,
                query.latest_only
            ],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )?;
        let mut page = PublicationPage {
            items: vec![],
            cursor: query.after,
            has_more: false,
        };
        for row in rows {
            let (cursor, document) = row?;
            if page.items.len() == query.limit {
                page.has_more = true;
                break;
            }
            page.items.push(serde_json::from_str(&document)?);
            page.cursor = cursor;
        }
        Ok(page)
    }
}
