//! Scans the filesystem for installed skills and detects which target
//! agents have their config directories present. Mirrors the
//! Electron-era `SkillsService.getInstalledSkills` and
//! `SkillsService.getDetectedAgents`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::skills::agent_targets::{agent_targets, skill_scan_paths, skills_root};
use crate::skills::frontmatter::parse as parse_frontmatter;
use crate::skills::model::{CatalogSkill, DetectedAgent, SkillSource};

pub fn detected_agents() -> Vec<DetectedAgent> {
    agent_targets()
        .into_iter()
        .map(|target| {
            let config_dir = target.config_dir.clone();
            let installed = config_dir.exists();
            DetectedAgent {
                id: target.id.to_string(),
                name: target.name.to_string(),
                config_dir: config_dir.to_string_lossy().to_string(),
                installed,
            }
        })
        .collect()
}

/// Returns one `CatalogSkill` per installed skill on disk. The
/// `source` is always `Local` here; callers merge this with the
/// bundled catalog to override the `installed` flag.
pub fn installed_skills() -> Vec<CatalogSkill> {
    let mut dirs: Vec<PathBuf> = vec![skills_root()];
    dirs.extend(skill_scan_paths());

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<CatalogSkill> = Vec::new();

    for dir in dirs {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if seen.contains(&name) {
                continue;
            }
            let resolved = match resolve_skill_dir(&entry.path()) {
                Some(p) => p,
                None => continue,
            };
            let skill_md = resolved.join("SKILL.md");
            let content = match fs::read_to_string(&skill_md) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let (fm, _) = parse_frontmatter(&content);
            seen.insert(name.clone());
            out.push(CatalogSkill {
                id: name.clone(),
                display_name: if fm.name.is_empty() {
                    name.clone()
                } else {
                    fm.name.clone()
                },
                description: fm.description.clone(),
                source: SkillSource::Local,
                source_url: None,
                icon_url: None,
                brand_color: None,
                default_prompt: None,
                skill_md_content: Some(content),
                frontmatter: fm,
                installed: true,
                local_path: Some(resolved.to_string_lossy().to_string()),
            });
        }
    }
    out
}

fn resolve_skill_dir(path: &Path) -> Option<PathBuf> {
    let metadata = fs::symlink_metadata(path).ok()?;
    let file_type = metadata.file_type();
    if !(file_type.is_dir() || file_type.is_symlink()) {
        return None;
    }
    let real = fs::canonicalize(path).ok()?;
    let real_meta = fs::metadata(&real).ok()?;
    if !real_meta.is_dir() {
        return None;
    }
    Some(real)
}
