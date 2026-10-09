//! Transcript history in SQLite (rusqlite, bundled SQLite).

use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    /// Unix time in milliseconds.
    pub created_at_ms: i64,
    pub duration_secs: f32,
    pub model_id: String,
}

/// Send + Sync history store.
pub struct History {
    conn: Mutex<Connection>,
}

impl History {
    /// Opens (creating if needed) e.g. `~/Library/Application Support/EasyVox/history.sqlite3`.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> anyhow::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS transcripts (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 text TEXT NOT NULL,
                 created_at_ms INTEGER NOT NULL,
                 duration_secs REAL NOT NULL,
                 model_id TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS transcripts_created ON transcripts(created_at_ms DESC);",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn add(&self, text: &str, duration_secs: f32, model_id: &str) -> anyhow::Result<HistoryEntry> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO transcripts (text, created_at_ms, duration_secs, model_id) VALUES (?1, ?2, ?3, ?4)",
            params![text, now, duration_secs as f64, model_id],
        )?;
        Ok(HistoryEntry {
            id: conn.last_insert_rowid(),
            text: text.into(),
            created_at_ms: now,
            duration_secs,
            model_id: model_id.into(),
        })
    }

    /// Newest first.
    pub fn recent(&self, limit: usize) -> anyhow::Result<Vec<HistoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut st = conn.prepare(
            "SELECT id, text, created_at_ms, duration_secs, model_id FROM transcripts ORDER BY created_at_ms DESC, id DESC LIMIT ?1",
        )?;
        let rows = st.query_map(params![limit as i64], |r| {
            Ok(HistoryEntry {
                id: r.get(0)?,
                text: r.get(1)?,
                created_at_ms: r.get(2)?,
                duration_secs: r.get::<_, f64>(3)? as f32,
                model_id: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_list_newest_first() {
        let h = History::open_in_memory().unwrap();
        h.add("first", 1.5, "parakeet-unified-en-0.6b-q5_k_m-gguf").unwrap();
        h.add("second", 2.0, "whisper-base.en-q8_0-gguf").unwrap();
        let r = h.recent(10).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].text, "second");
        assert_eq!(r[1].model_id, "parakeet-unified-en-0.6b-q5_k_m-gguf");
        assert_eq!(h.recent(1).unwrap().len(), 1);
    }

    #[test]
    fn persists_to_file() {
        let dir = std::env::temp_dir().join(format!("easyvox-hist-{}", std::process::id()));
        let p = dir.join("history.sqlite3");
        History::open(&p).unwrap().add("kept", 3.0, "m").unwrap();
        assert_eq!(History::open(&p).unwrap().recent(5).unwrap()[0].text, "kept");
        let _ = std::fs::remove_dir_all(dir);
    }
}
