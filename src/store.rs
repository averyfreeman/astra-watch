//! Transactional state plus immutable, hash-linked audit events.
use crate::{digest, files, now_ms};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Store {
    pub root: PathBuf,
    pub db: Connection,
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let db_path = files::inside(root, ".astra-watch/runtime/state.sqlite")?;
        fs::create_dir_all(db_path.parent().unwrap())?;
        let db = Connection::open(db_path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version == 0 || version == 1,
            "unsupported state database version"
        );
        db.execute_batch("CREATE TABLE IF NOT EXISTS events (
          seq INTEGER PRIMARY KEY, at_ms TEXT NOT NULL, kind TEXT NOT NULL,
          body TEXT NOT NULL, previous_hash TEXT NOT NULL, hash TEXT NOT NULL UNIQUE);
          CREATE TABLE IF NOT EXISTS artifacts (id TEXT PRIMARY KEY, kind TEXT NOT NULL, path TEXT NOT NULL, digest TEXT NOT NULL, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, version INTEGER NOT NULL, status TEXT NOT NULL, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS evidence (id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), criterion_id TEXT NOT NULL, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, hash TEXT NOT NULL, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS telemetry (id INTEGER PRIMARY KEY, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS cadences (id TEXT PRIMARY KEY, body TEXT NOT NULL, runs INTEGER NOT NULL DEFAULT 0);
          CREATE TRIGGER IF NOT EXISTS events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'audit events are immutable'); END;
          CREATE TRIGGER IF NOT EXISTS events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'audit events are immutable'); END;
          CREATE TRIGGER IF NOT EXISTS artifacts_no_update BEFORE UPDATE ON artifacts BEGIN SELECT RAISE(ABORT,'artifacts are immutable'); END;
          CREATE TRIGGER IF NOT EXISTS artifacts_no_delete BEFORE DELETE ON artifacts BEGIN SELECT RAISE(ABORT,'artifacts are immutable'); END;
          CREATE TRIGGER IF NOT EXISTS evidence_no_update BEFORE UPDATE ON evidence BEGIN SELECT RAISE(ABORT,'evidence is immutable'); END;
          CREATE TRIGGER IF NOT EXISTS evidence_no_delete BEFORE DELETE ON evidence BEGIN SELECT RAISE(ABORT,'evidence is immutable'); END;
          PRAGMA user_version=1;")?;
        Ok(Self {
            root: root.into(),
            db,
        })
    }

    pub fn write<T>(&mut self, f: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let result = f(&tx)?;
        crate::operation::check()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn artifact(&mut self, kind: &str, payload: &Value) -> Result<Value> {
        crate::config::identifier(kind)?;
        let body = serde_json::to_string_pretty(payload)?;
        let hash = digest(&body);
        let path = format!(".astra-watch/records/{kind}-{hash}.json");
        crate::operation::check()?;
        files::immutable(&self.root, &path, body.as_bytes())?;
        self.write(|tx| {
            tx.execute(
                "INSERT OR IGNORE INTO artifacts(id,kind,path,digest,body) VALUES (?1,?2,?3,?4,?5)",
                params![hash, kind, path, hash, body],
            )?;
            Ok(())
        })?;
        Ok(json!({"id":hash,"kind":kind,"path":path,"digest":hash}))
    }

    pub fn read_artifact(&self, id: &str) -> Result<Value> {
        let row: (String, String, String) = self
            .db
            .query_row(
                "SELECT path,digest,body FROM artifacts WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .context("artifact not found")?;
        let bytes = fs::read(files::inside(&self.root, &row.0)?)?;
        ensure!(
            digest(&bytes) == row.1 && digest(&row.2) == row.1,
            "artifact integrity failure"
        );
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn events(&self, after: i64, limit: usize) -> Result<Vec<Value>> {
        let mut stmt = self.db.prepare("SELECT seq,at_ms,kind,body,previous_hash,hash FROM events WHERE seq>?1 ORDER BY seq LIMIT ?2")?;
        let result = stmt.query_map(params![after, limit.clamp(1, 1000)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        result.map(|row| { let (seq,at,kind,body,prev,hash) = row?; Ok(json!({"seq":seq,"at_ms":at,"kind":kind,"body":serde_json::from_str::<Value>(&body)?,"previous_hash":prev,"hash":hash})) }).collect()
    }

    pub fn audit(&self) -> Result<()> {
        let mut last = String::new();
        let mut stmt = self
            .db
            .prepare("SELECT at_ms,kind,body,previous_hash,hash FROM events ORDER BY seq")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (at, kind, body, prev, hash) = row?;
            ensure!(
                last == prev && digest(format!("{at}\n{kind}\n{body}\n{prev}")) == hash,
                "audit event chain integrity failure"
            );
            last = hash;
        }
        Ok(())
    }
}

pub fn event(tx: &Transaction<'_>, kind: &str, body: &Value) -> Result<()> {
    let prev: String = tx
        .query_row(
            "SELECT hash FROM events ORDER BY seq DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default();
    let at = now_ms().to_string();
    let body = serde_json::to_string(body)?;
    let hash = digest(format!("{at}\n{kind}\n{body}\n{prev}"));
    tx.execute(
        "INSERT INTO events(at_ms,kind,body,previous_hash,hash) VALUES (?1,?2,?3,?4,?5)",
        params![at, kind, body, prev, hash],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audit_append_only_and_reopen_survive_transactions() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path()).unwrap();
        s.write(|tx| event(tx, "test", &json!({"observed":true})))
            .unwrap();
        assert!(s.db.execute("DELETE FROM events", []).is_err());
        s.audit().unwrap();
        drop(s);
        let s = Store::open(dir.path()).unwrap();
        s.audit().unwrap();
        assert_eq!(s.events(0, 10).unwrap().len(), 1);
    }
    #[test]
    fn artifact_tampering_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Store::open(dir.path()).unwrap();
        let a = s
            .artifact("mission", &json!({"outcome":"verified"}))
            .unwrap();
        s.read_artifact(a["id"].as_str().unwrap()).unwrap();
        fs::write(dir.path().join(a["path"].as_str().unwrap()), "{}").unwrap();
        assert!(s.read_artifact(a["id"].as_str().unwrap()).is_err());
    }
}
