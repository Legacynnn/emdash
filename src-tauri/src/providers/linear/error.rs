//! `LinearError` — typed error surface.

use thiserror::Error;

use crate::secrets::SecretsError;

#[derive(Debug, Error)]
pub enum LinearError {
    #[error("not signed in (no token stored)")]
    NotSignedIn,
    #[error("authentication failed (likely expired or revoked token)")]
    Unauthorized,
    #[error("rate limited; reset epoch: {0}")]
    RateLimited(i64),
    #[error("graphql error: {0}")]
    Graphql(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("malformed response from Linear: {0}")]
    Malformed(String),
    #[error("secrets storage error: {0}")]
    Secrets(#[from] SecretsError),
}
