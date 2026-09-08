//! Immutable, bounded event windows. Reading a recording never invokes the scheduler.
use crate::{
    error::{Result, require},
    storage::{Event, EventInput, Store},
};
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventWindow {
    pub events: Vec<Event>,
    pub cursor: i64,
    pub through: i64,
    pub complete: bool,
}
impl Store {
    pub fn recorded_events(
        &self,
        run_id: &str,
        after: i64,
        through: i64,
        limit: usize,
    ) -> Result<EventWindow> {
        require(
            !run_id.is_empty() && run_id.len() <= 200,
            "Некорректный запуск",
        )?;
        require(
            after >= 0 && through >= after && through <= self.last_sequence()?,
            "Некорректные границы записи",
        )?;
        require(
            (1..=500).contains(&limit),
            "Размер страницы должен быть от 1 до 500",
        )?;
        let mut stmt = self.connection().prepare("SELECT sequence,event_id,observed_at,kind,agent_id,run_id,body FROM events WHERE run_id=?1 AND sequence>?2 AND sequence<=?3 ORDER BY sequence LIMIT ?4")?;
        let mut rows = stmt.query(params![run_id, after, through, (limit + 1) as i64])?;
        let mut events = vec![];
        let mut size = 256;
        let mut complete = true;
        while let Some(row) = rows.next()? {
            let event = Event {
                sequence: row.get(0)?,
                event_id: row.get(1)?,
                observed_at: row.get(2)?,
                input: EventInput {
                    kind: row.get(3)?,
                    agent_id: row.get(4)?,
                    run_id: row.get(5)?,
                    body: serde_json::from_str(&row.get::<_, String>(6)?)?,
                },
            };
            let bytes = serde_json::to_vec(&event)?.len() + 1;
            if events.len() == limit || size + bytes > 2 * 1024 * 1024 {
                require(
                    !events.is_empty(),
                    "Событие превышает допустимый размер страницы",
                )?;
                complete = false;
                break;
            }
            size += bytes;
            events.push(event);
        }
        let cursor = if complete {
            through
        } else {
            events.last().map_or(after, |e| e.sequence)
        };
        Ok(EventWindow {
            events,
            cursor,
            through,
            complete,
        })
    }
}
