use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum KnowledgeCommand {
    AddSource {
        draft: SourceDraft,
    },
    Publish {
        draft: ClaimDraft,
    },
    Verify {
        claim_id: String,
        revision: i64,
        status: Verification,
    },
    Relate {
        from: String,
        to: String,
        relation: Relation,
    },
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    fingerprint: String,
    result: Value,
}
impl Knowledge {
    /// The caller must authorize the actor and operation. Receipt and mutation
    /// commit together, so a lost response is safely retried with the same key.
    pub fn execute(
        db: &mut Store,
        actor: &str,
        key: &str,
        command: KnowledgeCommand,
        timestamp: i64,
    ) -> Result<Value> {
        bounded(actor, 200, "Автор команды")?;
        bounded(key, 200, "Ключ команды")?;
        require(timestamp >= 0, "Некорректное время команды")?;
        let receipt_key = format!("{:x}", Sha256::digest(serde_json::to_vec(&(actor, key))?));
        let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&command)?));
        db.transaction(|tx| {
            if let Some(receipt) = tx.get::<Receipt>("knowledge_commands", &receipt_key)? {
                if receipt.value.fingerprint != fingerprint {
                    return Err(Error::Conflict(
                        "Этот ключ уже использован для другой команды знаний".into(),
                    ));
                }
                return Ok(receipt.value.result);
            }
            let result = match command {
                KnowledgeCommand::AddSource { draft } => {
                    serde_json::to_value(Self::source_in(tx, draft)?)?
                }
                KnowledgeCommand::Publish { draft } => {
                    serde_json::to_value(Self::publish_in(tx, actor, draft, timestamp)?)?
                }
                KnowledgeCommand::Verify {
                    claim_id,
                    revision,
                    status,
                } => {
                    let stored = tx
                        .get::<Claim>("claims", &claim_id)?
                        .ok_or_else(|| Error::NotFound("Утверждение не найдено".into()))?;
                    if stored.revision != revision {
                        return Err(Error::Conflict(
                            "Утверждение изменилось; перечитайте решение перед проверкой".into(),
                        ));
                    }
                    serde_json::to_value(Self::verify_in(tx, &claim_id, status, actor, timestamp)?)?
                }
                KnowledgeCommand::Relate { from, to, relation } => {
                    serde_json::to_value(Self::relate_in(tx, &from, &to, relation, timestamp)?)?
                }
            };
            tx.put(
                "knowledge_commands",
                &receipt_key,
                0,
                &Receipt {
                    fingerprint,
                    result: result.clone(),
                },
            )?;
            Ok(result)
        })
    }
}
