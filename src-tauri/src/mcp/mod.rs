//! MCP (Model Context Protocol) domain. Reads / writes per-agent
//! MCP config files (Claude, Cursor, Codex, OpenCode, Copilot, etc.)
//! and merges the entries into a canonical `McpServer` shape the
//! renderer consumes. Tauri-runtime-free; glue lives in
//! `commands::mcp`.

pub mod adapters;
pub mod catalog;
pub mod config_io;
pub mod config_paths;
pub mod conversion;
pub mod model;
pub mod service;

pub use model::{
    AdapterType, AgentMcpMeta, CredentialKey, McpCatalogEntry, McpError, McpLoadAllResponse,
    McpProviderInfo, McpServer, McpTransport,
};
pub use service::McpService;
