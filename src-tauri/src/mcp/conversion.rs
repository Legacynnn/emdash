//! `McpServer` ↔ raw entry conversion. Mirror of the Electron-era
//! `src/main/core/mcp/utils/conversion.ts`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::mcp::model::{McpServer, McpTransport, RawServerEntry};

pub fn raw_to_mcp_server(name: &str, raw: &RawServerEntry, providers: Vec<String>) -> McpServer {
    let typ = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let has_url = raw.contains_key("url");
    let has_command = raw.contains_key("command");
    let is_http = typ == "http" || (has_url && !has_command);

    let command = raw
        .get("command")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let args = raw.get("args").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
    });

    let url = raw.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());

    let headers = raw.get("headers").and_then(|v| value_to_string_map(v));
    let env = raw.get("env").and_then(|v| value_to_string_map(v));

    McpServer {
        name: name.to_string(),
        transport: if is_http {
            McpTransport::Http
        } else {
            McpTransport::Stdio
        },
        command,
        args,
        url,
        headers,
        env,
        providers,
    }
}

pub fn mcp_server_to_raw(server: &McpServer) -> RawServerEntry {
    let mut raw = RawServerEntry::new();
    match server.transport {
        McpTransport::Http => {
            raw.insert("type".to_string(), Value::String("http".to_string()));
            if let Some(url) = &server.url {
                raw.insert("url".to_string(), Value::String(url.clone()));
            }
            if let Some(headers) = &server.headers {
                if !headers.is_empty() {
                    raw.insert("headers".to_string(), string_map_to_value(headers));
                }
            }
        }
        McpTransport::Stdio => {
            if let Some(command) = &server.command {
                raw.insert("command".to_string(), Value::String(command.clone()));
            }
            if let Some(args) = &server.args {
                if !args.is_empty() {
                    raw.insert(
                        "args".to_string(),
                        Value::Array(
                            args.iter()
                                .map(|s| Value::String(s.clone()))
                                .collect(),
                        ),
                    );
                }
            }
        }
    }
    if let Some(env) = &server.env {
        if !env.is_empty() {
            raw.insert("env".to_string(), string_map_to_value(env));
        }
    }
    raw
}

pub fn raw_entry_field_count(server: &McpServer) -> usize {
    let mut c = 0;
    if server.command.is_some() {
        c += 1;
    }
    if server.args.as_ref().map(|a| !a.is_empty()).unwrap_or(false) {
        c += 1;
    }
    if server.url.is_some() {
        c += 1;
    }
    if server.headers.is_some() {
        c += 1;
    }
    if server.env.is_some() {
        c += 1;
    }
    c
}

fn value_to_string_map(v: &Value) -> Option<BTreeMap<String, String>> {
    let obj = v.as_object()?;
    let mut out = BTreeMap::new();
    for (k, val) in obj {
        if let Some(s) = val.as_str() {
            out.insert(k.clone(), s.to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn string_map_to_value(map: &BTreeMap<String, String>) -> Value {
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k.clone(), Value::String(v.clone()));
    }
    Value::Object(obj)
}
