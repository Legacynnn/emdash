//! `view-state/` port from Electron (ADR-0018). Opaque-value
//! key/value store over the `kv` table — UI layout state the
//! renderer wants to keep across launches.
//!
//! Values are stored as JSON-encoded text. The bindings expose
//! `serde_json::Value` end-to-end so callers can stash arbitrary
//! shapes without the host doing any schema work.

use std::collections::BTreeMap;
use std::sync::Arc;

use rusqlite::params;
use thiserror::Error;

use crate::db::{Db, DbError};

#[derive(Debug, Error)]
pub enum ViewStateError {
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("malformed JSON in kv row: {0}")]
    MalformedJson(String),
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn save(db: &Arc<Db>, key: &str, value: &serde_json::Value) -> Result<(), ViewStateError> {
    let encoded = serde_json::to_string(value).expect("serde_json::Value always serializes");
    let conn = db.write()?;
    conn.execute(
        "INSERT INTO kv (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, encoded, now_ms()],
    )?;
    Ok(())
}

pub fn get(db: &Arc<Db>, key: &str) -> Result<Option<serde_json::Value>, ViewStateError> {
    use rusqlite::OptionalExtension;
    let conn = db.read()?;
    let raw: Option<String> = conn
        .query_row("SELECT value FROM kv WHERE key = ?", params![key], |r| {
            r.get(0)
        })
        .optional()?;
    raw.map(|s| serde_json::from_str(&s).map_err(|e| ViewStateError::MalformedJson(e.to_string())))
        .transpose()
}

pub fn get_all(db: &Arc<Db>) -> Result<BTreeMap<String, serde_json::Value>, ViewStateError> {
    let conn = db.read()?;
    let mut stmt = conn.prepare("SELECT key, value FROM kv ORDER BY key")?;
    let rows = stmt
        .query_map([], |row| {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            Ok((key, value))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = BTreeMap::new();
    for (k, v) in rows {
        let parsed: serde_json::Value =
            serde_json::from_str(&v).map_err(|e| ViewStateError::MalformedJson(e.to_string()))?;
        out.insert(k, parsed);
    }
    Ok(out)
}

pub fn delete(db: &Arc<Db>, key: &str) -> Result<(), ViewStateError> {
    let conn = db.write()?;
    conn.execute("DELETE FROM kv WHERE key = ?", params![key])?;
    Ok(())
}

pub fn reset(db: &Arc<Db>) -> Result<(), ViewStateError> {
    let conn = db.write()?;
    conn.execute("DELETE FROM kv", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_temp_db() -> (TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn round_trips_arbitrary_json() {
        let (_d, db) = open_temp_db();
        let v = serde_json::json!({ "tabs": ["a", "b"], "size": 42 });
        save(&db, "layout", &v).unwrap();
        assert_eq!(get(&db, "layout").unwrap(), Some(v));
    }

    #[test]
    fn upsert_does_not_duplicate() {
        let (_d, db) = open_temp_db();
        save(&db, "k", &serde_json::json!("first")).unwrap();
        save(&db, "k", &serde_json::json!("second")).unwrap();
        let conn = db.read().unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM kv WHERE key = ?", params!["k"], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn delete_and_reset() {
        let (_d, db) = open_temp_db();
        save(&db, "a", &serde_json::json!(1)).unwrap();
        save(&db, "b", &serde_json::json!(2)).unwrap();
        delete(&db, "a").unwrap();
        assert!(get(&db, "a").unwrap().is_none());
        assert!(get(&db, "b").unwrap().is_some());
        reset(&db).unwrap();
        assert!(get(&db, "b").unwrap().is_none());
    }

    #[test]
    fn get_all_returns_sorted_keys() {
        let (_d, db) = open_temp_db();
        save(&db, "b", &serde_json::json!(2)).unwrap();
        save(&db, "a", &serde_json::json!(1)).unwrap();
        let all = get_all(&db).unwrap();
        let keys: Vec<_> = all.keys().cloned().collect();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }
}
