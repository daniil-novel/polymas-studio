use crate::error::{Error, Result, require};
use crate::registry::{id, now};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stored<T> {
    pub revision: i64,
    pub value: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub kind: String,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub body: Value,
}
impl EventInput {
    pub fn new(kind: &str, body: Value) -> Self {
        Self {
            kind: kind.into(),
            agent_id: None,
            run_id: None,
            body,
        }
    }
    pub fn agent(mut self, id: &str) -> Self {
        self.agent_id = Some(id.into());
        self
    }
    pub fn run(mut self, id: &str) -> Self {
        self.run_id = Some(id.into());
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub sequence: i64,
    pub event_id: String,
    pub observed_at: String,
    #[serde(flatten)]
    pub input: EventInput,
}

pub struct Store {
    connection: Connection,
}
pub struct WriteTransaction<'a> {
    transaction: Transaction<'a>,
}

impl Store {
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::initialize(Connection::open(path)?)
    }
    pub fn memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?)
    }
    fn initialize(connection: Connection) -> Result<Self> {
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 2 {
            return Err(Error::Storage(
                "База создана более новой версией приложения".into(),
            ));
        }
        connection.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS entities (
                collection TEXT NOT NULL, id TEXT NOT NULL, revision INTEGER NOT NULL CHECK(revision > 0),
                document TEXT NOT NULL CHECK(json_valid(document)), updated_at TEXT NOT NULL,
                PRIMARY KEY(collection,id));
            CREATE TABLE IF NOT EXISTS events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT UNIQUE NOT NULL,
                observed_at TEXT NOT NULL, kind TEXT NOT NULL, agent_id TEXT, run_id TEXT,
                body TEXT NOT NULL CHECK(json_valid(body)));
            CREATE INDEX IF NOT EXISTS events_run_sequence ON events(run_id,sequence);
            CREATE INDEX IF NOT EXISTS events_agent_sequence ON events(agent_id,sequence);
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY, run_id TEXT NOT NULL, sender TEXT NOT NULL, recipient TEXT NOT NULL,
                idempotency_key TEXT NOT NULL, fingerprint TEXT NOT NULL, correlation_id TEXT NOT NULL,
                body_hash TEXT NOT NULL, state TEXT NOT NULL, expires_at INTEGER NOT NULL,
                document TEXT NOT NULL CHECK(json_valid(document)), UNIQUE(run_id,sender,idempotency_key));
            CREATE INDEX IF NOT EXISTS messages_inbox ON messages(recipient,state,expires_at);
            CREATE INDEX IF NOT EXISTS messages_run ON messages(run_id);
            CREATE INDEX IF NOT EXISTS messages_dedup ON messages(correlation_id,sender,recipient,body_hash);
            PRAGMA user_version=2; COMMIT;")?;
        Ok(Self { connection })
    }
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&WriteTransaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let tx = WriteTransaction {
            transaction: self
                .connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?,
        };
        let result = f(&tx)?;
        tx.transaction.commit()?;
        Ok(result)
    }
    pub fn get<T: DeserializeOwned>(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<Stored<T>>> {
        read(&self.connection, collection, id)
    }
    pub fn list<T: DeserializeOwned>(
        &self,
        collection: &str,
        limit: usize,
    ) -> Result<Vec<Stored<T>>> {
        list(&self.connection, collection, limit)
    }
    pub fn events(&self, after: i64, limit: usize, run_id: Option<&str>) -> Result<Vec<Event>> {
        let mut stmt = self.connection.prepare("SELECT sequence,event_id,observed_at,kind,agent_id,run_id,body FROM events WHERE sequence>?1 AND (?2 IS NULL OR run_id=?2) ORDER BY sequence LIMIT ?3")?;
        let rows = stmt.query_map(params![after, run_id, limit.clamp(1, 5000) as i64], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (sequence, event_id, observed_at, kind, agent_id, run_id, body) = row?;
            Ok(Event {
                sequence,
                event_id,
                observed_at,
                input: EventInput {
                    kind,
                    agent_id,
                    run_id,
                    body: serde_json::from_str(&body)?,
                },
            })
        })
        .collect()
    }
    pub fn last_sequence(&self) -> Result<i64> {
        Ok(self
            .connection
            .query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get(0)
            })?)
    }
    pub fn backup(&self, destination: &Path) -> Result<()> {
        require(
            !destination.exists(),
            "Резервная копия по этому пути уже существует",
        )?;
        self.connection.backup("main", destination, None)?;
        Ok(())
    }
    pub fn integrity_check(&self) -> Result<()> {
        let result: String = self
            .connection
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if result != "ok" {
            return Err(Error::Storage(result));
        }
        Ok(())
    }
}

fn read<T: DeserializeOwned>(
    connection: &Connection,
    collection: &str,
    id: &str,
) -> Result<Option<Stored<T>>> {
    let row: Option<(i64, String)> = connection
        .query_row(
            "SELECT revision,document FROM entities WHERE collection=?1 AND id=?2",
            params![collection, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    row.map(|(revision, json)| {
        Ok(Stored {
            revision,
            value: serde_json::from_str(&json)?,
        })
    })
    .transpose()
}
fn list<T: DeserializeOwned>(
    connection: &Connection,
    collection: &str,
    limit: usize,
) -> Result<Vec<Stored<T>>> {
    let mut statement = connection.prepare("SELECT revision,document FROM entities WHERE collection=?1 ORDER BY updated_at DESC,id LIMIT ?2")?;
    let rows = statement.query_map(params![collection, limit.clamp(1, 100_000) as i64], |r| {
        Ok((r.get(0)?, r.get::<_, String>(1)?))
    })?;
    rows.map(|r| {
        let (revision, json) = r?;
        Ok(Stored {
            revision,
            value: serde_json::from_str(&json)?,
        })
    })
    .collect()
}

impl WriteTransaction<'_> {
    pub(crate) fn connection(&self) -> &Connection {
        &self.transaction
    }
    pub fn get<T: DeserializeOwned>(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<Stored<T>>> {
        read(&self.transaction, collection, id)
    }
    pub fn list<T: DeserializeOwned>(
        &self,
        collection: &str,
        limit: usize,
    ) -> Result<Vec<Stored<T>>> {
        list(&self.transaction, collection, limit)
    }
    pub fn put<T: Serialize>(
        &self,
        collection: &str,
        id: &str,
        expected: i64,
        value: &T,
    ) -> Result<i64> {
        require((0..i64::MAX).contains(&expected), "Некорректная ревизия")?;
        require(
            !collection.is_empty() && collection.len() <= 80 && !id.is_empty() && id.len() <= 200,
            "Некорректный ключ записи",
        )?;
        let document = serde_json::to_string(value)?;
        require(
            document.len() <= 32 * 1024 * 1024,
            "Запись превышает 32 МиБ",
        )?;
        if expected == 0 {
            let changed = self.transaction.execute("INSERT INTO entities(collection,id,revision,document,updated_at) VALUES(?1,?2,1,?3,?4) ON CONFLICT DO NOTHING", params![collection,id,document,now()])?;
            if changed == 0 {
                return Err(Error::Conflict("Запись уже существует".into()));
            }
        } else {
            let changed = self.transaction.execute("UPDATE entities SET revision=revision+1,document=?1,updated_at=?2 WHERE collection=?3 AND id=?4 AND revision=?5", params![document,now(),collection,id,expected])?;
            if changed == 0 {
                return Err(Error::Conflict(
                    "Конфигурация изменилась; обновите данные".into(),
                ));
            }
        }
        Ok(expected + 1)
    }
    pub fn emit(&self, event: EventInput) -> Result<i64> {
        require(
            !event.kind.is_empty() && event.kind.len() <= 120,
            "Некорректный тип события",
        )?;
        let body = serde_json::to_string(&event.body)?;
        require(body.len() <= 1024 * 1024, "Событие превышает 1 МиБ")?;
        self.transaction.execute("INSERT INTO events(event_id,observed_at,kind,agent_id,run_id,body) VALUES(?1,?2,?3,?4,?5,?6)", params![id("event"),now(),event.kind,event.agent_id,event.run_id,body])?;
        Ok(self.transaction.last_insert_rowid())
    }
}
