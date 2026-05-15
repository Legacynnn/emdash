//! Agent-hook subsystem (EMD-9).
//!
//! Local HTTP server that the configured agents POST events to.
//! Token-validated, routed by the body's `agent` field through a
//! `ClassifierRegistry`, classified into a typed `AgentEvent`, and
//! broadcast through the `UiMutationEvent::AgentHookEvent` variant.
//!
//! Domain side stays Tauri-runtime-free; the glue in
//! `commands::agent_hooks` is a thin Builder hook that wires the
//! server's broadcast channel into `UiSyncManager`.

pub mod classifier;
pub mod env;
pub mod event;
pub mod registry;
pub mod server;

pub use classifier::{ClassificationResult, Classifier};
pub use env::inject_hook_env_into;
pub use event::{AgentEvent, AgentEventKind};
pub use registry::ClassifierRegistry;
pub use server::{HookServer, HookServerError, HookServerHandle};
