//! MCP (Model Context Protocol) provider + server registry. Host
//! returns empty registries until a real loader reads the user's
//! MCP config (e.g. `~/.claude/mcp-servers.json`); the renderer's
//! MCP view degrades to an empty list cleanly.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct McpProvider {
    pub id: String,
    pub name: String,
    pub servers: Vec<McpServer>,
}

#[tauri::command]
#[specta::specta]
pub fn mcp_load_all() -> Vec<McpServer> {
    Vec::new()
}

#[tauri::command]
#[specta::specta]
pub fn mcp_get_providers() -> Vec<McpProvider> {
    Vec::new()
}
