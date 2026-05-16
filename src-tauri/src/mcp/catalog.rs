//! Bundled MCP catalog (Playwright, Context7, Supabase, …). The JSON
//! is embedded at compile time from `resources/mcp-catalog.json`.

use std::collections::BTreeMap;

use crate::mcp::model::{CredentialKey, McpCatalogEntry, McpError};

const BUNDLED: &str = include_str!("../../resources/mcp-catalog.json");

#[derive(Debug, serde::Deserialize)]
struct RawCatalogEntry {
    config: serde_json::Value,
    name: String,
    description: String,
    #[serde(rename = "docsUrl")]
    docs_url: String,
    #[serde(rename = "credentialKeys")]
    credential_keys: Vec<RawCredentialKey>,
}

#[derive(Debug, serde::Deserialize)]
struct RawCredentialKey {
    key: String,
    required: bool,
}

pub fn load() -> Result<Vec<McpCatalogEntry>, McpError> {
    let parsed: BTreeMap<String, RawCatalogEntry> = serde_json::from_str(BUNDLED)
        .map_err(|e| McpError::CatalogParse(e.to_string()))?;
    let mut out: Vec<McpCatalogEntry> = parsed
        .into_iter()
        .map(|(key, entry)| {
            let default_config = serde_json::to_string(&entry.config).unwrap_or_else(|_| "{}".into());
            McpCatalogEntry {
                key,
                name: entry.name,
                description: entry.description,
                docs_url: entry.docs_url,
                default_config,
                credential_keys: entry
                    .credential_keys
                    .into_iter()
                    .map(|c| CredentialKey {
                        key: c.key,
                        required: c.required,
                    })
                    .collect(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}
