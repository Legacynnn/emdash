//! User-facing telemetry toggle stored in `app_settings`.
//!
//! Defaults to `false` (off). The toggle composes with the
//! compile-time gate from `TelemetryConfig::is_compiled_in()` — both
//! must be `true` for the runtime to send anything.

use std::sync::Arc;

use rusqlite::{params, OptionalExtension};

use crate::db::{Db, DbError};

/// The `app_settings.key` value under which the toggle is stored.
/// Exposed for documentation and tests; the renderer reads it through
/// the dedicated Tauri commands rather than poking at the table.
pub const SETTINGS_KEY: &str = "telemetry.enabled";

pub fn get_enabled(db: &Arc<Db>) -> Result<bool, DbError> {
    let conn = db.read()?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?",
            params![SETTINGS_KEY],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value.as_deref() == Some("true"))
}

pub fn set_enabled(db: &Arc<Db>, enabled: bool) -> Result<(), DbError> {
    let conn = db.write()?;
    let value = if enabled { "true" } else { "false" };
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        params![SETTINGS_KEY, value],
    )?;
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
    fn default_is_off() {
        let (_dir, db) = open_temp_db();
        assert!(!get_enabled(&db).unwrap());
    }

    #[test]
    fn toggle_round_trips() {
        let (_dir, db) = open_temp_db();
        set_enabled(&db, true).unwrap();
        assert!(get_enabled(&db).unwrap());
        set_enabled(&db, false).unwrap();
        assert!(!get_enabled(&db).unwrap());
    }

    #[test]
    fn upsert_does_not_duplicate() {
        let (_dir, db) = open_temp_db();
        for _ in 0..5 {
            set_enabled(&db, true).unwrap();
        }
        let conn = db.read().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM app_settings WHERE key = ?",
                params![SETTINGS_KEY],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
