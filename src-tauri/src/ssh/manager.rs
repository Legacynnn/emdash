//! `SshConnectionManager` — connection pool. One `SshClient` per
//! saved connection id. Idempotent `connect`. Disconnect removes
//! the entry.
//!
//! Concurrency is `parking_lot::Mutex<HashMap>` because the inner
//! work (`connect`) is async and would need a different shape if we
//! used `tokio::Mutex`. The pattern: acquire lock → check existing
//! entry → drop lock → if absent, do the async connect → re-acquire
//! lock → insert (last writer wins, which is fine because we
//! short-circuit on the cheap path).

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use super::{ConnectionState, SshClient, SshConnection, SshError};

#[derive(Default)]
pub struct SshConnectionManager {
    sessions: Mutex<HashMap<String, Arc<SshClient>>>,
    states: Mutex<HashMap<String, ConnectionState>>,
}

impl SshConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.sessions.lock().contains_key(id)
    }

    pub fn get_state(&self, id: &str) -> ConnectionState {
        self.states
            .lock()
            .get(id)
            .cloned()
            .unwrap_or(ConnectionState::Disconnected)
    }

    pub fn all_states(&self) -> HashMap<String, ConnectionState> {
        self.states.lock().clone()
    }

    /// Ensure a session exists. Returns a clone of the `Arc` so
    /// callers can use it without holding the manager's lock.
    pub async fn connect(&self, connection: &SshConnection) -> Result<Arc<SshClient>, SshError> {
        {
            let sessions = self.sessions.lock();
            if let Some(existing) = sessions.get(&connection.id) {
                return Ok(existing.clone());
            }
        }
        self.states
            .lock()
            .insert(connection.id.clone(), ConnectionState::Connecting);
        match SshClient::connect(connection).await {
            Ok(client) => {
                let arc = Arc::new(client);
                self.sessions
                    .lock()
                    .insert(connection.id.clone(), arc.clone());
                self.states.lock().insert(
                    connection.id.clone(),
                    ConnectionState::Connected { latency_ms: None },
                );
                Ok(arc)
            }
            Err(e) => {
                self.states.lock().insert(
                    connection.id.clone(),
                    ConnectionState::Failed {
                        error: e.to_string(),
                    },
                );
                Err(e)
            }
        }
    }

    /// Drop the entry. The underlying session is consumed when the
    /// last `Arc` reference is dropped, which calls `Session::close`
    /// in `SshClient::disconnect`.
    pub async fn disconnect(&self, id: &str) -> Result<(), SshError> {
        let removed = self.sessions.lock().remove(id);
        self.states
            .lock()
            .insert(id.to_string(), ConnectionState::Disconnected);
        if let Some(arc) = removed {
            // If this was the last reference, the close happens in
            // `SshClient::drop` (which `openssh::Session` provides
            // implicitly).
            drop(arc);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manager_is_disconnected() {
        let m = SshConnectionManager::new();
        assert!(!m.is_connected("anything"));
        assert!(matches!(
            m.get_state("anything"),
            ConnectionState::Disconnected
        ));
    }

    #[test]
    fn all_states_is_empty_when_no_sessions() {
        let m = SshConnectionManager::new();
        assert!(m.all_states().is_empty());
    }

    // Successful-connect tests need a real SSH host, so they live in
    // an integration harness, not here. The client-side connect path
    // is exercised by `ssh::client::tests::test_connection_to_unreachable_host_reports_failure`.
}
