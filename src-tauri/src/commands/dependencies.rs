//! Dependency-probe commands. The renderer asks the host which agent
//! CLIs are installed on `PATH`. We resolve via `which`-style PATH
//! lookup so this works without any extra dependencies.

use std::path::PathBuf;

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEntry {
    pub id: String,
    pub installed: bool,
    pub resolved_path: Option<String>,
}

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(target_os = "windows")]
        {
            for ext in ["exe", "cmd", "bat"] {
                let with_ext = dir.join(format!("{name}.{ext}"));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

const KNOWN_AGENT_CLIS: &[&str] = &[
    "claude", "codex", "gemini", "amp", "auggie", "cline", "continue", "copilot", "cursor",
    "kiro", "qwen", "kimi", "goose", "opencode", "letta", "mistral", "rovo", "droid", "jules",
    "junie", "pi",
];

#[tauri::command]
#[specta::specta]
pub fn dependencies_get_all() -> Vec<DependencyEntry> {
    KNOWN_AGENT_CLIS
        .iter()
        .map(|id| {
            let path = which(id);
            DependencyEntry {
                id: (*id).into(),
                installed: path.is_some(),
                resolved_path: path.map(|p| p.to_string_lossy().to_string()),
            }
        })
        .collect()
}

#[tauri::command]
#[specta::specta]
pub fn dependencies_probe(id: String) -> DependencyEntry {
    let path = which(&id);
    DependencyEntry {
        id,
        installed: path.is_some(),
        resolved_path: path.map(|p| p.to_string_lossy().to_string()),
    }
}
