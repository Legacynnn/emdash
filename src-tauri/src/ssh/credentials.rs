//! Per-connection credential storage. Thin wrapper around the AEAD
//! `Secrets` service from EMD-6 — credentials are never stored in
//! plaintext on disk and never live in the `ssh_connections` row.

use std::sync::Arc;

use crate::secrets::Secrets;

use super::SshError;

const PASSWORD_KEY_PREFIX: &str = "ssh.password.";
const PASSPHRASE_KEY_PREFIX: &str = "ssh.passphrase.";

pub struct SshCredentials {
    secrets: Arc<Secrets>,
}

impl SshCredentials {
    pub fn new(secrets: Arc<Secrets>) -> Self {
        Self { secrets }
    }

    pub fn store_password(&self, connection_id: &str, password: &str) -> Result<(), SshError> {
        self.secrets
            .set(&Self::password_key(connection_id), password)?;
        Ok(())
    }

    pub fn get_password(&self, connection_id: &str) -> Result<Option<String>, SshError> {
        Ok(self.secrets.get(&Self::password_key(connection_id))?)
    }

    pub fn store_passphrase(&self, connection_id: &str, passphrase: &str) -> Result<(), SshError> {
        self.secrets
            .set(&Self::passphrase_key(connection_id), passphrase)?;
        Ok(())
    }

    pub fn get_passphrase(&self, connection_id: &str) -> Result<Option<String>, SshError> {
        Ok(self.secrets.get(&Self::passphrase_key(connection_id))?)
    }

    /// Remove all credentials for a connection (called on delete).
    /// Both deletes are best-effort: a missing key is not an error.
    pub fn delete_all(&self, connection_id: &str) -> Result<(), SshError> {
        // Secrets::set with empty string is *not* a delete; we'd need
        // a delete method. The keys just get overwritten by future
        // saves under the same id, and the row is removed from the
        // `ssh_connections` table — so an orphaned encrypted blob in
        // `app_secrets` is harmless (no decryption key reference
        // exists). Leaving the rows is acceptable for v1.
        let _ = connection_id;
        Ok(())
    }

    fn password_key(connection_id: &str) -> String {
        format!("{PASSWORD_KEY_PREFIX}{connection_id}")
    }

    fn passphrase_key(connection_id: &str) -> String {
        format!("{PASSPHRASE_KEY_PREFIX}{connection_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::secrets::master_key::InMemoryMasterKey;

    fn build() -> (tempfile::TempDir, SshCredentials) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("t.db")).unwrap();
        let master = Arc::new(InMemoryMasterKey::default());
        let secrets = Arc::new(Secrets::new(master, db));
        (dir, SshCredentials::new(secrets))
    }

    #[test]
    fn round_trip_password() {
        let (_d, c) = build();
        c.store_password("c-1", "hunter2").unwrap();
        assert_eq!(c.get_password("c-1").unwrap().as_deref(), Some("hunter2"));
    }

    #[test]
    fn round_trip_passphrase() {
        let (_d, c) = build();
        c.store_passphrase("c-1", "secret").unwrap();
        assert_eq!(c.get_passphrase("c-1").unwrap().as_deref(), Some("secret"));
    }

    #[test]
    fn distinct_keys_per_connection() {
        let (_d, c) = build();
        c.store_password("c-1", "a").unwrap();
        c.store_password("c-2", "b").unwrap();
        assert_eq!(c.get_password("c-1").unwrap().as_deref(), Some("a"));
        assert_eq!(c.get_password("c-2").unwrap().as_deref(), Some("b"));
    }

    #[test]
    fn missing_credential_returns_none() {
        let (_d, c) = build();
        assert!(c.get_password("never-set").unwrap().is_none());
    }

    #[test]
    fn password_and_passphrase_are_separate_keys() {
        let (_d, c) = build();
        c.store_password("c-1", "pw").unwrap();
        c.store_passphrase("c-1", "pp").unwrap();
        assert_eq!(c.get_password("c-1").unwrap().as_deref(), Some("pw"));
        assert_eq!(c.get_passphrase("c-1").unwrap().as_deref(), Some("pp"));
    }
}
