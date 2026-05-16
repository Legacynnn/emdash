//! Per-agent destinations for syncing installed skills. Mirrors the
//! Electron-era `src/shared/skills/agentTargets.ts`.

use std::path::PathBuf;

pub struct AgentTarget {
    pub id: &'static str,
    pub name: &'static str,
    pub config_dir: PathBuf,
    pub skill_subdir: &'static [&'static str],
}

impl AgentTarget {
    pub fn skill_dir(&self, skill_id: &str) -> PathBuf {
        let mut p = self.config_dir.clone();
        for seg in self.skill_subdir {
            p.push(seg);
        }
        p.push(skill_id);
        p
    }
}

fn home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_default()
}

pub fn agent_targets() -> Vec<AgentTarget> {
    let home = home();
    vec![
        AgentTarget {
            id: "claude-code",
            name: "Claude Code",
            config_dir: home.join(".claude"),
            skill_subdir: &["commands"],
        },
        AgentTarget {
            id: "codex",
            name: "Codex",
            config_dir: home.join(".codex"),
            skill_subdir: &["skills"],
        },
        AgentTarget {
            id: "opencode",
            name: "OpenCode",
            config_dir: home.join(".config").join("opencode"),
            skill_subdir: &["skills"],
        },
        AgentTarget {
            id: "cursor",
            name: "Cursor",
            config_dir: home.join(".cursor"),
            skill_subdir: &["skills"],
        },
        AgentTarget {
            id: "gemini",
            name: "Gemini CLI",
            config_dir: home.join(".gemini"),
            skill_subdir: &["skills"],
        },
        AgentTarget {
            id: "roo-code",
            name: "Roo Code",
            config_dir: home.join(".roo"),
            skill_subdir: &["skills"],
        },
        AgentTarget {
            id: "mistral-vibe",
            name: "Mistral Vibe",
            config_dir: home.join(".vibe"),
            skill_subdir: &["skills"],
        },
    ]
}

pub fn skills_root() -> PathBuf {
    home().join(".agentskills")
}

pub fn skill_scan_paths() -> Vec<PathBuf> {
    let home = home();
    let mut paths: Vec<PathBuf> = agent_targets()
        .iter()
        .map(|t| {
            let mut p = t.config_dir.clone();
            for seg in t.skill_subdir {
                p.push(seg);
            }
            p
        })
        .collect();
    paths.push(home.join(".claude").join("skills"));
    paths.push(home.join(".agent").join("skills"));
    paths.push(home.join(".agents").join("skills"));
    paths.sort();
    paths.dedup();
    paths
}
