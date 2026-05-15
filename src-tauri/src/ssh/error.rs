//! Error envelope for the SSH module.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshError {
    #[error("not found: ssh connection `{0}`")]
    NotFound(String),
    #[error("ssh connection `{0}` is in use by: {1}")]
    InUse(String, String),
    #[error("ssh client error: {0}")]
    Client(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("db error: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("secrets error: {0}")]
    Secrets(#[from] crate::secrets::SecretsError),
    #[error("not supported on this platform: {0}")]
    Unsupported(&'static str),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<openssh::Error> for SshError {
    fn from(e: openssh::Error) -> Self {
        SshError::Client(e.to_string())
    }
}
