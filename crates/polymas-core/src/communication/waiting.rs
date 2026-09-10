use super::*;
impl Publications {
    pub fn has_update(
        db: &Store,
        scope: &ExchangeScope,
        after: i64,
        sender: Option<&str>,
    ) -> Result<bool> {
        require(
            after >= 0 && scope.mode != CommunicationMode::Direct,
            "Некорректное ожидание публикации",
        )?;
        Ok(db.connection().query_row("SELECT EXISTS(SELECT 1 FROM entities e WHERE collection='publications' AND json_extract(document,'$.runId')=?1 AND rowid>?2 AND json_extract(document,'$.author')!=?3 AND (?4 IS NULL OR json_extract(document,'$.author')=?4) AND NOT EXISTS (SELECT 1 FROM json_each(e.document,'$.contributors') c WHERE c.value NOT IN (SELECT value FROM json_each(?5))))", params![scope.run_id, after, scope.author, sender, serde_json::to_string(&scope.readable_authors)?], |r| r.get(0))?)
    }
}
