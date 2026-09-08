use crate::error::{Error, Result, require};
use crate::registry::id;
use crate::storage::{EventInput, Store};
use crate::topology::Architecture;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct MessageInput {
    pub recipient: String,
    pub kind: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub ttl_seconds: i64,
}
impl Default for MessageInput {
    fn default() -> Self {
        Self {
            recipient: String::new(),
            kind: "evidence".into(),
            body: String::new(),
            reply_to: None,
            ttl_seconds: 600,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub run_id: String,
    pub sender: String,
    pub recipient: String,
    pub kind: String,
    pub body: String,
    pub correlation_id: String,
    pub reply_to: Option<String>,
    pub hops: u32,
    pub created_at: i64,
    pub expires_at: i64,
    pub state: String,
    pub delivered_attempt: Option<String>,
}
pub struct Router;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxPage {
    pub items: Vec<Message>,
    pub cursor: i64,
    pub has_more: bool,
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn read_message(conn: &rusqlite::Connection, id: &str) -> Result<Message> {
    let body: Option<String> = conn
        .query_row("SELECT document FROM messages WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(serde_json::from_str(&body.ok_or_else(|| {
        Error::NotFound("Сообщение не найдено".into())
    })?)?)
}
fn save_message(conn: &rusqlite::Connection, message: &Message) -> Result<()> {
    conn.execute(
        "UPDATE messages SET state=?1,document=?2 WHERE id=?3",
        params![message.state, serde_json::to_string(message)?, message.id],
    )?;
    Ok(())
}
impl Router {
    pub fn pending_senders(
        db: &Store,
        run_id: &str,
        recipient: &str,
        condition: &crate::scheduler::WaitCondition,
        timestamp: i64,
    ) -> Result<Vec<String>> {
        let mut query = db.connection().prepare("SELECT DISTINCT sender FROM messages WHERE run_id=?1 AND recipient=?2 AND state='queued' AND expires_at>?3 AND (?4 IS NULL OR sender=?4) AND (?5 IS NULL OR correlation_id=?5) LIMIT 5000")?;
        let rows = query.query_map(
            params![
                run_id,
                recipient,
                timestamp,
                condition.sender,
                condition.correlation_id
            ],
            |r| r.get::<_, String>(0),
        )?;
        rows.map(|r| Ok(r?)).collect()
    }
    pub fn get_for_recipient(
        db: &Store,
        run_id: &str,
        recipient: &str,
        message_id: &str,
    ) -> Result<Message> {
        let message = read_message(db.connection(), message_id)?;
        require(
            message.run_id == run_id && message.recipient == recipient,
            "Сообщение не принадлежит inbox этого запуска",
        )?;
        Ok(message)
    }
    pub fn pending_for_run(
        db: &Store,
        run_id: &str,
        recipient: &str,
        timestamp: i64,
        limit: usize,
    ) -> Result<Vec<Message>> {
        require((1..=100).contains(&limit), "Размер inbox batch: 1–100")?;
        let mut query = db.connection().prepare("SELECT document FROM messages WHERE run_id=?1 AND recipient=?2 AND state='queued' AND expires_at>?3 ORDER BY rowid LIMIT ?4")?;
        let rows = query.query_map(params![run_id, recipient, timestamp, limit as i64], |r| {
            r.get::<_, String>(0)
        })?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    /// Queued messages enter an exec turn through its recorded stdin batch. The
    /// read-only MCP inbox exposes only messages already delivered to this agent.
    pub fn inbox(
        db: &Store,
        run_id: &str,
        recipient: &str,
        after: i64,
        limit: usize,
        timestamp: i64,
    ) -> Result<InboxPage> {
        require(
            after >= 0 && (1..=100).contains(&limit),
            "Некорректный cursor или размер страницы 1–100",
        )?;
        let mut query = db.connection().prepare("SELECT rowid,document FROM messages WHERE run_id=?1 AND recipient=?2 AND rowid>?3 AND state IN ('delivered_to_context','acknowledged') AND expires_at>?4 ORDER BY rowid LIMIT ?5")?;
        let rows = query.query_map(
            params![run_id, recipient, after, timestamp, (limit + 1) as i64],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )?;
        let mut page = InboxPage {
            items: vec![],
            cursor: after,
            has_more: false,
        };
        for row in rows {
            let (cursor, document) = row?;
            if page.items.len() == limit {
                page.has_more = true;
                break;
            }
            page.items.push(serde_json::from_str(&document)?);
            page.cursor = cursor;
        }
        Ok(page)
    }
    pub fn send(
        db: &mut Store,
        architecture: &Architecture,
        run_id: &str,
        sender: &str,
        key: &str,
        input: MessageInput,
        timestamp: i64,
    ) -> Result<Message> {
        require(
            !key.is_empty() && key.len() <= 128 && !run_id.is_empty(),
            "Некорректный ключ сообщения",
        )?;
        require(
            architecture.permits_message(sender, &input.recipient),
            "Топология не разрешает эту связь",
        )?;
        require(
            !input.body.trim().is_empty() && input.body.len() <= 64 * 1024,
            "Сообщение должно содержать от 1 байта до 64 КиБ",
        )?;
        require(
            (1..=3600).contains(&input.ttl_seconds),
            "TTL сообщения: 1–3600 секунд",
        )?;
        require(
            [
                "task_request",
                "evidence",
                "question",
                "answer",
                "critique",
                "result",
                "escalation",
            ]
            .contains(&input.kind.as_str()),
            "Неизвестный тип сообщения",
        )?;
        let fingerprint = digest(&serde_json::to_string(&input)?);
        db.transaction(|tx|{
            let conn=tx.connection();
            let prior:Option<(String,String)>=conn.query_row("SELECT fingerprint,document FROM messages WHERE run_id=?1 AND sender=?2 AND idempotency_key=?3",params![run_id,sender,key],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((old,json))=prior {if old!=fingerprint{return Err(Error::Conflict("Ключ сообщения уже использован иначе".into()));}return Ok(serde_json::from_str(&json)?);}
            let count:i64=conn.query_row("SELECT COUNT(*) FROM messages WHERE run_id=?1",[run_id],|r|r.get(0))?;
            require(count<(architecture.max_messages_per_run as i64),"Исчерпан бюджет сообщений запуска")?;
            let (correlation,hops)=if let Some(parent_id)=&input.reply_to {
                let parent=read_message(conn,parent_id)?;
                require(parent.run_id==run_id && parent.recipient==sender,"Нельзя ответить от чужого получателя или другого запуска")?;
                (parent.correlation_id,parent.hops+1)
            }else{(id("conversation"),0)};
            require(hops<=architecture.message_hop_limit,"Достигнут предел hops; нужна эскалация оператору")?;
            let body_hash=digest(input.body.trim());
            let duplicate:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM messages WHERE correlation_id=?1 AND sender=?2 AND recipient=?3 AND body_hash=?4)",params![correlation,sender,input.recipient,body_hash],|r|r.get(0))?;
            require(!duplicate,"Повтор содержания в цепочке; автоматический цикл остановлен")?;
            let expires_at=timestamp.checked_add(input.ttl_seconds).ok_or_else(||Error::Validation("Переполнение времени".into()))?;
            let message=Message {id:id("message"),run_id:run_id.into(),sender:sender.into(),recipient:input.recipient,kind:input.kind,
                body:input.body,correlation_id:correlation,reply_to:input.reply_to,hops,created_at:timestamp,expires_at,state:"queued".into(),delivered_attempt:None};
            conn.execute("INSERT INTO messages(id,run_id,sender,recipient,idempotency_key,fingerprint,correlation_id,body_hash,state,expires_at,document) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![message.id,run_id,sender,message.recipient,key,fingerprint,message.correlation_id,body_hash,message.state,message.expires_at,serde_json::to_string(&message)?])?;
            tx.emit(EventInput::new("message.queued",json!({"messageId":message.id,"from":sender,"to":message.recipient,"kind":message.kind})).agent(sender).run(run_id))?;
            Ok(message)
        })
    }
    pub fn pending(
        db: &Store,
        recipient: &str,
        timestamp: i64,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let mut query=db.connection().prepare("SELECT document FROM messages WHERE recipient=?1 AND state='queued' AND expires_at>?2 ORDER BY rowid LIMIT ?3")?;
        let rows = query.query_map(
            params![recipient, timestamp, limit.clamp(1, 100) as i64],
            |r| r.get::<_, String>(0),
        )?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn history(db: &Store, run_id: Option<&str>, limit: usize) -> Result<Vec<Message>> {
        let mut query=db.connection().prepare("SELECT document FROM messages WHERE (?1 IS NULL OR run_id=?1) ORDER BY rowid DESC LIMIT ?2")?;
        let rows = query.query_map(params![run_id, limit.clamp(1, 5000) as i64], |r| {
            r.get::<_, String>(0)
        })?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn deliver(
        db: &mut Store,
        recipient: &str,
        ids: &[String],
        attempt: &str,
        timestamp: i64,
    ) -> Result<()> {
        require(
            ids.len() <= 100 && !attempt.is_empty(),
            "Некорректная доставка контекста",
        )?;
        db.transaction(|tx|{
            for id in ids {
                let mut message=read_message(tx.connection(),id)?;
                require(message.recipient==recipient,"Чужой почтовый ящик")?;
                if message.delivered_attempt.as_deref()==Some(attempt){continue;}
                require(message.state=="queued" && message.expires_at>timestamp,"Сообщение уже доставлено или истекло")?;
                message.state="delivered_to_context".into();message.delivered_attempt=Some(attempt.into());save_message(tx.connection(),&message)?;
                tx.emit(EventInput::new("message.delivered_to_context",json!({"messageId":id,"from":message.sender,"to":recipient,"attemptId":attempt})).agent(recipient).run(&message.run_id))?;
            }Ok(())
        })
    }
    pub fn acknowledge(db: &mut Store, recipient: &str, message_id: &str) -> Result<()> {
        db.transaction(|tx| {
            let mut message = read_message(tx.connection(), message_id)?;
            require(
                message.recipient == recipient
                    && ["delivered_to_context", "acknowledged"].contains(&message.state.as_str()),
                "Подтверждение требует доставки этому получателю",
            )?;
            if message.state == "acknowledged" {
                return Ok(());
            }
            message.state = "acknowledged".into();
            save_message(tx.connection(), &message)?;
            tx.emit(
                EventInput::new("message.acknowledged", json!({"messageId":message_id}))
                    .agent(recipient)
                    .run(&message.run_id),
            )?;
            Ok(())
        })
    }
}
