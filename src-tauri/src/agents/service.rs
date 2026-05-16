//! `AgentService` — start/stop an agent for a workspace.
//!
//! Composes:
//! - `workspaces::WorkspacesService` to resolve the workspace → path,
//!   `pty_id` column update on start/stop
//! - `pty::Registry` to spawn the PTY itself
//! - `shell_env::shell_env()` for the captured login-shell env
//! - `agent_hooks::inject_hook_env_into` to plant the hook URL +
//!   token before spawn
//! - `WorkspaceFsMutationLock` (re-used from workspaces) so concurrent
//!   start/stop on the same workspace serialize cleanly

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::params;
use thiserror::Error;

use crate::agent_hooks;
use crate::db::{Db, DbError};
use crate::pty::registry::Registry as PtyRegistry;
use crate::pty::types::{PtyId, PtySize, SpawnOptions};
use crate::shell_env;
use crate::workspaces::{WorkspaceFsMutationLock, WorkspacesService};

use super::provider::{provider_spec, AgentProvider};

#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),
    #[error("workspace already has an agent running (pty {0})")]
    AlreadyRunning(String),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("pty error: {0}")]
    Pty(String),
    #[error("workspaces error: {0}")]
    Workspaces(String),
}

/// Callback the Tauri glue passes in. The host wires this so each
/// PTY chunk routes to a `tauri::ipc::Channel<Vec<u8>>` the
/// renderer's xterm.js is consuming.
pub type OutputCallback = Arc<dyn Fn(Vec<u8>) + Send + Sync>;

#[derive(Clone)]
pub struct AgentService {
    db: Arc<Db>,
    pty: Arc<PtyRegistry>,
    fs_lock: Arc<WorkspaceFsMutationLock>,
    /// Coordinates injected into the agent's env so the agent can
    /// POST back to our local hook server. Both are stamped by
    /// `app.rs` setup before any agent can start.
    hook_port: u16,
    hook_token: String,
}

impl AgentService {
    pub fn new(
        db: Arc<Db>,
        pty: Arc<PtyRegistry>,
        fs_lock: Arc<WorkspaceFsMutationLock>,
        hook_port: u16,
        hook_token: String,
    ) -> Self {
        Self {
            db,
            pty,
            fs_lock,
            hook_port,
            hook_token,
        }
    }

    /// Start an agent for `workspace_id`. Returns the freshly-spawned
    /// `PtyId`. Side effects:
    /// - `workspaces.pty_id` updated to the new PTY id
    /// - PTY output streamed via `on_output`
    /// - hook env vars injected at spawn time
    pub fn start(
        &self,
        workspace_id: &str,
        provider: AgentProvider,
        size: PtySize,
        on_output: OutputCallback,
    ) -> Result<PtyId, AgentsError> {
        // Serialize start/stop on the same workspace so concurrent
        // requests can't double-spawn.
        let mutex = self.fs_lock.lock_for(workspace_id);
        let _guard = mutex.lock();

        let (workspace_path, existing_pty_id) = self.fetch_workspace_meta(workspace_id)?;
        if let Some(existing) = existing_pty_id {
            return Err(AgentsError::AlreadyRunning(existing));
        }

        let spec = provider_spec(provider);
        let mut env: HashMap<String, String> = shell_env::shell_env().captured.clone();
        for (k, v) in spec.env.iter() {
            env.insert(k.clone(), v.clone());
        }
        agent_hooks::inject_hook_env_into(&mut env, self.hook_port, &self.hook_token);

        let opts = SpawnOptions {
            command: spec.binary.to_string(),
            args: spec.args.clone(),
            cwd: Some(workspace_path.to_string_lossy().to_string()),
            env,
            size,
        };

        let pty_id = self
            .pty
            .spawn(opts, move |bytes| on_output(bytes))
            .map_err(|e| AgentsError::Pty(format!("{e:?}")))?;

        self.update_workspace_pty_id(workspace_id, Some(&pty_id.0.to_string()))?;
        Ok(pty_id)
    }

    /// Stop the running agent. Idempotent — calling on a workspace with
    /// no agent is a no-op.
    pub fn stop(&self, workspace_id: &str) -> Result<(), AgentsError> {
        let mutex = self.fs_lock.lock_for(workspace_id);
        let _guard = mutex.lock();

        let (_path, existing_pty_id) = self.fetch_workspace_meta(workspace_id)?;
        if let Some(id_str) = existing_pty_id {
            if let Ok(id_num) = id_str.parse::<u32>() {
                let _ = self.pty.kill(PtyId(id_num));
            }
        }
        self.update_workspace_pty_id(workspace_id, None)?;
        Ok(())
    }

    /// `(workspace_path, current_pty_id)` for the workspace.
    fn fetch_workspace_meta(
        &self,
        workspace_id: &str,
    ) -> Result<(PathBuf, Option<String>), AgentsError> {
        use rusqlite::OptionalExtension;
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT path, pty_id FROM workspaces WHERE id = ?",
                params![workspace_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (path, pty_id) =
            row.ok_or_else(|| AgentsError::WorkspaceNotFound(workspace_id.to_string()))?;
        Ok((PathBuf::from(path), pty_id))
    }

    fn update_workspace_pty_id(
        &self,
        workspace_id: &str,
        pty_id: Option<&str>,
    ) -> Result<(), AgentsError> {
        let conn = self.db.write()?;
        conn.execute(
            "UPDATE workspaces SET pty_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![pty_id, workspace_id],
        )?;
        Ok(())
    }
}

/// Convenience helper for the renderer-side stream wiring. The
/// `WorkspacesService` reference exists today to keep this struct
/// composable later; v1 only needs `Db + Registry + fs_lock + hook
/// coordinates`.
#[allow(dead_code)]
fn _workspaces_service_anchor(_: &WorkspacesService) {}
