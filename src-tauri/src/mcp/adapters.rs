//! Forward / reverse adapters between the canonical `ServerMap` shape
//! Emdash uses internally and the per-agent on-disk format. Mirrors
//! `src/main/core/mcp/utils/adapters.ts` from the Electron build.

use serde_json::{Map, Value};

use crate::mcp::model::{AdapterType, RawServerEntry, ServerMap};

const INJECTED_ACCEPT: &str = "application/json, text/event-stream";

pub fn forward(adapter: AdapterType, servers: &ServerMap) -> ServerMap {
    match adapter {
        AdapterType::Passthrough => clone_map(servers),
        AdapterType::Gemini => fwd_gemini(servers),
        AdapterType::Cursor => fwd_cursor(servers),
        AdapterType::Codex => fwd_codex(servers),
        AdapterType::Opencode => fwd_opencode(servers),
        AdapterType::Copilot => fwd_copilot(servers),
    }
}

pub fn reverse(adapter: AdapterType, servers: &ServerMap) -> ServerMap {
    match adapter {
        AdapterType::Passthrough => clone_map(servers),
        AdapterType::Gemini => rev_gemini(servers),
        AdapterType::Cursor => rev_cursor(servers),
        AdapterType::Codex => clone_map(servers),
        AdapterType::Opencode => rev_opencode(servers),
        AdapterType::Copilot => rev_copilot(servers),
    }
}

// ── Forward ──────────────────────────────────────────────────────────────

fn fwd_gemini(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        let entry = if is_http(v) {
            let url = v
                .get("url")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let mut headers = headers_clone(v);
            ensure_header(&mut headers, "Accept", INJECTED_ACCEPT);
            let mut e = RawServerEntry::new();
            e.insert("httpUrl".to_string(), Value::String(url));
            e.insert("headers".to_string(), Value::Object(headers));
            if let Some(env) = v.get("env") {
                e.insert("env".to_string(), env.clone());
            }
            e
        } else {
            v.clone()
        };
        out.insert(k.clone(), entry);
    }
    out
}

fn fwd_cursor(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        let entry = if is_http(v) {
            let url = v
                .get("url")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let headers = headers_clone(v);
            let mut e = RawServerEntry::new();
            e.insert("url".to_string(), Value::String(url));
            e.insert("headers".to_string(), Value::Object(headers));
            if let Some(env) = v.get("env") {
                e.insert("env".to_string(), env.clone());
            }
            e
        } else {
            v.clone()
        };
        out.insert(k.clone(), entry);
    }
    out
}

fn fwd_codex(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        if is_stdio(v) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn fwd_opencode(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        if is_http(v) {
            let mut headers = headers_clone(v);
            ensure_header(&mut headers, "Accept", INJECTED_ACCEPT);
            let mut e = RawServerEntry::new();
            e.insert("type".to_string(), Value::String("remote".to_string()));
            e.insert(
                "url".to_string(),
                Value::String(
                    v.get("url")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                ),
            );
            e.insert("headers".to_string(), Value::Object(headers));
            e.insert("enabled".to_string(), Value::Bool(true));
            if let Some(env) = v.get("env") {
                e.insert("env".to_string(), env.clone());
            }
            out.insert(k.clone(), e);
        } else if is_stdio(v) {
            let mut cmd = Vec::new();
            if let Some(c) = v.get("command").and_then(|x| x.as_str()) {
                cmd.push(Value::String(c.to_string()));
            }
            if let Some(args) = v.get("args").and_then(|x| x.as_array()) {
                cmd.extend(args.iter().cloned());
            }
            let mut e = RawServerEntry::new();
            e.insert("type".to_string(), Value::String("local".to_string()));
            e.insert("command".to_string(), Value::Array(cmd));
            e.insert("enabled".to_string(), Value::Bool(true));
            if let Some(env) = v.get("env") {
                e.insert("env".to_string(), env.clone());
            }
            out.insert(k.clone(), e);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn fwd_copilot(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        if !v.contains_key("tools") {
            let mut clone = v.clone();
            clone.insert(
                "tools".to_string(),
                Value::Array(vec![Value::String("*".to_string())]),
            );
            out.insert(k.clone(), clone);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

// ── Reverse ──────────────────────────────────────────────────────────────

fn rev_gemini(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        if v.contains_key("httpUrl") {
            let mut entry = v.clone();
            let url = entry.remove("httpUrl").unwrap_or(Value::Null);
            entry.insert("type".to_string(), Value::String("http".to_string()));
            entry.insert("url".to_string(), url);
            strip_injected_headers(&mut entry);
            out.insert(k.clone(), entry);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn rev_cursor(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        if v.contains_key("url") && !v.contains_key("command") {
            let mut clone = v.clone();
            clone.insert("type".to_string(), Value::String("http".to_string()));
            out.insert(k.clone(), clone);
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn rev_opencode(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        let typ = v.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if typ == "remote" {
            let mut clone = v.clone();
            clone.remove("type");
            clone.remove("enabled");
            clone.insert("type".to_string(), Value::String("http".to_string()));
            strip_injected_headers(&mut clone);
            out.insert(k.clone(), clone);
        } else if typ == "local" {
            if let Some(arr) = v.get("command").and_then(|x| x.as_array()) {
                let mut iter = arr.iter();
                let mut e = RawServerEntry::new();
                if let Some(first) = iter.next().and_then(|x| x.as_str()) {
                    e.insert("command".to_string(), Value::String(first.to_string()));
                }
                let rest: Vec<Value> = iter.cloned().collect();
                if !rest.is_empty() {
                    e.insert("args".to_string(), Value::Array(rest));
                }
                out.insert(k.clone(), e);
            } else {
                out.insert(k.clone(), v.clone());
            }
        } else {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

fn rev_copilot(servers: &ServerMap) -> ServerMap {
    let mut out = ServerMap::new();
    for (k, v) in servers {
        let mut clone = v.clone();
        if let Some(tools) = clone.get("tools").and_then(|x| x.as_array()) {
            if tools.len() == 1 && tools[0].as_str() == Some("*") {
                clone.remove("tools");
            }
        }
        out.insert(k.clone(), clone);
    }
    out
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn is_http(entry: &RawServerEntry) -> bool {
    entry
        .get("type")
        .and_then(|v| v.as_str())
        .map(|t| t == "http")
        .unwrap_or(false)
}

fn is_stdio(entry: &RawServerEntry) -> bool {
    !is_http(entry) && entry.contains_key("command")
}

fn headers_clone(entry: &RawServerEntry) -> Map<String, Value> {
    entry
        .get("headers")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn ensure_header(headers: &mut Map<String, Value>, key: &str, val: &str) {
    headers
        .entry(key.to_string())
        .or_insert_with(|| Value::String(val.to_string()));
}

fn strip_injected_headers(entry: &mut RawServerEntry) {
    let Some(headers) = entry.get_mut("headers").and_then(|v| v.as_object_mut()) else {
        return;
    };
    let drop = headers
        .get("Accept")
        .and_then(|v| v.as_str())
        .map(|s| s == INJECTED_ACCEPT)
        .unwrap_or(false);
    if drop {
        headers.remove("Accept");
        if headers.is_empty() {
            entry.remove("headers");
        }
    }
}

fn clone_map(servers: &ServerMap) -> ServerMap {
    servers.clone()
}
