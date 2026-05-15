//! `SshConnectionStore` — DB CRUD for the `ssh_connections` table.
//!
//! Credentials in `NewSshConnection` are deliberately stripped here
//! and forwarded to `SshCredentials`. The DB row never contains a
//! plaintext password, passphrase, or private key contents.

use std::sync::Arc;

use serde_json::json;
use uuid::Uuid;

use crate::db::Db;

use super::{AuthType, NewSshConnection, SshConnection, SshCredentials, SshError};

pub struct SshConnectionStore {
    db: Arc<Db>,
    credentials: Arc<SshCredentials>,
}

impl SshConnectionStore {
    pub fn new(db: Arc<Db>, credentials: Arc<SshCredentials>) -> Self {
        Self { db, credentials }
    }

    pub fn list(&self) -> Result<Vec<SshConnection>, SshError> {
        #[allow(clippy::let_and_return)]
        let rows: Vec<Result<SshConnection, SshError>> = {
            let conn = self.db.read()?;
            let mut stmt = conn.prepare_cached(
                "SELECT id, name, host, port, username, auth_type, private_key_path, \
                        use_agent, metadata, updated_at \
                 FROM ssh_connections ORDER BY name",
            )?;
            let collected = stmt
                .query_map([], row_to_connection)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            collected
        };
        rows.into_iter().collect()
    }

    pub fn get(&self, id: &str) -> Result<Option<SshConnection>, SshError> {
        #[allow(clippy::let_and_return)]
        let result: Option<Result<SshConnection, SshError>> = {
            let conn = self.db.read()?;
            let mut stmt = conn.prepare_cached(
                "SELECT id, name, host, port, username, auth_type, private_key_path, \
                        use_agent, metadata, updated_at \
                 FROM ssh_connections WHERE id = ?1",
            )?;
            let row_result =
                stmt.query_row([id], row_to_connection)
                    .map(Some)
                    .or_else(|e| match e {
                        rusqlite::Error::QueryReturnedNoRows => Ok(None),
                        other => Err(other),
                    })?;
            row_result
        };
        result.transpose()
    }

    /// Insert or update. Returns the persisted (non-secret) view.
    pub fn save(&self, payload: NewSshConnection) -> Result<SshConnection, SshError> {
        let id = payload
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        if let Some(pw) = payload.password.as_deref() {
            if !pw.is_empty() {
                self.credentials.store_password(&id, pw)?;
            }
        }
        if let Some(pp) = payload.passphrase.as_deref() {
            if !pp.is_empty() {
                self.credentials.store_passphrase(&id, pp)?;
            }
        }

        let metadata = json!({ "worktreesDir": payload.worktrees_dir }).to_string();
        let auth_type_str = match payload.auth_type {
            AuthType::Password => "password",
            AuthType::Key => "key",
            AuthType::Agent => "agent",
        };

        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.write()?;
        conn.execute(
            "INSERT INTO ssh_connections \
                 (id, name, host, port, username, auth_type, private_key_path, \
                  use_agent, metadata, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10) \
             ON CONFLICT(id) DO UPDATE SET \
                 name = excluded.name, \
                 host = excluded.host, \
                 port = excluded.port, \
                 username = excluded.username, \
                 auth_type = excluded.auth_type, \
                 private_key_path = excluded.private_key_path, \
                 use_agent = excluded.use_agent, \
                 metadata = excluded.metadata, \
                 updated_at = excluded.updated_at",
            rusqlite::params![
                id,
                payload.name,
                payload.host,
                payload.port,
                payload.username,
                auth_type_str,
                payload.private_key_path,
                payload.use_agent as i64,
                metadata,
                now,
            ],
        )?;
        drop(conn);
        self.get(&id)?.ok_or_else(|| SshError::NotFound(id))
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<(), SshError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.write()?;
        let updated = conn.execute(
            "UPDATE ssh_connections SET name = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![name, now, id],
        )?;
        if updated == 0 {
            return Err(SshError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), SshError> {
        // Refuse to delete if a project references this connection.
        #[allow(clippy::let_and_return)]
        let refs: Vec<String> = {
            let conn = self.db.read()?;
            let mut stmt =
                conn.prepare_cached("SELECT name FROM projects WHERE ssh_connection_id = ?1")?;
            let rows = stmt
                .query_map([id], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        if !refs.is_empty() {
            return Err(SshError::InUse(id.to_string(), refs.join(", ")));
        }
        self.credentials.delete_all(id)?;
        let conn = self.db.write()?;
        let removed = conn.execute(
            "DELETE FROM ssh_connections WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if removed == 0 {
            return Err(SshError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

fn row_to_connection(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<SshConnection, SshError>> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let host: String = row.get(2)?;
    let port: i64 = row.get(3)?;
    let username: String = row.get(4)?;
    let auth_type_str: String = row.get(5)?;
    let private_key_path: Option<String> = row.get(6)?;
    let use_agent: i64 = row.get(7)?;
    let metadata: Option<String> = row.get(8)?;
    let updated_at: String = row.get(9)?;

    let auth_type = match auth_type_str.as_str() {
        "password" => AuthType::Password,
        "key" => AuthType::Key,
        "agent" => AuthType::Agent,
        // Unknown auth_type: surface as a domain error.
        other => {
            return Ok(Err(SshError::Client(format!(
                "unknown auth_type `{other}` in DB row"
            ))))
        }
    };
    let worktrees_dir = metadata
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| {
            v.get("worktreesDir")
                .and_then(|w| w.as_str().map(|s| s.to_string()))
        });

    Ok(Ok(SshConnection {
        id,
        name,
        host,
        port: port as u16,
        username,
        auth_type,
        private_key_path,
        use_agent: use_agent != 0,
        worktrees_dir,
        updated_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::master_key::InMemoryMasterKey;
    use crate::secrets::Secrets;

    fn build() -> (tempfile::TempDir, SshConnectionStore) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db")).unwrap();
        let secrets = Arc::new(Secrets::new(
            Arc::new(InMemoryMasterKey::default()),
            db.clone(),
        ));
        let creds = Arc::new(SshCredentials::new(secrets));
        (dir, SshConnectionStore::new(db, creds))
    }

    fn fixture(name: &str) -> NewSshConnection {
        NewSshConnection {
            id: None,
            name: name.to_string(),
            host: "example.com".to_string(),
            port: 22,
            username: "u".to_string(),
            auth_type: AuthType::Agent,
            private_key_path: None,
            use_agent: true,
            worktrees_dir: Some("/wt".to_string()),
            password: None,
            passphrase: None,
        }
    }

    #[test]
    fn save_then_list_returns_row() {
        let (_d, s) = build();
        let saved = s.save(fixture("prod")).unwrap();
        assert_eq!(saved.name, "prod");
        let rows = s.list().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "prod");
    }

    #[test]
    fn save_with_existing_id_updates() {
        let (_d, s) = build();
        let saved = s.save(fixture("prod")).unwrap();
        let updated = s
            .save(NewSshConnection {
                id: Some(saved.id.clone()),
                name: "staging".to_string(),
                ..fixture("prod")
            })
            .unwrap();
        assert_eq!(updated.id, saved.id);
        assert_eq!(updated.name, "staging");
        // Only one row total.
        assert_eq!(s.list().unwrap().len(), 1);
    }

    #[test]
    fn save_password_writes_to_secrets_not_db() {
        let (_d, s) = build();
        let saved = s
            .save(NewSshConnection {
                password: Some("pw-1".into()),
                ..fixture("prod")
            })
            .unwrap();
        // Fetch via the credentials helper (same db, same secrets).
        let pw = s.credentials.get_password(&saved.id).unwrap();
        assert_eq!(pw.as_deref(), Some("pw-1"));
    }

    #[test]
    fn delete_clears_row() {
        let (_d, s) = build();
        let saved = s.save(fixture("prod")).unwrap();
        s.delete(&saved.id).unwrap();
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let (_d, s) = build();
        let err = s.delete("nope").unwrap_err();
        assert!(matches!(err, SshError::NotFound(_)));
    }

    #[test]
    fn rename_existing() {
        let (_d, s) = build();
        let saved = s.save(fixture("prod")).unwrap();
        s.rename(&saved.id, "renamed").unwrap();
        assert_eq!(s.get(&saved.id).unwrap().unwrap().name, "renamed");
    }

    #[test]
    fn rename_missing_returns_not_found() {
        let (_d, s) = build();
        let err = s.rename("nope", "x").unwrap_err();
        assert!(matches!(err, SshError::NotFound(_)));
    }
}
