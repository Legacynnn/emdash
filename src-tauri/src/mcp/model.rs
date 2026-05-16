//! MCP domain types. Wire shape matches `@shared/mcp/types`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub type RawServerEntry = serde_json::Map<String, serde_json::Value>;
pub type ServerMap = BTreeMap<String, RawServerEntry>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    pub providers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CredentialKey {
    pub key: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpCatalogEntry {
    pub key: String,
    pub name: String,
    pub description: String,
    pub docs_url: String,
    /// JSON-encoded raw server config. The renderer parses this with
    /// `JSON.parse` to recover a `Record<string, unknown>`. Stored as
    /// a string so specta doesn't have to express `serde_json::Value`
    /// (which trips its BigInt guard on `Number(i64)`).
    pub default_config: String,
    pub credential_keys: Vec<CredentialKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpLoadAllResponse {
    pub installed: Vec<McpServer>,
    pub catalog: Vec<McpCatalogEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpProviderInfo {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub supports_http: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum AdapterType {
    Passthrough,
    Gemini,
    Cursor,
    Codex,
    Opencode,
    Copilot,
}

#[derive(Clone, Debug)]
pub struct AgentMcpMeta {
    pub agent_id: &'static str,
    pub display_name: &'static str,
    pub config_path: PathBuf,
    pub servers_path: &'static [&'static str],
    pub template: serde_json::Value,
    pub is_toml: bool,
    pub adapter: AdapterType,
    pub supports_http: bool,
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error("invalid server name: {0}")]
    InvalidName(String),
    #[error("io error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("catalog parse error: {0}")]
    CatalogParse(String),
    #[error("config parse error in {path}: {message}")]
    ConfigParse { path: String, message: String },
    #[error("partial write failure: {0}")]
    PartialWrite(String),
}
