//! Pull-request bridge. Wraps `github_list_pulls` for the renderer's
//! richer surface area. Mutating ops (create / merge / mark-ready /
//! sync) are not implemented yet — host returns empty / not-supported
//! envelopes so the PR panel renders cleanly.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct PullRequestStub {
    pub id: String,
    pub number: i32,
    pub title: String,
    pub state: String,
    pub url: String,
}

#[tauri::command]
#[specta::specta]
pub fn pull_requests_list() -> Vec<PullRequestStub> {
    Vec::new()
}

#[tauri::command]
#[specta::specta]
pub fn pull_requests_for_task(_task_id: String) -> Vec<PullRequestStub> {
    Vec::new()
}

#[derive(Debug, Serialize, Type)]
pub struct PrComment {
    pub id: String,
    pub body: String,
    pub author: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PrFilterOptions {
    pub repos: Vec<String>,
    pub authors: Vec<String>,
    pub labels: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn pull_requests_get_comments(_repo: String, _number: i32) -> Vec<PrComment> {
    Vec::new()
}

#[tauri::command]
#[specta::specta]
pub fn pull_requests_get_filter_options() -> PrFilterOptions {
    PrFilterOptions {
        repos: vec![],
        authors: vec![],
        labels: vec![],
    }
}
