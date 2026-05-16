//! Read / write per-agent MCP config files. JSON for most agents,
//! TOML for Codex. JSONC is treated as JSON; the original Electron
//! implementation preserved comments via `jsonc-parser` when writing,
//! but no agent currently uses `.jsonc`, so we read it as plain JSON.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use crate::mcp::model::{AgentMcpMeta, McpError, RawServerEntry, ServerMap};

pub fn read_servers(meta: &AgentMcpMeta) -> Result<ServerMap, McpError> {
    let content = match fs::read_to_string(&meta.config_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ServerMap::new()),
        Err(e) => {
            return Err(McpError::Io {
                path: meta.config_path.display().to_string(),
                source: e,
            })
        }
    };
    if content.trim().is_empty() {
        return Ok(ServerMap::new());
    }

    let parsed: Value = if meta.is_toml {
        toml_to_json(&content).map_err(|e| McpError::ConfigParse {
            path: meta.config_path.display().to_string(),
            message: e,
        })?
    } else {
        serde_json::from_str(&content).map_err(|e| McpError::ConfigParse {
            path: meta.config_path.display().to_string(),
            message: e.to_string(),
        })?
    };

    Ok(extract_at_path(&parsed, meta.servers_path))
}

pub fn write_servers(meta: &AgentMcpMeta, servers: &ServerMap) -> Result<(), McpError> {
    if let Some(parent) = meta.config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| McpError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    let existing_raw = match fs::read_to_string(&meta.config_path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(McpError::Io {
                path: meta.config_path.display().to_string(),
                source: e,
            })
        }
    };

    let mut existing: Value = match existing_raw.as_deref() {
        Some(raw) if !raw.trim().is_empty() => {
            if meta.is_toml {
                toml_to_json(raw).unwrap_or_else(|_| meta.template.clone())
            } else {
                serde_json::from_str(raw).unwrap_or_else(|_| meta.template.clone())
            }
        }
        _ => meta.template.clone(),
    };

    let servers_value = server_map_to_value(servers);
    set_at_path(&mut existing, meta.servers_path, servers_value);

    let serialized = if meta.is_toml {
        json_to_toml(&existing).map_err(|e| McpError::ConfigParse {
            path: meta.config_path.display().to_string(),
            message: e,
        })?
    } else {
        serde_json::to_string_pretty(&existing).map_err(|e| McpError::ConfigParse {
            path: meta.config_path.display().to_string(),
            message: e.to_string(),
        })?
    };

    fs::write(&meta.config_path, serialized).map_err(|e| McpError::Io {
        path: meta.config_path.display().to_string(),
        source: e,
    })
}

fn extract_at_path(root: &Value, segments: &[&str]) -> ServerMap {
    let mut current = root;
    for seg in segments {
        match current.get(*seg) {
            Some(v) => current = v,
            None => return ServerMap::new(),
        }
    }
    let Some(obj) = current.as_object() else {
        return ServerMap::new();
    };
    let mut out = ServerMap::new();
    for (k, v) in obj {
        if let Some(o) = v.as_object() {
            out.insert(k.clone(), o.clone());
        }
    }
    out
}

fn set_at_path(root: &mut Value, segments: &[&str], value: Value) {
    if segments.is_empty() {
        *root = value;
        return;
    }
    let mut current = root;
    for seg in &segments[..segments.len() - 1] {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        let obj = current.as_object_mut().expect("ensured object above");
        if !obj.get(*seg).map(|v| v.is_object()).unwrap_or(false) {
            obj.insert((*seg).to_string(), Value::Object(Map::new()));
        }
        current = obj.get_mut(*seg).expect("just inserted");
    }
    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    let obj = current.as_object_mut().expect("ensured object above");
    obj.insert(segments[segments.len() - 1].to_string(), value);
}

fn server_map_to_value(servers: &ServerMap) -> Value {
    let mut map = Map::new();
    for (k, raw) in servers {
        map.insert(k.clone(), Value::Object(raw.clone()));
    }
    Value::Object(map)
}

fn toml_to_json(s: &str) -> Result<Value, String> {
    let parsed: toml::Value = toml::from_str(s).map_err(|e| e.to_string())?;
    toml_value_to_json(parsed).ok_or_else(|| "non-object root".to_string())
}

fn toml_value_to_json(v: toml::Value) -> Option<Value> {
    Some(match v {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null)
        }
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(arr) => Value::Array(
            arr.into_iter()
                .filter_map(toml_value_to_json)
                .collect(),
        ),
        toml::Value::Table(t) => {
            let mut map = Map::new();
            for (k, v) in t {
                if let Some(child) = toml_value_to_json(v) {
                    map.insert(k, child);
                }
            }
            Value::Object(map)
        }
    })
}

fn json_to_toml(v: &Value) -> Result<String, String> {
    let toml_value = json_to_toml_value(v.clone())
        .ok_or_else(|| "cannot serialize JSON to TOML".to_string())?;
    toml::to_string_pretty(&toml_value).map_err(|e| e.to_string())
}

fn json_to_toml_value(v: Value) -> Option<toml::Value> {
    Some(match v {
        Value::Null => return None,
        Value::Bool(b) => toml::Value::Boolean(b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                return None;
            }
        }
        Value::String(s) => toml::Value::String(s),
        Value::Array(arr) => toml::Value::Array(arr.into_iter().filter_map(json_to_toml_value).collect()),
        Value::Object(obj) => {
            let mut table = toml::map::Map::new();
            for (k, v) in obj {
                if let Some(child) = json_to_toml_value(v) {
                    table.insert(k, child);
                }
            }
            toml::Value::Table(table)
        }
    })
}

#[allow(dead_code)]
pub fn raw_to_value(raw: &RawServerEntry) -> Value {
    Value::Object(raw.clone())
}

#[allow(dead_code)]
pub fn config_path_display(meta: &AgentMcpMeta) -> String {
    Path::new(&meta.config_path).display().to_string()
}
