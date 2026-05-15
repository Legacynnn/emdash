//! Tauri glue for the Linear provider (EMD-14 / ADR-0023). Mirrors
//! `commands::github`: AEAD-only token storage via `Secrets`,
//! identity split into `app_settings`, typed `{code, message}`
//! envelopes on every error.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::db::Db;
use crate::providers::linear::{
    self, LinearClient, LinearCycle, LinearError, LinearIdentityRecord, LinearIssue,
    LinearIssueComment, LinearIssueCreateInput, LinearIssueFilter, LinearIssueUpdateInput,
    LinearLabel, LinearProject, LinearTeam, LinearWorkflowState, TOKEN_SECRET_KEY,
};
use crate::secrets::Secrets;
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct LinearCommandError {
    pub code: LinearErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LinearErrorCode {
    NotSignedIn,
    Unauthorized,
    Storage,
    Graphql,
    Network,
    RateLimited,
    Malformed,
}

fn map_err(e: LinearError) -> LinearCommandError {
    let code = match &e {
        LinearError::NotSignedIn => LinearErrorCode::NotSignedIn,
        LinearError::Unauthorized => LinearErrorCode::Unauthorized,
        LinearError::Graphql(_) => LinearErrorCode::Graphql,
        LinearError::Network(_) => LinearErrorCode::Network,
        LinearError::RateLimited(_) => LinearErrorCode::RateLimited,
        LinearError::Malformed(_) => LinearErrorCode::Malformed,
        LinearError::Secrets(_) => LinearErrorCode::Storage,
    };
    LinearCommandError {
        code,
        message: e.to_string(),
    }
}

fn map_storage(msg: String) -> LinearCommandError {
    LinearCommandError {
        code: LinearErrorCode::Storage,
        message: msg,
    }
}

async fn authed_client(secrets: &Arc<Secrets>) -> Result<LinearClient, LinearCommandError> {
    let token = secrets
        .get(TOKEN_SECRET_KEY)
        .map_err(|e| map_storage(e.to_string()))?
        .unwrap_or_default();
    if token.is_empty() {
        return Err(LinearCommandError {
            code: LinearErrorCode::NotSignedIn,
            message: "no token stored".into(),
        });
    }
    LinearClient::new(token).map_err(map_err)
}

async fn handle_auth_failure(
    secrets: &Arc<Secrets>,
    db: &Arc<Db>,
    ui_sync: &Arc<UiSyncManager>,
    err: &LinearCommandError,
) {
    if matches!(err.code, LinearErrorCode::Unauthorized) {
        // EMD-14 spec: 401 clears the token + surfaces a re-auth prompt.
        let _ = secrets.set(TOKEN_SECRET_KEY, "");
        let _ = linear::clear_identity(db);
        ui_sync.broadcast(UiMutationEvent::LinearIdentityChanged);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn linear_sign_in(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    token: String,
) -> Result<LinearIdentityRecord, LinearCommandError> {
    let secrets = secrets.inner().clone();
    let db = db.inner().clone();
    let ui_sync = ui_sync.inner().clone();

    if token.trim().is_empty() {
        return Err(LinearCommandError {
            code: LinearErrorCode::NotSignedIn,
            message: "empty token".into(),
        });
    }
    secrets
        .set(TOKEN_SECRET_KEY, token.trim())
        .map_err(|e| map_storage(e.to_string()))?;
    refresh_identity(&secrets, &db, &ui_sync).await
}

#[tauri::command]
#[specta::specta]
pub fn linear_sign_out(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
) -> Result<(), LinearCommandError> {
    secrets
        .set(TOKEN_SECRET_KEY, "")
        .map_err(|e| map_storage(e.to_string()))?;
    linear::clear_identity(&db).map_err(|e| map_storage(e.to_string()))?;
    ui_sync.broadcast(UiMutationEvent::LinearIdentityChanged);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn linear_me(
    db: State<'_, Arc<Db>>,
) -> Result<Option<LinearIdentityRecord>, LinearCommandError> {
    linear::get_identity(&db).map_err(|e| map_storage(e.to_string()))
}

async fn refresh_identity(
    secrets: &Arc<Secrets>,
    db: &Arc<Db>,
    ui_sync: &Arc<UiSyncManager>,
) -> Result<LinearIdentityRecord, LinearCommandError> {
    let client = authed_client(secrets).await?;
    let viewer = client.fetch_viewer().await.map_err(map_err)?;
    let record = LinearIdentityRecord {
        id: viewer.id,
        name: viewer.name,
        display_name: viewer.display_name,
        email: viewer.email,
        avatar_url: viewer.avatar_url,
    };
    linear::set_identity(db, &record).map_err(|e| map_storage(e.to_string()))?;
    ui_sync.broadcast(UiMutationEvent::LinearIdentityChanged);
    Ok(record)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_teams(
    secrets: State<'_, Arc<Secrets>>,
    db: State<'_, Arc<Db>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
) -> Result<Vec<LinearTeam>, LinearCommandError> {
    let secrets = secrets.inner().clone();
    let db = db.inner().clone();
    let ui_sync = ui_sync.inner().clone();
    let client = authed_client(&secrets).await?;
    let res = client.list_teams().await.map_err(map_err);
    if let Err(e) = &res {
        handle_auth_failure(&secrets, &db, &ui_sync, e).await;
    }
    res
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_projects(
    secrets: State<'_, Arc<Secrets>>,
    team_id: String,
) -> Result<Vec<LinearProject>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_projects(&team_id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_cycles(
    secrets: State<'_, Arc<Secrets>>,
    team_id: String,
) -> Result<Vec<LinearCycle>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_cycles(&team_id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_labels(
    secrets: State<'_, Arc<Secrets>>,
    team_id: String,
) -> Result<Vec<LinearLabel>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_labels(&team_id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_states(
    secrets: State<'_, Arc<Secrets>>,
    team_id: String,
) -> Result<Vec<LinearWorkflowState>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_states(&team_id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_issues(
    secrets: State<'_, Arc<Secrets>>,
    filter: LinearIssueFilter,
) -> Result<Vec<LinearIssue>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_issues(filter).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_get_issue(
    secrets: State<'_, Arc<Secrets>>,
    id: String,
) -> Result<LinearIssue, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.get_issue(&id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_create_issue(
    secrets: State<'_, Arc<Secrets>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    input: LinearIssueCreateInput,
) -> Result<LinearIssue, LinearCommandError> {
    let team_id = input.team_id.clone();
    let client = authed_client(secrets.inner()).await?;
    let issue = client.create_issue(input).await.map_err(map_err)?;
    ui_sync.broadcast(UiMutationEvent::LinearDataChanged { team: team_id });
    Ok(issue)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_update_issue(
    secrets: State<'_, Arc<Secrets>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    id: String,
    input: LinearIssueUpdateInput,
) -> Result<LinearIssue, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    let issue = client.update_issue(&id, input).await.map_err(map_err)?;
    ui_sync.broadcast(UiMutationEvent::LinearDataChanged {
        team: issue.team_id.clone(),
    });
    Ok(issue)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_list_comments(
    secrets: State<'_, Arc<Secrets>>,
    issue_id: String,
) -> Result<Vec<LinearIssueComment>, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client.list_comments(&issue_id).await.map_err(map_err)
}

#[tauri::command]
#[specta::specta]
pub async fn linear_create_comment(
    secrets: State<'_, Arc<Secrets>>,
    issue_id: String,
    body: String,
) -> Result<LinearIssueComment, LinearCommandError> {
    let client = authed_client(secrets.inner()).await?;
    client
        .create_comment(&issue_id, &body)
        .await
        .map_err(map_err)
}
