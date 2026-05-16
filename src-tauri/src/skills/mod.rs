//! Skills domain. Bundled catalog of installable agent skills plus
//! on-disk install / uninstall / scan logic. Tauri-runtime-free; the
//! glue layer is in `commands::skills`.

pub mod agent_targets;
pub mod frontmatter;
pub mod model;
pub mod scanner;
pub mod service;

pub use model::{
    CatalogIndex, CatalogSkill, DetectedAgent, SkillFrontmatter, SkillSource, SkillsError,
};
pub use service::SkillsService;
