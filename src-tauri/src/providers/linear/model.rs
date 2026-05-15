//! Renderer-facing projections. Narrow on purpose so the wire
//! format stays stable across Linear API churn.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearViewerProfile {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearTeam {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearProject {
    pub id: String,
    pub name: String,
    pub state: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearCycle {
    pub id: String,
    pub number: i32,
    pub name: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearWorkflowState {
    pub id: String,
    pub name: String,
    /// One of: `triage`, `backlog`, `unstarted`, `started`, `completed`, `canceled`.
    pub state_type: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String,
    pub number: i32,
    pub title: String,
    pub description: Option<String>,
    /// `state.name`; the full state object is not flattened to keep
    /// the v1 surface narrow.
    pub state_name: String,
    pub state_id: String,
    pub priority: i32,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    pub team_id: String,
    pub project_id: Option<String>,
    pub cycle_id: Option<String>,
    pub parent_id: Option<String>,
    pub url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct LinearIssueComment {
    pub id: String,
    pub body: String,
    pub user_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Filter set the renderer can pass into `linear.list_issues`. Each
/// field narrows the result set; `team_id` is typically the anchor.
#[derive(Clone, Debug, Serialize, Deserialize, Default, Type)]
pub struct LinearIssueFilter {
    pub team_id: Option<String>,
    pub project_id: Option<String>,
    pub cycle_id: Option<String>,
    pub state_id: Option<String>,
    pub assignee_id: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct LinearIssueCreateInput {
    pub team_id: String,
    pub title: String,
    pub description: Option<String>,
    pub state_id: Option<String>,
    pub assignee_id: Option<String>,
    pub priority: Option<i32>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Type)]
pub struct LinearIssueUpdateInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state_id: Option<String>,
    pub assignee_id: Option<String>,
    pub priority: Option<i32>,
}
