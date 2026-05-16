//! High-level skills orchestration: bundled catalog merge, install,
//! uninstall, create. Tauri-runtime-free; the glue layer broadcasts
//! UiMutationEvents after each successful write.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;

use crate::skills::agent_targets::{agent_targets, skill_scan_paths, skills_root};
use crate::skills::frontmatter::{generate as generate_skill_md, is_valid_skill_name, parse as parse_frontmatter};
use crate::skills::model::{CatalogIndex, CatalogSkill, DetectedAgent, SkillsError};
use crate::skills::scanner;

const BUNDLED_CATALOG: &str = include_str!("../../resources/skills-catalog.json");

pub struct SkillsService {
    /// Cached catalog with merged installed-state. `None` means cold.
    cache: RwLock<Option<CatalogIndex>>,
}

impl Default for SkillsService {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsService {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }

    fn load_bundled(&self) -> Result<CatalogIndex, SkillsError> {
        let parsed: CatalogIndex = serde_json::from_str(BUNDLED_CATALOG)?;
        Ok(parsed)
    }

    fn ensure_root(&self) -> Result<(), SkillsError> {
        let root = skills_root();
        fs::create_dir_all(&root)?;
        fs::create_dir_all(root.join(".emdash"))?;
        Ok(())
    }

    fn merge_installed(&self, mut catalog: CatalogIndex) -> CatalogIndex {
        let installed = scanner::installed_skills();
        let by_id: HashMap<String, CatalogSkill> =
            installed.into_iter().map(|s| (s.id.clone(), s)).collect();

        // Override installed-state on bundled entries.
        for entry in &mut catalog.skills {
            if let Some(local) = by_id.get(&entry.id) {
                entry.installed = true;
                entry.local_path = local.local_path.clone();
                // Prefer bundled metadata; only use local frontmatter if
                // the bundled entry lacks a description.
                if entry.description.is_empty() {
                    entry.description = local.description.clone();
                }
            } else {
                entry.installed = false;
                entry.local_path = None;
            }
        }

        // Append local-only skills not in the bundled catalog.
        let known: std::collections::HashSet<String> =
            catalog.skills.iter().map(|s| s.id.clone()).collect();
        for (id, skill) in by_id {
            if !known.contains(&id) {
                catalog.skills.push(skill);
            }
        }
        catalog
    }

    pub fn get_catalog(&self) -> Result<CatalogIndex, SkillsError> {
        if let Some(cached) = self.cache.read().clone() {
            return Ok(self.merge_installed(cached));
        }
        let bundled = self.load_bundled()?;
        *self.cache.write() = Some(bundled.clone());
        Ok(self.merge_installed(bundled))
    }

    pub fn refresh_catalog(&self) -> Result<CatalogIndex, SkillsError> {
        // The Electron impl also fetched live OpenAI/Anthropic catalogs.
        // For the Rust port we re-read the bundled JSON; live refresh
        // can be wired later through reqwest + tokio.
        let bundled = self.load_bundled()?;
        *self.cache.write() = Some(bundled.clone());
        Ok(self.merge_installed(bundled))
    }

    pub fn get_detail(&self, id: &str) -> Result<Option<CatalogSkill>, SkillsError> {
        let catalog = self.get_catalog()?;
        let Some(mut skill) = catalog.skills.into_iter().find(|s| s.id == id) else {
            return Ok(None);
        };
        if skill.installed {
            if let Some(local) = &skill.local_path {
                let p = PathBuf::from(local).join("SKILL.md");
                if let Ok(content) = fs::read_to_string(&p) {
                    skill.skill_md_content = Some(content);
                }
            }
        }
        Ok(Some(skill))
    }

    pub fn detected_agents(&self) -> Vec<DetectedAgent> {
        scanner::detected_agents()
    }

    pub fn install(&self, id: &str) -> Result<CatalogSkill, SkillsError> {
        self.ensure_root()?;
        let catalog = self.get_catalog()?;
        let skill = catalog
            .skills
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| SkillsError::NotFound(id.to_string()))?;
        if skill.installed {
            return Err(SkillsError::AlreadyInstalled(id.to_string()));
        }
        let root = skills_root();
        let target = root.join(id);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let tmp = root.join(format!("{id}.tmp-{now}"));

        let result: Result<CatalogSkill, SkillsError> = (|| {
            fs::create_dir_all(&tmp)?;
            let content =
                generate_skill_md(&skill.display_name, &skill.description, None);
            fs::write(tmp.join("SKILL.md"), &content)?;
            // Replace any stale destination atomically.
            let _ = fs::remove_dir_all(&target);
            fs::rename(&tmp, &target)?;
            sync_to_agents(id, &target);
            *self.cache.write() = None;
            let (fm, _) = parse_frontmatter(&content);
            Ok(CatalogSkill {
                installed: true,
                local_path: Some(target.to_string_lossy().to_string()),
                skill_md_content: Some(content),
                frontmatter: fm,
                ..skill
            })
        })();

        if result.is_err() {
            let _ = fs::remove_dir_all(&tmp);
            let _ = fs::remove_dir_all(&target);
        }
        result
    }

    pub fn uninstall(&self, id: &str) -> Result<(), SkillsError> {
        let root = skills_root();
        let target = root.join(id);
        unsync_from_agents(id, &root);

        match fs::symlink_metadata(&target) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    fs::remove_file(&target)?;
                } else if meta.is_dir() {
                    fs::remove_dir_all(&target)?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        *self.cache.write() = None;
        Ok(())
    }

    pub fn create(
        &self,
        name: &str,
        description: &str,
        body: Option<&str>,
    ) -> Result<CatalogSkill, SkillsError> {
        if !is_valid_skill_name(name) {
            return Err(SkillsError::InvalidName(name.to_string()));
        }
        self.ensure_root()?;
        let target = skills_root().join(name);
        if target.exists() {
            return Err(SkillsError::AlreadyExists(name.to_string()));
        }
        fs::create_dir_all(&target)?;
        let content = generate_skill_md(name, description, body);
        fs::write(target.join("SKILL.md"), &content)?;
        sync_to_agents(name, &target);
        *self.cache.write() = None;
        let (fm, _) = parse_frontmatter(&content);
        Ok(CatalogSkill {
            id: name.to_string(),
            display_name: name.to_string(),
            description: description.to_string(),
            source: crate::skills::model::SkillSource::Local,
            source_url: None,
            icon_url: None,
            brand_color: None,
            default_prompt: None,
            skill_md_content: Some(content),
            frontmatter: fm,
            installed: true,
            local_path: Some(target.to_string_lossy().to_string()),
        })
    }
}

fn sync_to_agents(skill_id: &str, source: &Path) {
    for target in agent_targets() {
        if !target.config_dir.exists() {
            continue;
        }
        let dest = target.skill_dir(skill_id);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Replace any existing entry; never write into agent dirs if
        // the parent doesn't exist (agent not installed).
        let _ = remove_skill_link(&dest);
        let _ = make_symlink(source, &dest);
    }
}

fn unsync_from_agents(skill_id: &str, skills_root: &Path) {
    let mut paths: Vec<PathBuf> = agent_targets()
        .iter()
        .map(|t| t.skill_dir(skill_id))
        .collect();
    paths.extend(skill_scan_paths().into_iter().map(|p| p.join(skill_id)));
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for dest in paths {
        if !seen.insert(dest.clone()) {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&dest) else {
            continue;
        };
        if !meta.file_type().is_symlink() {
            // Never rm -rf a real directory in an agent config dir.
            continue;
        }
        // Only unlink if it points into our skills root.
        let Ok(target) = fs::read_link(&dest) else {
            continue;
        };
        let abs = if target.is_absolute() {
            target
        } else {
            dest.parent().unwrap_or(Path::new("")).join(target)
        };
        let canonical = abs.canonicalize().unwrap_or(abs);
        if canonical.starts_with(skills_root) {
            let _ = fs::remove_file(&dest);
        }
    }
}

#[cfg(unix)]
fn make_symlink(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, dest)
}

#[cfg(windows)]
fn make_symlink(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(source, dest)
}

fn remove_skill_link(dest: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(dest) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                fs::remove_file(dest)
            } else if meta.is_dir() {
                fs::remove_dir_all(dest)
            } else {
                fs::remove_file(dest)
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
