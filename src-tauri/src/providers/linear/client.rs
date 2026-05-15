//! Lightweight Linear GraphQL client.
//!
//! Linear's API is GraphQL-only. We hand-roll one function per query
//! to keep the dep surface small (no `graphql-client` codegen). Each
//! query string is inlined; responses are deserialized into the
//! narrow renderer-facing structs in `model.rs`.
//!
//! Rate-limit headers (`X-RateLimit-Remaining` / `X-RateLimit-Reset`)
//! are read on every response; 429 / 401 map to typed errors so the
//! renderer can surface the right message.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

use super::error::LinearError;
use super::model::{
    LinearCycle, LinearIssue, LinearIssueComment, LinearIssueCreateInput, LinearIssueFilter,
    LinearIssueUpdateInput, LinearLabel, LinearProject, LinearTeam, LinearViewerProfile,
    LinearWorkflowState,
};

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

pub struct LinearClient {
    http: reqwest::Client,
    token: String,
}

impl LinearClient {
    pub fn new(token: String) -> Result<Self, LinearError> {
        let http = reqwest::Client::builder()
            .user_agent(format!("emdash-dev/{}", env!("CARGO_PKG_VERSION")))
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| LinearError::Network(e.to_string()))?;
        Ok(Self { http, token })
    }

    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T, LinearError> {
        let body = json!({ "query": query, "variables": variables });
        let resp = self
            .http
            .post(LINEAR_API_URL)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LinearError::Network(e.to_string()))?;

        let status = resp.status();
        if status == 401 || status == 403 {
            return Err(LinearError::Unauthorized);
        }
        if status == 429 {
            // Linear sends `X-RateLimit-Reset` as a Unix epoch second.
            let reset = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            return Err(LinearError::RateLimited(reset));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|_| LinearError::Malformed("non-JSON response".into()))?;

        if let Some(errors) = body.get("errors") {
            let msg = errors
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            return Err(LinearError::Graphql(msg.to_string()));
        }

        let data = body
            .get("data")
            .ok_or_else(|| LinearError::Malformed("response missing `data`".into()))?
            .clone();
        serde_json::from_value(data).map_err(|e| LinearError::Malformed(e.to_string()))
    }

    /// `viewer { id name displayName email avatarUrl }`
    pub async fn fetch_viewer(&self) -> Result<LinearViewerProfile, LinearError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ViewerRaw {
            id: String,
            name: String,
            display_name: Option<String>,
            email: Option<String>,
            avatar_url: Option<String>,
        }
        #[derive(Deserialize)]
        struct Wrap {
            viewer: ViewerRaw,
        }
        let q = "query { viewer { id name displayName email avatarUrl } }";
        let w: Wrap = self.query(q, json!({})).await?;
        Ok(LinearViewerProfile {
            id: w.viewer.id,
            name: w.viewer.name,
            display_name: w.viewer.display_name,
            email: w.viewer.email,
            avatar_url: w.viewer.avatar_url,
        })
    }

    pub async fn list_teams(&self) -> Result<Vec<LinearTeam>, LinearError> {
        #[derive(Deserialize)]
        struct TeamRaw {
            id: String,
            key: String,
            name: String,
            description: Option<String>,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<TeamRaw>,
        }
        #[derive(Deserialize)]
        struct Wrap {
            teams: Conn,
        }
        let q = "query { teams { nodes { id key name description } } }";
        let w: Wrap = self.query(q, json!({})).await?;
        Ok(w.teams
            .nodes
            .into_iter()
            .map(|t| LinearTeam {
                id: t.id,
                key: t.key,
                name: t.name,
                description: t.description,
            })
            .collect())
    }

    pub async fn list_projects(&self, team_id: &str) -> Result<Vec<LinearProject>, LinearError> {
        #[derive(Deserialize)]
        struct ProjectRaw {
            id: String,
            name: String,
            state: String,
            description: Option<String>,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<ProjectRaw>,
        }
        #[derive(Deserialize)]
        struct TeamWrap {
            projects: Conn,
        }
        #[derive(Deserialize)]
        struct Wrap {
            team: TeamWrap,
        }
        let q = "query($teamId: String!) { \
                 team(id: $teamId) { \
                   projects { nodes { id name state description } } \
                 } }";
        let w: Wrap = self.query(q, json!({ "teamId": team_id })).await?;
        Ok(w.team
            .projects
            .nodes
            .into_iter()
            .map(|p| LinearProject {
                id: p.id,
                name: p.name,
                state: p.state,
                description: p.description,
            })
            .collect())
    }

    pub async fn list_cycles(&self, team_id: &str) -> Result<Vec<LinearCycle>, LinearError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CycleRaw {
            id: String,
            number: i32,
            name: Option<String>,
            starts_at: Option<String>,
            ends_at: Option<String>,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<CycleRaw>,
        }
        #[derive(Deserialize)]
        struct TeamWrap {
            cycles: Conn,
        }
        #[derive(Deserialize)]
        struct Wrap {
            team: TeamWrap,
        }
        let q = "query($teamId: String!) { \
                 team(id: $teamId) { \
                   cycles { nodes { id number name startsAt endsAt } } \
                 } }";
        let w: Wrap = self.query(q, json!({ "teamId": team_id })).await?;
        Ok(w.team
            .cycles
            .nodes
            .into_iter()
            .map(|c| LinearCycle {
                id: c.id,
                number: c.number,
                name: c.name,
                starts_at: c.starts_at,
                ends_at: c.ends_at,
            })
            .collect())
    }

    pub async fn list_labels(&self, team_id: &str) -> Result<Vec<LinearLabel>, LinearError> {
        #[derive(Deserialize)]
        struct LabelRaw {
            id: String,
            name: String,
            color: String,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<LabelRaw>,
        }
        #[derive(Deserialize)]
        struct TeamWrap {
            labels: Conn,
        }
        #[derive(Deserialize)]
        struct Wrap {
            team: TeamWrap,
        }
        let q = "query($teamId: String!) { \
                 team(id: $teamId) { \
                   labels { nodes { id name color } } \
                 } }";
        let w: Wrap = self.query(q, json!({ "teamId": team_id })).await?;
        Ok(w.team
            .labels
            .nodes
            .into_iter()
            .map(|l| LinearLabel {
                id: l.id,
                name: l.name,
                color: l.color,
            })
            .collect())
    }

    pub async fn list_states(
        &self,
        team_id: &str,
    ) -> Result<Vec<LinearWorkflowState>, LinearError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct StateRaw {
            id: String,
            name: String,
            #[serde(rename = "type")]
            state_type: String,
            color: String,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<StateRaw>,
        }
        #[derive(Deserialize)]
        struct TeamWrap {
            states: Conn,
        }
        #[derive(Deserialize)]
        struct Wrap {
            team: TeamWrap,
        }
        let q = "query($teamId: String!) { \
                 team(id: $teamId) { \
                   states { nodes { id name type color } } \
                 } }";
        let w: Wrap = self.query(q, json!({ "teamId": team_id })).await?;
        Ok(w.team
            .states
            .nodes
            .into_iter()
            .map(|s| LinearWorkflowState {
                id: s.id,
                name: s.name,
                state_type: s.state_type,
                color: s.color,
            })
            .collect())
    }

    pub async fn list_issues(
        &self,
        filter: LinearIssueFilter,
    ) -> Result<Vec<LinearIssue>, LinearError> {
        let mut filter_map = serde_json::Map::new();
        if let Some(team_id) = &filter.team_id {
            filter_map.insert("team".into(), json!({ "id": { "eq": team_id } }));
        }
        if let Some(project_id) = &filter.project_id {
            filter_map.insert("project".into(), json!({ "id": { "eq": project_id } }));
        }
        if let Some(cycle_id) = &filter.cycle_id {
            filter_map.insert("cycle".into(), json!({ "id": { "eq": cycle_id } }));
        }
        if let Some(state_id) = &filter.state_id {
            filter_map.insert("state".into(), json!({ "id": { "eq": state_id } }));
        }
        if let Some(assignee_id) = &filter.assignee_id {
            filter_map.insert("assignee".into(), json!({ "id": { "eq": assignee_id } }));
        }

        let limit = filter.limit.unwrap_or(50).clamp(1, 250);
        let q = "query($filter: LinearIssueFilter, $first: Int) { \
                 issues(filter: $filter, first: $first) { \
                   nodes { \
                     id identifier number title description priority url createdAt updatedAt \
                     state { id name } \
                     assignee { id name } \
                     team { id } \
                     project { id } \
                     cycle { id } \
                     parent { id } \
                   } \
                 } }";
        let payload: Value = self
            .query(
                q,
                json!({ "filter": Value::Object(filter_map), "first": limit }),
            )
            .await?;
        decode_issue_list(payload)
    }

    pub async fn get_issue(&self, id: &str) -> Result<LinearIssue, LinearError> {
        let q = "query($id: String!) { \
                 issue(id: $id) { \
                   id identifier number title description priority url createdAt updatedAt \
                   state { id name } \
                   assignee { id name } \
                   team { id } \
                   project { id } \
                   cycle { id } \
                   parent { id } \
                 } }";
        let payload: Value = self.query(q, json!({ "id": id })).await?;
        decode_single_issue(payload)
    }

    pub async fn create_issue(
        &self,
        input: LinearIssueCreateInput,
    ) -> Result<LinearIssue, LinearError> {
        let mut vars = serde_json::Map::new();
        vars.insert("teamId".into(), json!(input.team_id));
        vars.insert("title".into(), json!(input.title));
        if let Some(d) = input.description {
            vars.insert("description".into(), json!(d));
        }
        if let Some(s) = input.state_id {
            vars.insert("stateId".into(), json!(s));
        }
        if let Some(a) = input.assignee_id {
            vars.insert("assigneeId".into(), json!(a));
        }
        if let Some(p) = input.priority {
            vars.insert("priority".into(), json!(p));
        }
        if let Some(p) = input.project_id {
            vars.insert("projectId".into(), json!(p));
        }

        let q = "mutation($teamId: String!, $title: String!, $description: String, $stateId: String, $assigneeId: String, $priority: Int, $projectId: String) { \
                 issueCreate(input: { teamId: $teamId, title: $title, description: $description, stateId: $stateId, assigneeId: $assigneeId, priority: $priority, projectId: $projectId }) { \
                   success issue { \
                     id identifier number title description priority url createdAt updatedAt \
                     state { id name } \
                     assignee { id name } \
                     team { id } \
                     project { id } \
                     cycle { id } \
                     parent { id } \
                   } \
                 } }";
        let payload: Value = self.query(q, Value::Object(vars)).await?;
        let issue_value = payload
            .get("issueCreate")
            .and_then(|m| m.get("issue"))
            .cloned()
            .ok_or_else(|| LinearError::Malformed("issueCreate missing issue".into()))?;
        decode_issue(issue_value)
    }

    pub async fn update_issue(
        &self,
        id: &str,
        input: LinearIssueUpdateInput,
    ) -> Result<LinearIssue, LinearError> {
        let mut input_map = serde_json::Map::new();
        if let Some(t) = input.title {
            input_map.insert("title".into(), json!(t));
        }
        if let Some(d) = input.description {
            input_map.insert("description".into(), json!(d));
        }
        if let Some(s) = input.state_id {
            input_map.insert("stateId".into(), json!(s));
        }
        if let Some(a) = input.assignee_id {
            input_map.insert("assigneeId".into(), json!(a));
        }
        if let Some(p) = input.priority {
            input_map.insert("priority".into(), json!(p));
        }

        let q = "mutation($id: String!, $input: LinearIssueUpdateInput!) { \
                 issueUpdate(id: $id, input: $input) { \
                   success issue { \
                     id identifier number title description priority url createdAt updatedAt \
                     state { id name } \
                     assignee { id name } \
                     team { id } \
                     project { id } \
                     cycle { id } \
                     parent { id } \
                   } \
                 } }";
        let payload: Value = self
            .query(q, json!({ "id": id, "input": Value::Object(input_map) }))
            .await?;
        let issue_value = payload
            .get("issueUpdate")
            .and_then(|m| m.get("issue"))
            .cloned()
            .ok_or_else(|| LinearError::Malformed("issueUpdate missing issue".into()))?;
        decode_issue(issue_value)
    }

    pub async fn list_comments(
        &self,
        issue_id: &str,
    ) -> Result<Vec<LinearIssueComment>, LinearError> {
        #[derive(Deserialize)]
        struct UserRaw {
            name: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CommentRaw {
            id: String,
            body: String,
            user: Option<UserRaw>,
            created_at: String,
            updated_at: String,
        }
        #[derive(Deserialize)]
        struct Conn {
            nodes: Vec<CommentRaw>,
        }
        #[derive(Deserialize)]
        struct IssueWrap {
            comments: Conn,
        }
        #[derive(Deserialize)]
        struct Wrap {
            issue: IssueWrap,
        }
        let q = "query($id: String!) { \
                 issue(id: $id) { \
                   comments { nodes { id body user { name } createdAt updatedAt } } \
                 } }";
        let w: Wrap = self.query(q, json!({ "id": issue_id })).await?;
        Ok(w.issue
            .comments
            .nodes
            .into_iter()
            .map(|c| LinearIssueComment {
                id: c.id,
                body: c.body,
                user_name: c.user.and_then(|u| u.name),
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect())
    }

    pub async fn create_comment(
        &self,
        issue_id: &str,
        body: &str,
    ) -> Result<LinearIssueComment, LinearError> {
        let q = "mutation($issueId: String!, $body: String!) { \
                 commentCreate(input: { issueId: $issueId, body: $body }) { \
                   success comment { id body user { name } createdAt updatedAt } \
                 } }";
        let payload: Value = self
            .query(q, json!({ "issueId": issue_id, "body": body }))
            .await?;
        let c = payload
            .get("commentCreate")
            .and_then(|m| m.get("comment"))
            .ok_or_else(|| LinearError::Malformed("commentCreate missing comment".into()))?;
        let id = c.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let cbody = c.get("body").and_then(|v| v.as_str()).unwrap_or_default();
        let user_name = c
            .get("user")
            .and_then(|u| u.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        let created_at = c
            .get("createdAt")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let updated_at = c
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        Ok(LinearIssueComment {
            id: id.to_string(),
            body: cbody.to_string(),
            user_name,
            created_at: created_at.to_string(),
            updated_at: updated_at.to_string(),
        })
    }
}

fn decode_issue_list(payload: Value) -> Result<Vec<LinearIssue>, LinearError> {
    let nodes = payload
        .get("issues")
        .and_then(|i| i.get("nodes"))
        .and_then(|n| n.as_array())
        .ok_or_else(|| LinearError::Malformed("issues.nodes missing".into()))?;
    nodes.iter().map(|v| decode_issue(v.clone())).collect()
}

fn decode_single_issue(payload: Value) -> Result<LinearIssue, LinearError> {
    let v = payload
        .get("issue")
        .cloned()
        .ok_or_else(|| LinearError::Malformed("issue missing".into()))?;
    decode_issue(v)
}

fn decode_issue(v: Value) -> Result<LinearIssue, LinearError> {
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| LinearError::Malformed("issue.id".into()))?
        .to_string();
    let identifier = v
        .get("identifier")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let number = v.get("number").and_then(|x| x.as_i64()).unwrap_or_default() as i32;
    let title = v
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let description = v
        .get("description")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let priority = v.get("priority").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    let url = v
        .get("url")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let created_at = v
        .get("createdAt")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let updated_at = v
        .get("updatedAt")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let state = v.get("state");
    let state_id = state
        .and_then(|s| s.get("id"))
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let state_name = state
        .and_then(|s| s.get("name"))
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let assignee = v.get("assignee");
    let assignee_id = assignee
        .and_then(|a| a.get("id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let assignee_name = assignee
        .and_then(|a| a.get("name"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let team_id = v
        .get("team")
        .and_then(|t| t.get("id"))
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let project_id = v
        .get("project")
        .and_then(|p| p.get("id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let cycle_id = v
        .get("cycle")
        .and_then(|c| c.get("id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let parent_id = v
        .get("parent")
        .and_then(|p| p.get("id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    Ok(LinearIssue {
        id,
        identifier,
        number,
        title,
        description,
        state_name,
        state_id,
        priority,
        assignee_id,
        assignee_name,
        team_id,
        project_id,
        cycle_id,
        parent_id,
        url,
        created_at,
        updated_at,
    })
}
