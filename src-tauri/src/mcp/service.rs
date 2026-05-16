//! `McpService` — orchestrates `load_all`, `save_server`,
//! `remove_server`, and `get_providers`. Mirror of
//! `src/main/core/mcp/services/McpService.ts`.

use std::collections::{BTreeMap, HashSet};

use parking_lot::Mutex;

use crate::mcp::adapters::{forward, reverse};
use crate::mcp::catalog;
use crate::mcp::config_io::{read_servers, write_servers};
use crate::mcp::config_paths::{
    agent_supports_http, all_agent_ids, all_agents, get_agent_meta,
};
use crate::mcp::conversion::{mcp_server_to_raw, raw_entry_field_count, raw_to_mcp_server};
use crate::mcp::model::{
    McpError, McpLoadAllResponse, McpProviderInfo, McpServer, ServerMap,
};

/// Holds a single-writer guard so concurrent saves don't race on the
/// agent config files. Synchronous Mutex is fine because every IO
/// step is blocking already.
pub struct McpService {
    write_lock: Mutex<()>,
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

impl McpService {
    pub fn new() -> Self {
        Self {
            write_lock: Mutex::new(()),
        }
    }

    pub fn load_all(&self) -> Result<McpLoadAllResponse, McpError> {
        let _g = self.write_lock.lock();

        let mut by_name: BTreeMap<String, (McpServer, HashSet<String>)> = BTreeMap::new();

        for meta in all_agents() {
            let raw_servers = match read_servers(&meta) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let canonical = reverse(meta.adapter, &raw_servers);
            for (name, raw) in canonical {
                let entry = by_name.entry(name.clone()).or_insert_with(|| {
                    let mut providers = HashSet::new();
                    providers.insert(meta.agent_id.to_string());
                    (
                        raw_to_mcp_server(&name, &raw, vec![meta.agent_id.to_string()]),
                        providers,
                    )
                });
                entry.1.insert(meta.agent_id.to_string());
                let candidate =
                    raw_to_mcp_server(&name, &raw, entry.1.iter().cloned().collect());
                if raw_entry_field_count(&candidate) > raw_entry_field_count(&entry.0) {
                    entry.0 = candidate;
                }
            }
        }

        let mut installed: Vec<McpServer> = by_name
            .into_iter()
            .map(|(_, (mut server, providers))| {
                let mut p: Vec<String> = providers.into_iter().collect();
                p.sort();
                server.providers = p;
                server
            })
            .collect();
        installed.sort_by(|a, b| a.name.cmp(&b.name));

        let catalog = catalog::load()?;
        Ok(McpLoadAllResponse { installed, catalog })
    }

    pub fn save_server(&self, server: &McpServer) -> Result<(), McpError> {
        if !is_valid_server_name(&server.name) {
            return Err(McpError::InvalidName(server.name.clone()));
        }

        let _g = self.write_lock.lock();
        let selected: HashSet<&str> =
            server.providers.iter().map(|s| s.as_str()).collect();
        let raw = mcp_server_to_raw(server);
        let mut single = ServerMap::new();
        single.insert(server.name.clone(), raw);

        let mut failures: Vec<String> = Vec::new();
        for meta in all_agents() {
            let mut existing = match read_servers(&meta) {
                Ok(s) => s,
                Err(_) => ServerMap::new(),
            };

            if selected.contains(meta.agent_id) {
                let adapted = forward(meta.adapter, &single);
                if let Some(entry) = adapted.get(&server.name) {
                    existing.insert(server.name.clone(), entry.clone());
                }
            } else if existing.contains_key(&server.name) {
                existing.remove(&server.name);
            } else {
                continue;
            }

            if write_servers(&meta, &existing).is_err() {
                failures.push(meta.agent_id.to_string());
            }
        }

        if !failures.is_empty() {
            return Err(McpError::PartialWrite(failures.join(", ")));
        }
        Ok(())
    }

    pub fn remove_server(&self, name: &str) -> Result<(), McpError> {
        let _g = self.write_lock.lock();
        let mut failures: Vec<String> = Vec::new();
        for meta in all_agents() {
            let mut existing = match read_servers(&meta) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !existing.contains_key(name) {
                continue;
            }
            existing.remove(name);
            if write_servers(&meta, &existing).is_err() {
                failures.push(meta.agent_id.to_string());
            }
        }
        if !failures.is_empty() {
            return Err(McpError::PartialWrite(failures.join(", ")));
        }
        Ok(())
    }

    pub fn get_providers(&self) -> Vec<McpProviderInfo> {
        all_agent_ids()
            .into_iter()
            .map(|id| {
                let meta = get_agent_meta(id);
                let display = meta.as_ref().map(|m| m.display_name).unwrap_or(id);
                let installed = meta
                    .as_ref()
                    .map(|m| m.config_path.exists())
                    .unwrap_or(false);
                McpProviderInfo {
                    id: id.to_string(),
                    name: display.to_string(),
                    installed,
                    supports_http: agent_supports_http(id),
                }
            })
            .collect()
    }
}

fn is_valid_server_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_server_names() {
        assert!(!is_valid_server_name(""));
        assert!(!is_valid_server_name("has space"));
        assert!(!is_valid_server_name("has/slash"));
        assert!(is_valid_server_name("good_name"));
        assert!(is_valid_server_name("good-name"));
        assert!(is_valid_server_name("good.name"));
        assert!(is_valid_server_name("name123"));
    }

    #[test]
    fn load_all_returns_catalog_at_minimum() {
        let svc = McpService::new();
        let res = svc.load_all().expect("load_all");
        // Catalog always contains the bundled entries; installed may
        // be empty on a clean machine.
        assert!(!res.catalog.is_empty(), "bundled catalog should not be empty");
    }

    #[test]
    fn providers_includes_all_known_agents() {
        let svc = McpService::new();
        let providers = svc.get_providers();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.id == "claude"));
        assert!(providers.iter().any(|p| p.id == "codex"));
    }
}
