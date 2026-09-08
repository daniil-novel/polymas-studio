use super::*;
use crate::storage::Stored;
use rusqlite::params;
use serde_json::Value;

#[derive(Clone, Copy)]
pub enum KnowledgeView {
    Sources,
    Claims,
    Links,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct KnowledgeQuery {
    pub namespace: Option<String>,
    pub query: String,
    pub source_id: Option<String>,
    pub claim_id: Option<String>,
    pub status: Option<Verification>,
    pub include_expired: bool,
    pub cursor: Option<String>,
    pub limit: usize,
}
impl Default for KnowledgeQuery {
    fn default() -> Self {
        Self {
            namespace: None,
            query: String::new(),
            source_id: None,
            claim_id: None,
            status: None,
            include_expired: false,
            cursor: None,
            limit: 30,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgePage {
    pub items: Vec<Stored<Value>>,
    pub next_cursor: Option<String>,
    pub observed_at: i64,
}
impl Knowledge {
    /// Keyset pages are bounded and filtered in storage before deserialization.
    /// Admin callers may omit namespace; gateway callers must authorize it first.
    pub fn page(
        db: &Store,
        view: KnowledgeView,
        input: &KnowledgeQuery,
        timestamp: i64,
    ) -> Result<KnowledgePage> {
        require(
            input.query.len() <= 200 && (1..=100).contains(&input.limit),
            "Поиск до 200 байт; страница 1–100",
        )?;
        for value in [&input.cursor, &input.source_id, &input.claim_id]
            .into_iter()
            .flatten()
        {
            bounded(value, 200, "Ключ поиска")?;
        }
        if let Some(value) = &input.namespace {
            namespace(value)?;
        }
        let collection = match view {
            KnowledgeView::Sources => "sources",
            KnowledgeView::Claims => "claims",
            KnowledgeView::Links => "knowledge_links",
        };
        let status = input
            .status
            .map(serde_json::to_value)
            .transpose()?
            .and_then(|v| v.as_str().map(str::to_owned));
        // Source/claim text is searched literally (including '%' and '_').
        let mut query = db.connection().prepare(
            "SELECT e.id,e.revision,e.document FROM entities e
             WHERE e.collection=?1 AND (?2 IS NULL OR json_extract(e.document,'$.namespace')=?2)
             AND (?3 IS NULL OR e.id>?3)
             AND (?4='' OR instr(polymas_lower(COALESCE(json_extract(e.document,'$.assertion'),json_extract(e.document,'$.name'),'')),?4)>0)
             AND (?1!='claims' OR ((?5 IS NULL OR json_extract(e.document,'$.sourceId')=?5)
                AND (?6 IS NULL OR json_extract(e.document,'$.status')=?6)
                AND (?7 OR json_extract(e.document,'$.expiresAt')>?8)))
             AND (?1!='knowledge_links' OR ((?9 IS NULL OR json_extract(e.document,'$.from')=?9 OR json_extract(e.document,'$.to')=?9)
                AND (?7 OR (EXISTS(SELECT 1 FROM entities a WHERE a.collection='claims' AND a.id=json_extract(e.document,'$.from') AND json_extract(a.document,'$.expiresAt')>?8)
                        AND EXISTS(SELECT 1 FROM entities b WHERE b.collection='claims' AND b.id=json_extract(e.document,'$.to') AND json_extract(b.document,'$.expiresAt')>?8)))))
             ORDER BY e.id LIMIT ?10"
        )?;
        let rows = query.query_map(
            params![
                collection,
                input.namespace,
                input.cursor,
                input.query.to_lowercase(),
                input.source_id,
                status,
                input.include_expired,
                timestamp,
                input.claim_id,
                (input.limit + 1) as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )?;
        let mut items = vec![];
        let mut last = None;
        let mut size = 0;
        for row in rows {
            let (id, revision, document) = row?;
            if items.len() == input.limit
                || (!items.is_empty() && size + document.len() > 512 * 1024)
            {
                return Ok(KnowledgePage {
                    items,
                    next_cursor: last,
                    observed_at: timestamp,
                });
            }
            require(
                document.len() <= 512 * 1024,
                "Запись знаний превышает размер страницы",
            )?;
            size += document.len();
            items.push(Stored {
                revision,
                value: serde_json::from_str(&document)?,
            });
            last = Some(id);
        }
        Ok(KnowledgePage {
            items,
            next_cursor: None,
            observed_at: timestamp,
        })
    }
}
