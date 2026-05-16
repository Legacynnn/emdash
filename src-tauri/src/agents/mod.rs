//! Local agent invocation (EMD-27).
//!
//! Where the three foundational primitives meet:
//! - **Workspace** (EMD-17): the workspace's on-disk path (worktree or
//!   project dir), providing the working directory for the agent.
//! - **PTY** (EMD-8): the streaming primitive carrying agent output
//!   to the renderer's xterm.js.
//! - **Hook server** (EMD-9): env vars injected at spawn time so the
//!   agent can POST events back to `127.0.0.1:<port>/hook`.
//!
//! Tauri-runtime-free domain logic; the Tauri glue lives in
//! `commands::agents`.

pub mod provider;
pub mod service;

pub use provider::{provider_spec, AgentProvider, ProviderSpec};
pub use service::{AgentService, AgentsError};
