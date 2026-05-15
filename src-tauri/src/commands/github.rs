//! Tauri glue for the GitHub provider (EMD-13 / ADR-0022). All
//! token reads/writes go through `Arc<Secrets>` via the auth +
//! identity helpers; no plaintext touches the renderer.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::db::Db;
use crate::providers::github::{
    self, client, identity, DeviceFlow, DeviceFlowStart, GithubError, IdentityRecord,
    PullRequestSummary, RepoSummary, TOKEN_SECRET_KEY,
};
use crate::secrets::Secrets;
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct GithubCommandError {
    pub code: GithubErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GithubErrorCode {
    NotSignedIn,
    Unauthorized,
    Storage,
    Oauth,
    Network,
    RateLimited,
    GhCliUnavailable,
    Malformed,
}

fn map_github_error(e: GithubError) -> GithubCommandError {
    let code = match &e {
        GithubError::NotSignedIn => GithubErrorCode::NotSignedIn,
        GithubError::Unauthorized => GithubErrorCode::Unauthorized,
        GithubError::RateLimited(_) => GithubErrorCode::RateLimited,
        GithubError::Malformed(_) => GithubErrorCode::Malformed,
        GithubError::Octocrab(err) => match err {
            octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == 401 => {
                GithubErrorCode::Unauthorized
            }
            octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == 403 => {
                GithubErrorCode::RateLimited
            }
            _ => GithubErrorCode::Network,
        },
    };
    GithubCommandError {
        code,
        message: e.to_string(),
    }
}

fn map_oauth_error(e: github::DeviceFlowError) -> GithubCommandError {
    let code = match &e {
        github::DeviceFlowError::Oauth(_) | github::DeviceFlowError::DenyOrExpired => {
            GithubErrorCode::Oauth
        }
        github::DeviceFlowError::Network(_) => GithubErrorCode::Network,
        github::DeviceFlowError::GhCliUnavailable => GithubErrorCode::GhCliUnavailable,
        github::DeviceFlowError::Malformed => GithubErrorCode::Malformed,
        github::DeviceFlowError::Secrets(_) => GithubErrorCode::Storage,
    };
    GithubCommandError {
        code,
        message: e.to_string(),
    }
}

async fn refresh_identity(
    secrets: &Arc<Secrets>,
    db: &Arc<Db>,
    ui_sync: &Arc<UiSyncManager>,
) -> Result<IdentityRecord, GithubCommandError> {
    let token = secrets
        .get(TOKEN_SECRET_KEY)
        .map_err(|e| GithubCommandError {
            code: GithubErrorCode::Storage,
            message: e.to_string(),
        })?
        .unwrap_or_default();
    if token.is_empty() {
        return Err(GithubCommandError {
            code: GithubErrorCode::NotSignedIn,
            message: "no token stored".into(),
        });
    }
    let client = client::client_for_token(&token).map_err(map_github_error)?;
    let viewer = client::fetch_viewer(&client)
        .await
        .map_err(map_github_error)?;
    let record = IdentityRecord {
        login: viewer.login,
        id: viewer.id,
        name: viewer.name,
        email: viewer.email,
        avatar_url: viewer.avatar_url,
    };
    identity::set_identity(db, &record).map_err(|e| GithubCommandError {
        code: GithubErrorCode::Storage,
        message: e.to_string(),
    })?;
    ui_sync.broadcast(UiMutationEvent::GithubIdentityChanged);
    Ok(record)
}

#[tauri::command]
#[specta::specta]
pub async fn github_sign_in_device_flow_start() -> Result<DeviceFlowStart, GithubCommandError> {
    github::device_flow_start(github::auth::DEFAULT_SCOPES)
        .await
        .map_err(map_oauth_error)
}

#[tauri::command]
#[specta::specta]
pub async fn github_sign_in_device_flow_poll(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    flow: DeviceFlow,
) -> Result<IdentityRecord, GithubCommandError> {
    github::device_flow_poll(secrets.inner().clone(), &flow)
        .await
        .map_err(map_oauth_error)?;
    refresh_identity(secrets.inner(), db.inner(), ui_sync.inner()).await
}

#[tauri::command]
#[specta::specta]
pub async fn github_sign_in_via_gh_cli(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
) -> Result<IdentityRecord, GithubCommandError> {
    let token = github::gh_cli_token().map_err(map_oauth_error)?;
    github::sign_in_with_token(secrets.inner(), &token).map_err(map_oauth_error)?;
    refresh_identity(secrets.inner(), db.inner(), ui_sync.inner()).await
}

#[tauri::command]
#[specta::specta]
pub fn github_sign_out(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
) -> Result<(), GithubCommandError> {
    github::sign_out(secrets.inner()).map_err(map_oauth_error)?;
    identity::clear_identity(&db).map_err(|e| GithubCommandError {
        code: GithubErrorCode::Storage,
        message: e.to_string(),
    })?;
    ui_sync.broadcast(UiMutationEvent::GithubIdentityChanged);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn github_me(db: State<'_, Arc<Db>>) -> Result<Option<IdentityRecord>, GithubCommandError> {
    identity::get_identity(&db).map_err(|e| GithubCommandError {
        code: GithubErrorCode::Storage,
        message: e.to_string(),
    })
}

async fn authed_client(secrets: &Arc<Secrets>) -> Result<octocrab::Octocrab, GithubCommandError> {
    let token = secrets
        .get(TOKEN_SECRET_KEY)
        .map_err(|e| GithubCommandError {
            code: GithubErrorCode::Storage,
            message: e.to_string(),
        })?
        .unwrap_or_default();
    if token.is_empty() {
        return Err(GithubCommandError {
            code: GithubErrorCode::NotSignedIn,
            message: "no token stored".into(),
        });
    }
    client::client_for_token(&token).map_err(map_github_error)
}

#[tauri::command]
#[specta::specta]
pub async fn github_list_repos(
    secrets: State<'_, Arc<Secrets>>,
) -> Result<Vec<RepoSummary>, GithubCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client::list_repos(&client).await.map_err(map_github_error)
}

#[tauri::command]
#[specta::specta]
pub async fn github_list_pulls(
    secrets: State<'_, Arc<Secrets>>,
    owner: String,
    repo: String,
) -> Result<Vec<PullRequestSummary>, GithubCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client::list_pulls(&client, &owner, &repo)
        .await
        .map_err(map_github_error)
}

#[tauri::command]
#[specta::specta]
pub async fn github_get_pull(
    secrets: State<'_, Arc<Secrets>>,
    owner: String,
    repo: String,
    number: u32,
) -> Result<PullRequestSummary, GithubCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client::get_pull(&client, &owner, &repo, number as u64)
        .await
        .map_err(map_github_error)
}

#[tauri::command]
#[specta::specta]
pub async fn github_get_pull_diff(
    secrets: State<'_, Arc<Secrets>>,
    owner: String,
    repo: String,
    number: u32,
) -> Result<String, GithubCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client::get_pull_diff(&client, &owner, &repo, number as u64)
        .await
        .map_err(map_github_error)
}
