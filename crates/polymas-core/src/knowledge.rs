//! Provenance is stored separately from agent topology. A model's assertion is
//! unverified until an explicitly authorized reviewer records a decision.
use crate::error::{Error, Result, require};
use crate::registry::{id, now};
use crate::storage::{EventInput, Store, WriteTransaction};
use serde::{Deserialize, Serialize};
use serde_json::json;
mod commands;
mod query;
pub use commands::KnowledgeCommand;
pub use query::{KnowledgePage, KnowledgeQuery, KnowledgeView};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDraft {
    pub name: String,
    pub uri: String,
    pub version: String,
    pub namespace: String,
    pub connection_id: Option<String>,
    pub retention_days: u32,
}
impl SourceDraft {
    pub fn validate(&self) -> Result<()> {
        bounded(&self.name, 200, "Название источника")?;
        bounded(&self.version, 512, "Версия или hash источника")?;
        namespace(&self.namespace)?;
        require(
            (1..=3650).contains(&self.retention_days),
            "Retention: 1–3650 дней",
        )?;
        bounded(&self.uri, 4096, "URI источника")?;
        let uri = url::Url::parse(&self.uri)
            .map_err(|_| Error::Validation("Некорректный URI источника".into()))?;
        require(
            matches!(uri.scheme(), "https" | "http" | "file" | "mcp" | "artifact"),
            "Неподдерживаемая схема URI",
        )?;
        require(
            uri.username().is_empty() && uri.password().is_none() && uri.query().is_none(),
            "URI источника не должен содержать credentials или query-параметры",
        )
    }
}
fn bounded(value: &str, max: usize, name: &str) -> Result<()> {
    require(
        !value.trim().is_empty() && value.len() <= max,
        &format!("{name}: значение отсутствует или слишком длинное"),
    )
}
pub fn namespace(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= 120
            && value
                .chars()
                .all(|c| c.is_alphanumeric() || "_-./".contains(c)),
        "Некорректный namespace",
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub draft: SourceDraft,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimDraft {
    pub assertion: String,
    pub source_id: String,
    pub namespace: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Unverified,
    Verified,
    Disputed,
    Rejected,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub id: String,
    pub assertion: String,
    pub source_id: String,
    pub source_version: String,
    pub source_uri: String,
    pub namespace: String,
    pub author: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub status: Verification,
    pub verified_by: Option<String>,
    pub verified_at: Option<i64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Supports,
    Contradicts,
    DerivedFrom,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeLink {
    pub id: String,
    pub from: String,
    pub to: String,
    pub relation: Relation,
    pub namespace: String,
    pub created_at: i64,
}

pub struct Knowledge;
impl Knowledge {
    pub fn add_source(db: &mut Store, draft: SourceDraft) -> Result<Source> {
        db.transaction(|tx| Self::source_in(tx, draft))
    }
    fn source_in(tx: &WriteTransaction<'_>, draft: SourceDraft) -> Result<Source> {
        draft.validate()?;
        let source = Source {
            id: id("source"),
            created_at: now(),
            draft,
        };
        tx.put("sources", &source.id, 0, &source)?;
        tx.emit(EventInput::new("knowledge.source_added",json!({"sourceId":source.id,"namespace":source.draft.namespace,"version":source.draft.version})))?;
        Ok(source)
    }
    pub fn publish(
        db: &mut Store,
        author: &str,
        draft: ClaimDraft,
        timestamp: i64,
    ) -> Result<Claim> {
        db.transaction(|tx| Self::publish_in(tx, author, draft, timestamp))
    }
    fn publish_in(
        tx: &WriteTransaction<'_>,
        author: &str,
        draft: ClaimDraft,
        timestamp: i64,
    ) -> Result<Claim> {
        bounded(author, 200, "Автор")?;
        bounded(&draft.assertion, 65536, "Утверждение")?;
        namespace(&draft.namespace)?;
        let source = tx
            .get::<Source>("sources", &draft.source_id)?
            .ok_or_else(|| Error::NotFound("Источник не найден".into()))?
            .value;
        require(
            source.draft.namespace == draft.namespace,
            "Источник находится в другом namespace",
        )?;
        let claim = Claim {
            id: id("claim"),
            assertion: draft.assertion,
            source_id: source.id,
            source_version: source.draft.version,
            source_uri: source.draft.uri,
            namespace: draft.namespace,
            author: author.into(),
            created_at: timestamp,
            expires_at: timestamp
                .checked_add(i64::from(source.draft.retention_days) * 86400)
                .ok_or_else(|| Error::Validation("Некорректное время".into()))?,
            status: Verification::Unverified,
            verified_by: None,
            verified_at: None,
        };
        tx.put("claims", &claim.id, 0, &claim)?;
        tx.emit(
            EventInput::new(
                "knowledge.claim_published",
                json!({"claimId":claim.id,"sourceId":claim.source_id,"namespace":claim.namespace}),
            )
            .agent(author),
        )?;
        Ok(claim)
    }
    pub fn claims(db: &Store, namespace: &str, timestamp: i64) -> Result<Vec<Claim>> {
        // Filter before deserialization/return; inaccessible namespaces never
        // become a caller-visible mixed result that the model must filter.
        let mut stmt = db.connection().prepare("SELECT document FROM entities WHERE collection='claims' AND json_extract(document,'$.namespace')=?1 AND json_extract(document,'$.expiresAt')>?2 ORDER BY updated_at DESC,id LIMIT 5000")?;
        let rows = stmt.query_map(rusqlite::params![namespace, timestamp], |r| {
            r.get::<_, String>(0)
        })?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn verify(
        db: &mut Store,
        claim_id: &str,
        status: Verification,
        reviewer: &str,
        timestamp: i64,
    ) -> Result<Claim> {
        db.transaction(|tx| Self::verify_in(tx, claim_id, status, reviewer, timestamp))
    }
    fn verify_in(
        tx: &WriteTransaction<'_>,
        claim_id: &str,
        status: Verification,
        reviewer: &str,
        timestamp: i64,
    ) -> Result<Claim> {
        bounded(reviewer, 200, "Проверяющий")?;
        require(
            status != Verification::Unverified,
            "Укажите решение проверки",
        )?;
        let mut stored = tx
            .get::<Claim>("claims", claim_id)?
            .ok_or_else(|| Error::NotFound("Утверждение не найдено".into()))?;
        require(
            timestamp < stored.value.expires_at,
            "Retention утверждения истёк",
        )?;
        stored.value.status = status;
        stored.value.verified_by = Some(reviewer.into());
        stored.value.verified_at = Some(timestamp);
        tx.put("claims", claim_id, stored.revision, &stored.value)?;
        tx.emit(EventInput::new(
            "knowledge.verification_changed",
            json!({"claimId":claim_id,"status":status,"reviewer":reviewer}),
        ))?;
        Ok(stored.value)
    }
    pub fn relate(
        db: &mut Store,
        from: &str,
        to: &str,
        relation: Relation,
        timestamp: i64,
    ) -> Result<KnowledgeLink> {
        db.transaction(|tx| Self::relate_in(tx, from, to, relation, timestamp))
    }
    fn relate_in(
        tx: &WriteTransaction<'_>,
        from: &str,
        to: &str,
        relation: Relation,
        timestamp: i64,
    ) -> Result<KnowledgeLink> {
        require(from != to, "Связь требует разных утверждений")?;
        let a = tx
            .get::<Claim>("claims", from)?
            .ok_or_else(|| Error::NotFound("Утверждение не найдено".into()))?
            .value;
        let b = tx
            .get::<Claim>("claims", to)?
            .ok_or_else(|| Error::NotFound("Утверждение не найдено".into()))?
            .value;
        require(
            a.namespace == b.namespace && a.expires_at > timestamp && b.expires_at > timestamp,
            "Namespace или retention не допускает связь",
        )?;
        let link = KnowledgeLink {
            id: id("knowledge_link"),
            from: from.into(),
            to: to.into(),
            relation,
            namespace: a.namespace,
            created_at: timestamp,
        };
        tx.put("knowledge_links", &link.id, 0, &link)?;
        tx.emit(EventInput::new(
            "knowledge.relation_created",
            json!({"linkId":link.id,"from":from,"to":to,"relation":relation}),
        ))?;
        Ok(link)
    }
}
