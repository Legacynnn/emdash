//! `HookServer` — axum on `127.0.0.1:0` with constant-time token
//! validation. POST `/hook` with `x-emdash-token: <UUID>` and a JSON
//! body whose top-level `agent` field selects a classifier.
//!
//! Output of a classifier is wrapped in an enriched `AgentEvent`
//! and handed to the broadcaster the caller supplies (typically a
//! thin closure that forwards into the `UiSyncManager`).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::event::{AgentEvent, AgentEventKind};
use super::registry::ClassifierRegistry;

#[derive(Debug, Error)]
pub enum HookServerError {
    #[error("io error binding hook server: {0}")]
    Io(#[from] std::io::Error),
    #[error("axum runtime error: {0}")]
    Serve(String),
}

/// Live handle to a running hook server. Drop it to stop the server.
pub struct HookServerHandle {
    pub port: u16,
    pub token: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl HookServerHandle {
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl Drop for HookServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub type EventBroadcaster = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Clone)]
struct AppState {
    registry: Arc<ClassifierRegistry>,
    token: String,
    broadcaster: EventBroadcaster,
}

/// Spawn the hook server. Binds before the future returned by
/// `start` resolves, so the caller can read `.port()` immediately.
pub struct HookServer;

impl HookServer {
    pub async fn start(
        registry: Arc<ClassifierRegistry>,
        broadcaster: EventBroadcaster,
    ) -> Result<HookServerHandle, HookServerError> {
        let token = Uuid::new_v4().to_string();
        let state = AppState {
            registry,
            token: token.clone(),
            broadcaster,
        };

        let app = Router::new()
            .route("/hook", post(handle_hook))
            .with_state(state);

        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
        let port = listener.local_addr()?.port();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let serve = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            if let Err(err) = serve.await {
                eprintln!("[agent_hooks] server exited with error: {err}");
            }
        });

        Ok(HookServerHandle {
            port,
            token,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        })
    }
}

#[derive(Debug, Deserialize)]
struct HookBody {
    agent: String,
    #[serde(flatten)]
    rest: serde_json::Value,
}

async fn handle_hook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Constant-time token compare. `subtle::ConstantTimeEq` ensures the
    // comparison's timing doesn't depend on which byte differs — so a
    // remote attacker can't iterate one byte at a time.
    let supplied = headers
        .get("x-emdash-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let supplied_bytes = supplied.as_bytes();
    let expected_bytes = state.token.as_bytes();
    if supplied_bytes.len() != expected_bytes.len()
        || !bool::from(supplied_bytes.ct_eq(expected_bytes))
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let parsed: HookBody = match serde_json::from_value(body.clone()) {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // Re-wrap `rest` to give the classifier the full original object
    // (some classifiers want `agent` itself accessible too).
    let mut classifier_input = parsed.rest.clone();
    if let serde_json::Value::Object(ref mut map) = classifier_input {
        map.insert(
            "agent".to_string(),
            serde_json::Value::String(parsed.agent.clone()),
        );
    }

    let (classifier_name, classified) = match state.registry.for_agent(&parsed.agent) {
        Some(classifier) => {
            let name = classifier.name().to_string();
            let result = classifier.classify(&classifier_input);
            (name, result)
        }
        None => (
            "<unknown>".to_string(),
            super::classifier::ClassificationResult {
                kind: AgentEventKind::Unknown,
                message: None,
            },
        ),
    };

    let event = AgentEvent {
        agent: parsed.agent,
        classifier: classifier_name,
        kind: classified.kind,
        message: classified.message,
        timestamp: Utc::now().to_rfc3339(),
        workspace_id: None,
        project_id: None,
    };
    (state.broadcaster)(event);
    StatusCode::OK.into_response()
}

#[cfg(test)]
mod tests {
    use super::super::classifier::ClaudeClassifier;
    use super::*;
    use std::sync::Mutex as StdMutex;

    fn make_registry() -> Arc<ClassifierRegistry> {
        let r = ClassifierRegistry::new();
        r.register(ClaudeClassifier);
        Arc::new(r)
    }

    fn collecting_broadcaster() -> (Arc<StdMutex<Vec<AgentEvent>>>, EventBroadcaster) {
        let log: Arc<StdMutex<Vec<AgentEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = log.clone();
        let b: EventBroadcaster = Arc::new(move |e| sink.lock().unwrap().push(e));
        (log, b)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_request_without_token() {
        let (_log, b) = collecting_broadcaster();
        let handle = HookServer::start(make_registry(), b).await.unwrap();
        let url = format!("http://127.0.0.1:{}/hook", handle.port);

        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .json(&serde_json::json!({ "agent": "claude" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_wrong_token() {
        let (_log, b) = collecting_broadcaster();
        let handle = HookServer::start(make_registry(), b).await.unwrap();
        let url = format!("http://127.0.0.1:{}/hook", handle.port);

        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .header("x-emdash-token", "wrong")
            .json(&serde_json::json!({ "agent": "claude" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn valid_request_classifies_and_broadcasts() {
        let (log, b) = collecting_broadcaster();
        let handle = HookServer::start(make_registry(), b).await.unwrap();
        let url = format!("http://127.0.0.1:{}/hook", handle.port);
        let token = handle.token.clone();

        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .header("x-emdash-token", &token)
            .json(&serde_json::json!({
                "agent": "claude",
                "hook_event_name": "Stop",
                "reason": "session ended"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);

        // Give the broadcaster a tick to land the event.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let events = log.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].agent, "claude");
        assert!(matches!(events[0].kind, AgentEventKind::Stop));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unknown_agent_still_broadcasts_unknown_event() {
        let (log, b) = collecting_broadcaster();
        let handle = HookServer::start(make_registry(), b).await.unwrap();
        let url = format!("http://127.0.0.1:{}/hook", handle.port);
        let token = handle.token.clone();

        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .header("x-emdash-token", &token)
            .json(&serde_json::json!({ "agent": "future-agent-x" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let events = log.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].kind, AgentEventKind::Unknown));
        assert_eq!(events[0].classifier, "<unknown>");
    }
}
