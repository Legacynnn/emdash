//! `AgentService` — start/stop an agent for a conversation.
//!
//! Composes:
//! - `conversations` table for the conversation's workspace + `pty_id`
//!   column update on start/stop
//! - `workspaces` table to resolve the conversation's workspace → path
//! - `pty::Registry` to spawn the PTY itself
//! - `shell_env::shell_env()` for the captured login-shell env
//! - `agent_hooks::inject_hook_env_into` to plant the hook URL +
//!   token + conversation id before spawn
//! - `WorkspaceFsMutationLock` (re-used) so concurrent start/stop on
//!   the same conversation serialize cleanly

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
    #[error("conversation not found: {0}")]
    ConversationNotFound(String),
    #[error("workspace not found for conversation: {0}")]
    WorkspaceNotFound(String),
    #[error("conversation already has an agent running (pty {0})")]
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

    /// Start an agent for `conversation_id`. Returns the freshly-spawned
    /// `PtyId`. Side effects:
    /// - `conversations.pty_id` updated to the new PTY id
    /// - PTY output streamed via `on_output`
    /// - hook env vars + `EMDASH_CONVERSATION_ID` injected at spawn time
    pub fn start(
        &self,
        conversation_id: &str,
        provider: AgentProvider,
        size: PtySize,
        on_output: OutputCallback,
    ) -> Result<PtyId, AgentsError> {
        // Serialize start/stop on the same conversation so concurrent
        // requests can't double-spawn.
        let mutex = self.fs_lock.lock_for(conversation_id);
        let _guard = mutex.lock();

        let meta = self.fetch_conversation_meta(conversation_id)?;
        if let Some(existing) = meta.pty_id {
            return Err(AgentsError::AlreadyRunning(existing));
        }

        let spec = provider_spec(provider);
        let mut env: HashMap<String, String> = shell_env::shell_env().captured.clone();
        for (k, v) in spec.env.iter() {
            env.insert(k.clone(), v.clone());
        }
        agent_hooks::inject_hook_env_into(&mut env, self.hook_port, &self.hook_token);
        // Per-conversation routing for hook callbacks. User-configured
        // hook commands (e.g. in `~/.claude/settings.json`) can forward
        // this into the POST body so the hook server attributes the
        // event to the right conversation.
        env.insert(
            "EMDASH_CONVERSATION_ID".to_string(),
            conversation_id.to_string(),
        );
        env.insert(
            "EMDASH_WORKSPACE_ID".to_string(),
            meta.workspace_id.clone(),
        );

        let opts = SpawnOptions {
            command: spec.binary.to_string(),
            args: spec.args.clone(),
            cwd: Some(meta.workspace_path.to_string_lossy().to_string()),
            env,
            size,
        };

        let pty_id = self
            .pty
            .spawn(opts, move |bytes| on_output(bytes))
            .map_err(|e| AgentsError::Pty(format!("{e:?}")))?;

        self.update_conversation_pty_id(conversation_id, Some(&pty_id.0.to_string()))?;
        Ok(pty_id)
    }

    /// Stop the running agent. Idempotent — calling on a conversation
    /// with no agent is a no-op.
    pub fn stop(&self, conversation_id: &str) -> Result<(), AgentsError> {
        let mutex = self.fs_lock.lock_for(conversation_id);
        let _guard = mutex.lock();

        let meta = self.fetch_conversation_meta(conversation_id)?;
        if let Some(id_str) = meta.pty_id {
            if let Ok(id_num) = id_str.parse::<u32>() {
                let _ = self.pty.kill(PtyId(id_num));
            }
        }
        self.update_conversation_pty_id(conversation_id, None)?;
        Ok(())
    }

    fn fetch_conversation_meta(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationMeta, AgentsError> {
        use rusqlite::OptionalExtension;
        let conn = self.db.read()?;
        // One join keeps workspace_id + workspace.path resolution atomic
        // with the conversation lookup.
        let row = conn
            .query_row(
                "SELECT c.workspace_id, w.path, c.pty_id
                 FROM conversations c
                 LEFT JOIN workspaces w ON w.id = c.workspace_id
                 WHERE c.id = ?",
                params![conversation_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let (workspace_id, workspace_path, pty_id) = row
            .ok_or_else(|| AgentsError::ConversationNotFound(conversation_id.to_string()))?;
        let workspace_path = workspace_path
            .ok_or_else(|| AgentsError::WorkspaceNotFound(workspace_id.clone()))?;
        Ok(ConversationMeta {
            workspace_id,
            workspace_path: PathBuf::from(workspace_path),
            pty_id,
        })
    }

    fn update_conversation_pty_id(
        &self,
        conversation_id: &str,
        pty_id: Option<&str>,
    ) -> Result<(), AgentsError> {
        let conn = self.db.write()?;
        conn.execute(
            "UPDATE conversations
             SET pty_id = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            params![pty_id, conversation_id],
        )?;
        Ok(())
    }
}

struct ConversationMeta {
    workspace_id: String,
    workspace_path: PathBuf,
    pty_id: Option<String>,
}

/// Convenience helper for the renderer-side stream wiring. The
/// `WorkspacesService` reference exists today to keep this struct
/// composable later; v1 only needs `Db + Registry + fs_lock + hook
/// coordinates`.
#[allow(dead_code)]
fn _workspaces_service_anchor(_: &WorkspacesService) {}
