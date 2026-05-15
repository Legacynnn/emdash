//! Persisted GitHub identity (login / id / name / email / avatar).
//!
//! Stored in `app_settings` under one key per field so callers don't
//! need to manage a separate schema migration. The token itself
//! never appears here — that lives in AEAD `app_secrets`.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::db::{Db, DbError};

const KEY_LOGIN: &str = "github.identity.login";
const KEY_ID: &str = "github.identity.id";
const KEY_NAME: &str = "github.identity.name";
const KEY_EMAIL: &str = "github.identity.email";
const KEY_AVATAR: &str = "github.identity.avatar_url";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct IdentityRecord {
    pub login: String,
    pub id: String,
    pub name: Option<String>,
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

pub fn get_identity(db: &Arc<Db>) -> Result<Option<IdentityRecord>, DbError> {
    let login = match read_key(db, KEY_LOGIN)? {
        Some(l) if !l.is_empty() => l,
        _ => return Ok(None),
    };
    let id = read_key(db, KEY_ID)?.unwrap_or_default();
    let name = read_key(db, KEY_NAME)?.filter(|v| !v.is_empty());
    let email = read_key(db, KEY_EMAIL)?.filter(|v| !v.is_empty());
    let avatar_url = read_key(db, KEY_AVATAR)?.filter(|v| !v.is_empty());
    Ok(Some(IdentityRecord {
        login,
        id,
        name,
        email,
        avatar_url,
    }))
}

pub fn set_identity(db: &Arc<Db>, identity: &IdentityRecord) -> Result<(), DbError> {
    write_key(db, KEY_LOGIN, &identity.login)?;
    write_key(db, KEY_ID, &identity.id)?;
    write_key(db, KEY_NAME, identity.name.as_deref().unwrap_or(""))?;
    write_key(db, KEY_EMAIL, identity.email.as_deref().unwrap_or(""))?;
    write_key(db, KEY_AVATAR, identity.avatar_url.as_deref().unwrap_or(""))?;
    Ok(())
}

pub fn clear_identity(db: &Arc<Db>) -> Result<(), DbError> {
    delete_keys(db, &[KEY_LOGIN, KEY_ID, KEY_NAME, KEY_EMAIL, KEY_AVATAR])
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
        let rec = IdentityRecord {
            login: "octocat".into(),
            id: "1".into(),
            name: Some("Octo Cat".into()),
            email: Some("octo@example.com".into()),
            avatar_url: Some("https://example.com/a.png".into()),
        };
        set_identity(&db, &rec).unwrap();
        assert_eq!(get_identity(&db).unwrap(), Some(rec));
    }

    #[test]
    fn clear_removes_all_keys() {
        let (_d, db) = open_temp_db();
        set_identity(
            &db,
            &IdentityRecord {
                login: "octocat".into(),
                id: "1".into(),
                name: None,
                email: None,
                avatar_url: None,
            },
        )
        .unwrap();
        clear_identity(&db).unwrap();
        assert_eq!(get_identity(&db).unwrap(), None);
    }
}
