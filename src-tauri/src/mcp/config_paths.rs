//! Per-agent MCP config metadata. Mirrors the Electron-era
//! `src/shared/mcp/catalog.ts` (the `AGENT_CONFIGS` map).

use std::path::PathBuf;

use serde_json::json;

use crate::mcp::model::{AdapterType, AgentMcpMeta};

struct AgentDef {
    id: &'static str,
    display_name: &'static str,
    path_segments: &'static [&'static str],
    servers_path: &'static [&'static str],
    template: fn() -> serde_json::Value,
    is_toml: bool,
    adapter: AdapterType,
    supports_http: bool,
}

const AGENTS: &[AgentDef] = &[
    AgentDef {
        id: "claude",
        display_name: "Claude Code",
        path_segments: &[".claude.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Passthrough,
        supports_http: true,
    },
    AgentDef {
        id: "cursor",
        display_name: "Cursor",
        path_segments: &[".cursor", "mcp.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Cursor,
        supports_http: true,
    },
    AgentDef {
        id: "codex",
        display_name: "Codex",
        path_segments: &[".codex", "config.toml"],
        servers_path: &["mcp_servers"],
        template: || json!({ "mcp_servers": {} }),
        is_toml: true,
        adapter: AdapterType::Codex,
        supports_http: false,
    },
    AgentDef {
        id: "amp",
        display_name: "Amp",
        path_segments: &[".config", "amp", "settings.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Passthrough,
        supports_http: true,
    },
    AgentDef {
        id: "gemini",
        display_name: "Gemini CLI",
        path_segments: &[".gemini", "settings.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Gemini,
        supports_http: true,
    },
    AgentDef {
        id: "qwen",
        display_name: "Qwen",
        path_segments: &[".qwen", "settings.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Gemini,
        supports_http: true,
    },
    AgentDef {
        id: "opencode",
        display_name: "OpenCode",
        path_segments: &[".config", "opencode", "opencode.json"],
        servers_path: &["mcp"],
        template: || json!({ "mcp": {} }),
        is_toml: false,
        adapter: AdapterType::Opencode,
        supports_http: true,
    },
    AgentDef {
        id: "copilot",
        display_name: "GitHub Copilot",
        path_segments: &[".copilot", "mcp-config.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Copilot,
        supports_http: true,
    },
    AgentDef {
        id: "droid",
        display_name: "Droid",
        path_segments: &[".droid", "settings.json"],
        servers_path: &["mcpServers"],
        template: || json!({ "mcpServers": {} }),
        is_toml: false,
        adapter: AdapterType::Passthrough,
        supports_http: true,
    },
];

fn home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_default()
}

pub fn get_agent_meta(agent_id: &str) -> Option<AgentMcpMeta> {
    let def = AGENTS.iter().find(|d| d.id == agent_id)?;
    let mut p = home();
    for seg in def.path_segments {
        p.push(seg);
    }
    Some(AgentMcpMeta {
        agent_id: def.id,
        display_name: def.display_name,
        config_path: p,
        servers_path: def.servers_path,
        template: (def.template)(),
        is_toml: def.is_toml,
        adapter: def.adapter,
        supports_http: def.supports_http,
    })
}

pub fn all_agent_ids() -> Vec<&'static str> {
    AGENTS.iter().map(|d| d.id).collect()
}

pub fn all_agents() -> Vec<AgentMcpMeta> {
    AGENTS.iter().filter_map(|d| get_agent_meta(d.id)).collect()
}

pub fn agent_supports_http(agent_id: &str) -> bool {
    AGENTS
        .iter()
        .find(|d| d.id == agent_id)
        .map(|d| d.supports_http)
        .unwrap_or(true)
}
