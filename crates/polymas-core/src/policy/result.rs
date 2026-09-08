use super::*;

pub const MAX_OPERATION_RESULT_BYTES: usize = 4 * 1024 * 1024 + 8192;
const PAGE_BYTES: usize = 32768;
const INLINE_BYTES: usize = 16384;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultMetadata {
    pub byte_length: usize,
    pub sha256: String,
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResultQuery {
    pub offset: usize,
    pub limit: usize,
}
impl Default for ResultQuery {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: PAGE_BYTES,
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPage {
    pub metadata: ResultMetadata,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub chunk: String,
}

/// Private immutable JSON. Diagnostic formatting deliberately omits its content.
pub struct OperationResult {
    text: String,
    metadata: ResultMetadata,
}
impl OperationResult {
    pub fn new(value: Value) -> Result<Self> {
        let text = serde_json::to_string(&canonical(value))?;
        require(
            text.len() <= MAX_OPERATION_RESULT_BYTES,
            "Результат операции превышает допустимый размер",
        )?;
        let metadata = ResultMetadata {
            byte_length: text.len(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        };
        Ok(Self { text, metadata })
    }
    pub fn metadata(&self) -> &ResultMetadata {
        &self.metadata
    }
    pub fn inline(&self) -> Result<Option<Value>> {
        if self.text.len() <= INLINE_BYTES {
            Ok(Some(serde_json::from_str(&self.text)?))
        } else {
            Ok(None)
        }
    }
    pub fn page(&self, query: ResultQuery) -> Result<ResultPage> {
        require(
            (4..=PAGE_BYTES).contains(&query.limit),
            "Размер страницы результата: 4–32768 байт",
        )?;
        require(
            query.offset <= self.text.len() && self.text.is_char_boundary(query.offset),
            "Некорректное смещение результата",
        )?;
        let mut end = query
            .offset
            .saturating_add(query.limit)
            .min(self.text.len());
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        Ok(ResultPage {
            metadata: self.metadata.clone(),
            offset: query.offset,
            next_offset: (end < self.text.len()).then_some(end),
            chunk: self.text[query.offset..end].to_owned(),
        })
    }
}
impl ApprovalLedger {
    pub fn result(db: &Store, id: &str) -> Result<Option<OperationResult>> {
        db.get::<Value>("approval_results", id)?
            .map(|s| OperationResult::new(s.value))
            .transpose()
    }
}
