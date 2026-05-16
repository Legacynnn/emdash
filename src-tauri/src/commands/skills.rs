//! Skill catalog. The renderer asks for an installable-skill catalog
//! plus per-skill detail. Host returns an empty catalog until a
//! real skill registry is wired (the renderer surfaces an empty
//! state cleanly, so this is the "no skills published yet" path).

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed: bool,
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_catalog() -> Vec<Skill> {
    Vec::new()
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_detail(_id: String) -> Option<Skill> {
    None
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_detected_agents() -> Vec<String> {
    Vec::new()
}
