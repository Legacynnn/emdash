//! Auth surface — device-flow + `gh` CLI fallback.
//!
//! The device flow is two halves: `device_flow_start` POSTs to
//! GitHub's `/login/device/code` endpoint and returns the user code +
//! verification URL + polling cadence. `device_flow_poll` POSTs to
//! `/login/oauth/access_token` until GitHub returns a token, then
//! persists it via `Secrets`.
//!
//! oauth2 0.x's typed builders make the two-call split awkward, so
//! the calls are hand-rolled. Schemas are documented at
//! <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow>.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::secrets::{Secrets, SecretsError};

use super::TOKEN_SECRET_KEY;

/// emdash's GitHub OAuth app (the one the Electron build uses). The
/// client ID is public; the device flow doesn't need a client secret.
/// **If this changes, rotate the constant and document it in the next
/// ADR.**
const GH_CLIENT_ID: &str = "Ov23liNqGq9F4O7B2DwY";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const POLL_TIMEOUT_SECS: u64 = 600; // GitHub's device code lifetime is 15 min; we cap polling at 10.

/// Scopes per the EMD-13 spec.
pub const DEFAULT_SCOPES: &[OauthScope] = &[
    OauthScope::Repo,
    OauthScope::ReadUser,
    OauthScope::UserEmail,
    OauthScope::ReadOrg,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OauthScope {
    Repo,
    ReadUser,
    UserEmail,
    ReadOrg,
}

impl OauthScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::ReadUser => "read:user",
            Self::UserEmail => "user:email",
            Self::ReadOrg => "read:org",
        }
    }
}

/// What the renderer needs to display when the flow starts.
/// `u32` for the timing fields because specta forbids BigInt types
/// over IPC. GitHub's intervals + expiries fit comfortably in u32.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub polling_interval_seconds: u32,
    pub expires_in_seconds: u32,
}

/// Opaque handle for the renderer to pass back to
/// `device_flow_poll`. We expose just the bits the OAuth server
/// needs and keep the rest internal.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct DeviceFlow {
    pub device_code: String,
    pub polling_interval_seconds: u32,
}

#[derive(Debug, Error)]
pub enum DeviceFlowError {
    #[error("oauth request failed: {0}")]
    Oauth(String),
    #[error("user declined or device code expired")]
    DenyOrExpired,
    #[error("gh CLI not available or returned no token")]
    GhCliUnavailable,
    #[error("malformed token response")]
    Malformed,
    #[error("secrets storage error: {0}")]
    Secrets(#[from] SecretsError),
    #[error("network error: {0}")]
    Network(String),
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct AccessTokenSuccess {
    access_token: String,
}

#[derive(Deserialize)]
struct AccessTokenError {
    error: String,
}

fn http_client() -> Result<reqwest::Client, DeviceFlowError> {
    reqwest::Client::builder()
        .user_agent(format!("emdash-dev/{}", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))
}

/// Kick off the device flow. Returns immediately with the code + URL
/// the user needs to enter.
pub async fn device_flow_start(scopes: &[OauthScope]) -> Result<DeviceFlowStart, DeviceFlowError> {
    let scope_str = scopes
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let client = http_client()?;
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", GH_CLIENT_ID), ("scope", &scope_str)])
        .send()
        .await
        .map_err(|e| DeviceFlowError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| DeviceFlowError::Oauth(e.to_string()))?;
    let parsed: DeviceCodeResponse = resp.json().await.map_err(|_| DeviceFlowError::Malformed)?;
    Ok(DeviceFlowStart {
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        device_code: parsed.device_code,
        polling_interval_seconds: parsed.interval as u32,
        expires_in_seconds: parsed.expires_in as u32,
    })
}

/// Poll the OAuth server until the user authorizes (or until the
/// device code expires). On success, persist the token via `Secrets`.
pub async fn device_flow_poll(
    secrets: Arc<Secrets>,
    flow: &DeviceFlow,
) -> Result<(), DeviceFlowError> {
    let client = http_client()?;
    let device_code = flow.device_code.clone();
    let mut interval: u64 = flow.polling_interval_seconds.max(1) as u64;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(POLL_TIMEOUT_SECS);

    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if tokio::time::Instant::now() > deadline {
            return Err(DeviceFlowError::DenyOrExpired);
        }

        let resp = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GH_CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| DeviceFlowError::Network(e.to_string()))?;

        let body: serde_json::Value = resp.json().await.map_err(|_| DeviceFlowError::Malformed)?;

        if let Ok(success) = serde_json::from_value::<AccessTokenSuccess>(body.clone()) {
            secrets.set(TOKEN_SECRET_KEY, &success.access_token)?;
            return Ok(());
        }

        match serde_json::from_value::<AccessTokenError>(body) {
            Ok(err) => match err.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval = interval.saturating_add(5);
                    continue;
                }
                "expired_token" | "access_denied" => {
                    return Err(DeviceFlowError::DenyOrExpired);
                }
                other => return Err(DeviceFlowError::Oauth(other.to_string())),
            },
            Err(_) => return Err(DeviceFlowError::Malformed),
        }
    }
}

/// Direct sign-in by handing us a token (used by both the test path
/// and `gh_cli_token`'s downstream).
pub fn sign_in_with_token(secrets: &Arc<Secrets>, token: &str) -> Result<(), DeviceFlowError> {
    secrets.set(TOKEN_SECRET_KEY, token)?;
    Ok(())
}

/// Read a token from `gh auth token`. Errors if `gh` is not on PATH,
/// exits non-zero, or returns an empty body.
pub fn gh_cli_token() -> Result<String, DeviceFlowError> {
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|_| DeviceFlowError::GhCliUnavailable)?;
    if !output.status.success() {
        return Err(DeviceFlowError::GhCliUnavailable);
    }
    let token = String::from_utf8(output.stdout)
        .map_err(|_| DeviceFlowError::Malformed)?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(DeviceFlowError::GhCliUnavailable);
    }
    Ok(token)
}

/// Clear the stored token.
pub fn sign_out(secrets: &Arc<Secrets>) -> Result<(), DeviceFlowError> {
    secrets.set(TOKEN_SECRET_KEY, "")?;
    Ok(())
}
