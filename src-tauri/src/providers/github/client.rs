//! Thin `octocrab` wrapper. `client_for_token` is the single
//! constructor — every GitHub-backed call goes through it.
//!
//! `clippy::result_large_err` is allowed at the module level
//! because `octocrab::Error` is intentionally large (it wraps the
//! HTTP body for diagnostic detail). Boxing one variant would only
//! shave a few words off the enum and propagate `Box::leak`-style
//! patterns through every caller.

#![allow(clippy::result_large_err)]

use octocrab::Octocrab;
use thiserror::Error;

use super::model::{PullRequestSummary, RepoSummary, ViewerProfile};

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("octocrab error: {0}")]
    Octocrab(#[from] octocrab::Error),
    #[error("not signed in (no token stored)")]
    NotSignedIn,
    #[error("authentication failed (likely expired token)")]
    Unauthorized,
    #[error("rate limited; reset epoch: {0}")]
    RateLimited(i64),
    #[error("malformed response from GitHub: {0}")]
    Malformed(String),
}

/// Build an `Octocrab` client from a stored PAT / OAuth token.
pub fn client_for_token(token: &str) -> Result<Octocrab, GithubError> {
    Octocrab::builder()
        .personal_token(token.to_string())
        .build()
        .map_err(GithubError::Octocrab)
}

/// `GET /user` projection.
pub async fn fetch_viewer(client: &Octocrab) -> Result<ViewerProfile, GithubError> {
    let user = client.current().user().await?;
    // `octocrab::models::Author` doesn't expose `name` / `email` —
    // those live on the richer `Profile` variant. The `current().user()`
    // shortcut hands back the slim Author; for v1 we render with the
    // login + avatar and surface name/email only if the renderer
    // wants to fetch the richer endpoint later.
    Ok(ViewerProfile {
        login: user.login,
        id: user.id.to_string(),
        name: None,
        email: None,
        avatar_url: Some(user.avatar_url.to_string()),
    })
}

/// `GET /user/repos` — the user's accessible repos. octocrab returns
/// rich `Repository` structs; we project to the narrow renderer
/// surface and let the renderer ask for more on demand.
pub async fn list_repos(client: &Octocrab) -> Result<Vec<RepoSummary>, GithubError> {
    let mut out = Vec::new();
    let mut page: octocrab::Page<octocrab::models::Repository> = client
        .current()
        .list_repos_for_authenticated_user()
        .per_page(100)
        .send()
        .await?;
    loop {
        for repo in &page.items {
            let owner_login = repo
                .owner
                .as_ref()
                .map(|o| o.login.clone())
                .unwrap_or_default();
            let name = repo.name.clone();
            let full_name = repo
                .full_name
                .clone()
                .unwrap_or_else(|| format!("{owner_login}/{name}"));
            out.push(RepoSummary {
                owner: owner_login,
                name,
                full_name,
                private: repo.private.unwrap_or(false),
                default_branch: repo.default_branch.clone(),
                description: repo.description.clone(),
            });
        }
        match client
            .get_page::<octocrab::models::Repository>(&page.next)
            .await?
        {
            Some(next) => page = next,
            None => break,
        }
    }
    Ok(out)
}

/// `GET /repos/{owner}/{repo}/pulls` projection.
pub async fn list_pulls(
    client: &Octocrab,
    owner: &str,
    repo: &str,
) -> Result<Vec<PullRequestSummary>, GithubError> {
    let pulls = client
        .pulls(owner, repo)
        .list()
        .state(octocrab::params::State::Open)
        .per_page(50)
        .send()
        .await?;
    let summaries = pulls
        .items
        .into_iter()
        .map(|pr| PullRequestSummary {
            number: pr.number as i32,
            title: pr.title.unwrap_or_default(),
            state: pr
                .state
                .map(|s| match s {
                    octocrab::models::IssueState::Open => "open".to_string(),
                    octocrab::models::IssueState::Closed => "closed".to_string(),
                    _ => "unknown".to_string(),
                })
                .unwrap_or_else(|| "unknown".to_string()),
            draft: pr.draft.unwrap_or(false),
            html_url: pr.html_url.map(|u| u.to_string()).unwrap_or_default(),
            author: pr.user.map(|u| u.login),
            base_ref: pr.base.ref_field,
            head_ref: pr.head.ref_field,
            created_at: pr.created_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            updated_at: pr.updated_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
        })
        .collect();
    Ok(summaries)
}

/// `GET /repos/{owner}/{repo}/pulls/{number}` projection.
pub async fn get_pull(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<PullRequestSummary, GithubError> {
    let pr = client.pulls(owner, repo).get(number).await?;
    Ok(PullRequestSummary {
        number: pr.number as i32,
        title: pr.title.unwrap_or_default(),
        state: pr
            .state
            .map(|s| match s {
                octocrab::models::IssueState::Open => "open".to_string(),
                octocrab::models::IssueState::Closed => "closed".to_string(),
                _ => "unknown".to_string(),
            })
            .unwrap_or_else(|| "unknown".to_string()),
        draft: pr.draft.unwrap_or(false),
        html_url: pr.html_url.map(|u| u.to_string()).unwrap_or_default(),
        author: pr.user.map(|u| u.login),
        base_ref: pr.base.ref_field,
        head_ref: pr.head.ref_field,
        created_at: pr.created_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
        updated_at: pr.updated_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
    })
}

/// `GET /repos/{owner}/{repo}/pulls/{number}.diff` — returns the
/// unified diff as plain text. octocrab's `diff` method returns the
/// raw body.
pub async fn get_pull_diff(
    client: &Octocrab,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<String, GithubError> {
    let diff = client.pulls(owner, repo).get_diff(number).await?;
    Ok(diff)
}
