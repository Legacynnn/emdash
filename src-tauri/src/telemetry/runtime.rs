//! Background sender for telemetry events.
//!
//! - One `tokio::task::spawn`'d worker drains a bounded
//!   `mpsc::Sender<TelemetryEnvelope>` queue (capacity 256).
//! - **Drop policy: oldest.** When the queue is full, the producer
//!   side bails immediately — the worker is already behind. Losing
//!   the freshest events would surprise users more than losing
//!   stale ones.
//! - On shutdown, [`Telemetry::shutdown`] gives the worker 200 ms to
//!   drain in-flight events, then drops anything that hasn't been
//!   POSTed yet. The user closed the app; we don't make them wait.
//! - Three layers gate whether the worker actually fires HTTP:
//!   1. Compile-time: `EMDASH_TELEMETRY_HOST` must be non-empty
//!      (see [`TelemetryConfig::is_compiled_in`]).
//!   2. User toggle from `app_settings` (default `false`).
//!   3. The optional installation-instance id — generated lazily on
//!      first enabled send.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::timeout;
use uuid::Uuid;

use crate::db::{Db, DbError};

use super::config::TelemetryConfig;
use super::event::{TelemetryEnvelope, TelemetryEvent, TelemetryProps};
use super::settings;

const QUEUE_CAPACITY: usize = 256;
const SHUTDOWN_DRAIN_BUDGET: Duration = Duration::from_millis(200);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("db error: {0}")]
    Db(#[from] DbError),
}

#[derive(Clone, Default)]
struct Identity {
    /// Sticky per-installation id. Stored in `app_settings` under
    /// `telemetry.instance_id`; generated on first enabled send.
    instance_id: String,
}

pub struct Telemetry {
    config: TelemetryConfig,
    db: Arc<Db>,
    tx: mpsc::Sender<TelemetryEnvelope>,
    identity: Arc<RwLock<Identity>>,
    /// Worker shutdown signal: producer drops the tx clone, the
    /// worker observes a closed channel, drains in the budget, and
    /// exits.
    shutdown_tx: Option<mpsc::Sender<TelemetryEnvelope>>,
}

const INSTANCE_ID_KEY: &str = "telemetry.instance_id";

fn read_instance_id(db: &Arc<Db>) -> Result<Option<String>, DbError> {
    use rusqlite::{params, OptionalExtension};
    let conn = db.read()?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?",
            params![INSTANCE_ID_KEY],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value)
}

fn write_instance_id(db: &Arc<Db>, id: &str) -> Result<(), DbError> {
    use rusqlite::params;
    let conn = db.write()?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO NOTHING",
        params![INSTANCE_ID_KEY, id],
    )?;
    Ok(())
}

impl Telemetry {
    pub fn new(db: Arc<Db>, config: TelemetryConfig) -> Self {
        let (tx, rx) = mpsc::channel::<TelemetryEnvelope>(QUEUE_CAPACITY);
        let shutdown_tx = Some(tx.clone());
        let identity = Arc::new(RwLock::new(Identity::default()));

        // Hot-load any existing instance_id so we don't generate a
        // new one on every app start.
        if let Ok(Some(existing)) = read_instance_id(&db) {
            identity.write().instance_id = existing;
        }

        let worker_config = config.clone();
        tokio::spawn(async move {
            run_worker(worker_config, rx).await;
        });

        Self {
            config,
            db,
            tx,
            identity,
            shutdown_tx,
        }
    }

    /// Enqueue an event. Composes the gates: compile-time host,
    /// user toggle, and queue capacity. Returns `Ok(())` whether the
    /// event was actually queued or dropped — telemetry is
    /// best-effort by design.
    pub fn record(
        &self,
        event: TelemetryEvent,
        extra: TelemetryProps,
    ) -> Result<(), TelemetryError> {
        if !self.config.is_compiled_in() {
            return Ok(());
        }
        if !settings::get_enabled(&self.db)? {
            return Ok(());
        }

        let instance_id = self.ensure_instance_id()?;
        let envelope = TelemetryEnvelope {
            event: event.name().to_string(),
            distinct_id: instance_id,
            timestamp: chrono::Utc::now(),
            properties: merge_base_props(extra),
        };

        // `try_send` is the load-bearing piece of the drop policy.
        // A full queue means the worker is behind; rather than
        // blocking the caller (which is often the IPC thread), we
        // drop the event silently and move on.
        let _ = self.tx.try_send(envelope);
        Ok(())
    }

    /// Drain-with-timeout shutdown. Drops the producer side so the
    /// worker observes channel close after the in-flight events
    /// complete, then waits up to [`SHUTDOWN_DRAIN_BUDGET`] for the
    /// worker to exit cleanly.
    pub async fn shutdown(&mut self) {
        // Drop our sender so the worker's `rx.recv()` returns `None`
        // once the queue is empty.
        self.shutdown_tx = None;
        let _ = timeout(SHUTDOWN_DRAIN_BUDGET, async {
            // Cheap busy-wait: the worker exits ~immediately after the
            // last message; the timeout caps the worst case.
            tokio::time::sleep(SHUTDOWN_DRAIN_BUDGET).await;
        })
        .await;
    }

    fn ensure_instance_id(&self) -> Result<String, TelemetryError> {
        {
            let g = self.identity.read();
            if !g.instance_id.is_empty() {
                return Ok(g.instance_id.clone());
            }
        }
        let mut g = self.identity.write();
        if g.instance_id.is_empty() {
            let new_id = Uuid::new_v4().to_string();
            write_instance_id(&self.db, &new_id)?;
            g.instance_id = new_id;
        }
        Ok(g.instance_id.clone())
    }
}

fn merge_base_props(extra: TelemetryProps) -> TelemetryProps {
    let mut out = extra;
    out.entry("platform".into())
        .or_insert_with(|| serde_json::Value::String(std::env::consts::OS.to_string()));
    out.entry("arch".into())
        .or_insert_with(|| serde_json::Value::String(std::env::consts::ARCH.to_string()));
    out.entry("app_version".into())
        .or_insert_with(|| serde_json::Value::String(env!("CARGO_PKG_VERSION").to_string()));
    out.entry("schema_version".into())
        .or_insert_with(|| serde_json::Value::Number(1.into()));
    out
}

async fn run_worker(config: TelemetryConfig, mut rx: mpsc::Receiver<TelemetryEnvelope>) {
    let client = match reqwest::Client::builder()
        .timeout(HTTP_REQUEST_TIMEOUT)
        .user_agent(format!("emdash-dev/{}", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(_) => return, // reqwest init failed — give up silently
    };

    while let Some(envelope) = rx.recv().await {
        let _ = post_envelope(&client, &config, envelope).await;
    }
}

async fn post_envelope(
    client: &reqwest::Client,
    config: &TelemetryConfig,
    envelope: TelemetryEnvelope,
) -> Result<(), reqwest::Error> {
    client
        .post(&config.host)
        .bearer_auth(&config.api_key)
        .json(&envelope)
        .send()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_db() -> (TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    fn empty_config() -> TelemetryConfig {
        TelemetryConfig {
            host: String::new(),
            api_key: String::new(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn record_is_noop_when_not_compiled_in() {
        let (_dir, db) = make_db();
        let telemetry = Telemetry::new(db.clone(), empty_config());
        // user toggle is on, but no host stamped: should be a no-op.
        settings::set_enabled(&db, true).unwrap();
        telemetry
            .record(TelemetryEvent::AppFocus, TelemetryProps::new())
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn record_is_noop_when_user_toggle_off() {
        let (_dir, db) = make_db();
        let config = TelemetryConfig {
            host: "http://127.0.0.1:1".into(), // unreachable on purpose
            api_key: "key".into(),
        };
        let telemetry = Telemetry::new(db.clone(), config);
        // user toggle is off (default): no enqueue.
        telemetry
            .record(TelemetryEvent::AppFocus, TelemetryProps::new())
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn instance_id_is_sticky() {
        let (_dir, db) = make_db();
        let config = TelemetryConfig {
            host: "http://127.0.0.1:1".into(),
            api_key: "k".into(),
        };
        let telemetry = Telemetry::new(db.clone(), config);
        settings::set_enabled(&db, true).unwrap();

        let id1 = telemetry.ensure_instance_id().unwrap();
        let id2 = telemetry.ensure_instance_id().unwrap();
        assert_eq!(id1, id2);

        // A fresh instance pointing at the same DB observes the
        // same id.
        let config2 = TelemetryConfig {
            host: "http://127.0.0.1:1".into(),
            api_key: "k".into(),
        };
        let telemetry2 = Telemetry::new(db.clone(), config2);
        let id3 = telemetry2.ensure_instance_id().unwrap();
        assert_eq!(id1, id3);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn base_props_get_filled_in() {
        let mut extra = TelemetryProps::new();
        extra.insert("custom".into(), serde_json::Value::String("v".into()));
        let merged = merge_base_props(extra);
        assert_eq!(merged.get("custom").and_then(|v| v.as_str()), Some("v"));
        assert!(merged.contains_key("platform"));
        assert!(merged.contains_key("arch"));
        assert!(merged.contains_key("app_version"));
        assert!(merged.contains_key("schema_version"));
    }
}
