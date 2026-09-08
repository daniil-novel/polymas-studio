//! Persistent Codex session ownership, independent of an agent's latest conversation.
use crate::{
    error::{Error, Result, require},
    storage::{Store, WriteTransaction},
};
use serde::{Deserialize, Serialize};

const OWNERS: &str = "codex_session_owners";
#[derive(Serialize, Deserialize)]
struct Owner {
    agent_id: String,
}
pub struct SessionRegistry;
fn canonical(session: &str) -> Result<String> {
    uuid::Uuid::parse_str(session)
        .map(|id| id.to_string())
        .map_err(|_| Error::Validation("Некорректный UUID сессии Codex".into()))
}
impl SessionRegistry {
    pub fn verify(db: &Store, session: &str, agent: &str) -> Result<()> {
        let owner = db
            .get::<Owner>(OWNERS, &canonical(session)?)?
            .ok_or_else(|| Error::Denied("Владелец сессии не подтверждён".into()))?;
        require(
            owner.value.agent_id == agent,
            "Сессия принадлежит другому экземпляру",
        )
    }
    pub fn register(tx: &WriteTransaction<'_>, session: &str, agent: &str) -> Result<()> {
        require(
            !agent.is_empty() && agent.len() <= 200,
            "Не указан владелец сессии",
        )?;
        let key = canonical(session)?;
        if let Some(owner) = tx.get::<Owner>(OWNERS, &key)? {
            require(
                owner.value.agent_id == agent,
                "Сессия уже принадлежит другому экземпляру",
            )
        } else {
            tx.put(
                OWNERS,
                &key,
                0,
                &Owner {
                    agent_id: agent.into(),
                },
            )?;
            Ok(())
        }
    }
    /// Metadata only. Conflicting historical ownership fails the whole migration.
    pub fn migrate(db: &mut Store) -> Result<()> {
        db.transaction(|tx| {
            if tx.get::<bool>("schema_updates", "codex-session-owners-v1")?.is_some() { return Ok(()); }
            let mut query = tx.connection().prepare(
                "SELECT json_extract(document,'$.sessionId'), json_extract(document,'$.agentId')
                 FROM entities WHERE collection IN ('agent_runtime','task_runtime')
                 UNION
                 SELECT COALESCE(json_extract(body,'$.sessionId'), json_extract(body,'$.data.thread_id')), agent_id
                 FROM events WHERE kind IN ('runtime.session','runtime.result_saved')
                    OR (kind='runtime.event' AND json_extract(body,'$.data.type')='thread.started')"
            )?;
            let rows = query.query_map([], |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)))?;
            for row in rows {
                let (session, agent) = row?;
                if let Some(session) = session {
                    Self::register(tx, &session, agent.as_deref().unwrap_or_default())?;
                }
            }
            tx.put("schema_updates", "codex-session-owners-v1", 0, &true)?;
            Ok(())
        })
    }
}
