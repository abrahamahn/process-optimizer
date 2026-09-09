use crate::{integrity, model::*};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, time::Duration};

pub trait Journal {
    fn save(&mut self, session: &Session) -> AppResult<()>;
}

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        let c = Connection::open(path).map_err(|e| e.to_string())?;
        let version: u32 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version > 1 {
            return Err("Unsupported database schema; preserve this store for the matching application version.".into());
        }
        c.busy_timeout(Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, body TEXT NOT NULL, finished INTEGER NOT NULL, stop INTEGER NOT NULL DEFAULT 0, sequence INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), body TEXT NOT NULL);").map_err(|e| e.to_string())?;
        c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS one_unfinished_session ON sessions(finished) WHERE finished=0; PRAGMA user_version=1;").map_err(|e| e.to_string())?;
        Ok(Self { connection: c })
    }

    pub fn create(&mut self, session: &Session) -> AppResult<()> {
        integrity::validate_record(session)?;
        if session.stage != Stage::Pending
            || !session.changes.is_empty()
            || !session.closed.is_empty()
        {
            return Err("Only a pristine pending session can be created.".into());
        }
        let body = integrity::encode(session)?;
        let t = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let active: i64 = t
            .query_row("SELECT COUNT(*) FROM sessions WHERE finished=0", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if active != 0 {
            return Err(
                "An unfinished session exists. Restore or review it before starting another."
                    .into(),
            );
        }
        t.execute("INSERT INTO sessions(id,body,finished,sequence) VALUES(?1,?2,0,(SELECT COALESCE(MAX(sequence),0)+1 FROM sessions))", params![session.id, body]).map_err(|e| e.to_string())?;
        t.commit().map_err(|e| e.to_string())
    }

    pub fn get(&self, id: &str) -> AppResult<Session> {
        let body: String = self
            .connection
            .query_row("SELECT body FROM sessions WHERE id=?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        let session = Self::decode(&body)?;
        if session.id != id {
            return Err("Stored key and recovery ID disagree; preserve this record.".into());
        }
        Ok(session)
    }

    pub fn latest(&self) -> AppResult<Option<Session>> {
        let body: Option<String> = self
            .connection
            .query_row(
                "SELECT body FROM sessions ORDER BY sequence DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        body.map(|b| Self::decode(&b)).transpose()
    }

    pub fn active(&self) -> AppResult<Option<Session>> {
        let body: Option<String> = self
            .connection
            .query_row(
                "SELECT body FROM sessions WHERE finished=0 ORDER BY sequence DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        body.map(|b| Self::decode(&b)).transpose()
    }

    fn decode(body: &str) -> AppResult<Session> {
        if body.len() > 4 * 1024 * 1024 {
            return Err("Journal record exceeds the size limit; no changes were attempted.".into());
        }
        let session: Session =
            serde_json::from_str(body).map_err(|e| format!("Unreadable recovery journal: {e}"))?;
        if session.schema != SCHEMA_VERSION {
            return Err(
                "Unsupported journal version. Preserve it for recovery with the matching version."
                    .into(),
            );
        }
        integrity::validate_record(&session)?;
        Ok(session)
    }

    pub fn request_stop(&self, id: &str) -> AppResult<()> {
        self.connection
            .execute("UPDATE sessions SET stop=1 WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stop_requested(&self, id: &str) -> AppResult<bool> {
        self.connection
            .query_row("SELECT stop FROM sessions WHERE id=?1", [id], |r| {
                r.get::<_, i32>(0)
            })
            .map(|v| v != 0)
            .map_err(|e| e.to_string())
    }

    pub fn settings(&self) -> AppResult<Settings> {
        let body: Option<String> = self
            .connection
            .query_row("SELECT body FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        body.map(|s| serde_json::from_str(&s).map_err(|e| format!("Invalid settings: {e}")))
            .unwrap_or_else(|| Ok(Settings::default()))
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        let body = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        self.connection.execute("INSERT INTO settings(id,body) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET body=excluded.body", [body]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Journal for Database {
    fn save(&mut self, s: &Session) -> AppResult<()> {
        let body = integrity::encode(s)?;
        let t = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let previous: String = t
            .query_row("SELECT body FROM sessions WHERE id=?1", [&s.id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        let previous = Self::decode(&previous)?;
        integrity::validate_update(&previous, s)?;
        let updated = t
            .execute(
                "UPDATE sessions SET body=?1, finished=?2 WHERE id=?3",
                params![body, s.stage.finished() as i32, s.id],
            )
            .map_err(|e| e.to_string())?;
        if updated != 1 {
            return Err("Recovery record disappeared; refusing further changes.".into());
        }
        t.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(id: &str) -> Session {
        Session::new(
            id.into(),
            Plan {
                game_path: r"C:\Game\game.exe".into(),
                game: Some(Identity {
                    pid: 99,
                    created: 100,
                    path: r"C:\Game\game.exe".into(),
                    session_id: 1,
                    provenance: fixture_provenance(),
                }),
                actions: vec![],
                protected_paths: vec![],
                options: Options::default(),
                consent: true,
                force_consent: false,
                experimental_consent: true,
            },
        )
    }
    #[test]
    fn only_one_unfinished_session() {
        let mut db = Database::open(Path::new(":memory:")).unwrap();
        let mut first = session("one");
        db.create(&first).unwrap();
        assert!(db.create(&session("two")).is_err());
        first.stage = Stage::Restored;
        db.save(&first).unwrap();
        db.create(&session("two")).unwrap();
        assert_eq!(db.active().unwrap().unwrap().id, "two");
    }
    #[test]
    fn stop_survives_body_updates() {
        let mut db = Database::open(Path::new(":memory:")).unwrap();
        let s = session("a");
        db.create(&s).unwrap();
        db.request_stop("a").unwrap();
        db.save(&s).unwrap();
        assert!(db.stop_requested("a").unwrap());
    }
    #[test]
    fn corrupt_record_fails_closed() {
        let mut db = Database::open(Path::new(":memory:")).unwrap();
        db.create(&session("a")).unwrap();
        db.connection
            .execute("UPDATE sessions SET body='broken'", [])
            .unwrap();
        assert!(db.active().is_err());
    }
    #[test]
    fn settings_round_trip() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let s = Settings {
            game_path: r"C:\게임\test.exe".into(),
            ..Settings::default()
        };
        db.save_settings(&s).unwrap();
        assert_eq!(db.settings().unwrap().game_path, s.game_path);
    }
    #[test]
    fn absent_row_is_not_success() {
        let mut db = Database::open(Path::new(":memory:")).unwrap();
        assert!(db.save(&session("missing")).is_err());
    }
    #[test]
    fn unsupported_schema_is_not_replayed() {
        let mut db = Database::open(Path::new(":memory:")).unwrap();
        let mut s = session("a");
        s.schema = 999;
        assert!(db.create(&s).is_err());
    }
}
