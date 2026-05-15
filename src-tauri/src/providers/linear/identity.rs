//! Persisted Linear identity (id / name / email / avatar).
//!
//! Stored in `app_settings` under one key per field — same pattern
//! as the GitHub provider (ADR-0022). The token never appears here.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::db::{Db, DbError};

const KEY_ID: &str = "linear.identity.id";
const KEY_NAME: &str = "linear.identity.name";
const KEY_DISPLAY_NAME: &str = "linear.identity.display_name";
const KEY_EMAIL: &str = "linear.identity.email";
const KEY_AVATAR: &str = "linear.identity.avatar_url";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearIdentityRecord {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

fn read_key(db: &Arc<Db>, key: &str) -> Result<Option<String>, DbError> {
    let conn = db.read()?;
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?",
            params![key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v)
}

fn write_key(db: &Arc<Db>, key: &str, value: &str) -> Result<(), DbError> {
    let conn = db.write()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        params![key, value],
    )?;
    Ok(())
}

fn delete_keys(db: &Arc<Db>, keys: &[&str]) -> Result<(), DbError> {
    let conn = db.write()?;
    for k in keys {
        conn.execute("DELETE FROM app_settings WHERE key = ?", params![k])?;
    }
    Ok(())
}

pub fn get_identity(db: &Arc<Db>) -> Result<Option<LinearIdentityRecord>, DbError> {
    let id = match read_key(db, KEY_ID)? {
        Some(i) if !i.is_empty() => i,
        _ => return Ok(None),
    };
    let name = read_key(db, KEY_NAME)?.unwrap_or_default();
    let display_name = read_key(db, KEY_DISPLAY_NAME)?.filter(|v| !v.is_empty());
    let email = read_key(db, KEY_EMAIL)?.filter(|v| !v.is_empty());
    let avatar_url = read_key(db, KEY_AVATAR)?.filter(|v| !v.is_empty());
    Ok(Some(LinearIdentityRecord {
        id,
        name,
        display_name,
        email,
        avatar_url,
    }))
}

pub fn set_identity(db: &Arc<Db>, identity: &LinearIdentityRecord) -> Result<(), DbError> {
    write_key(db, KEY_ID, &identity.id)?;
    write_key(db, KEY_NAME, &identity.name)?;
    write_key(
        db,
        KEY_DISPLAY_NAME,
        identity.display_name.as_deref().unwrap_or(""),
    )?;
    write_key(db, KEY_EMAIL, identity.email.as_deref().unwrap_or(""))?;
    write_key(db, KEY_AVATAR, identity.avatar_url.as_deref().unwrap_or(""))?;
    Ok(())
}

pub fn clear_identity(db: &Arc<Db>) -> Result<(), DbError> {
    delete_keys(
        db,
        &[KEY_ID, KEY_NAME, KEY_DISPLAY_NAME, KEY_EMAIL, KEY_AVATAR],
    )
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
    fn round_trips_identity() {
        let (_d, db) = open_temp_db();
        let rec = LinearIdentityRecord {
            id: "u_abc".into(),
            name: "Alice".into(),
            display_name: Some("alice".into()),
            email: Some("alice@example.com".into()),
            avatar_url: None,
        };
        set_identity(&db, &rec).unwrap();
        assert_eq!(get_identity(&db).unwrap(), Some(rec));
    }

    #[test]
    fn clear_removes_all_keys() {
        let (_d, db) = open_temp_db();
        set_identity(
            &db,
            &LinearIdentityRecord {
                id: "u_abc".into(),
                name: "Alice".into(),
                display_name: None,
                email: None,
                avatar_url: None,
            },
        )
        .unwrap();
        clear_identity(&db).unwrap();
        assert_eq!(get_identity(&db).unwrap(), None);
    }
}
