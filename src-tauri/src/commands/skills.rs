//! Skills Tauri glue. Wraps `SkillsService` and maps `SkillsError`
//! onto the serializable envelope the renderer expects.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::skills::{
    CatalogIndex, CatalogSkill, DetectedAgent, SkillsError, SkillsService,
};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct SkillsCommandError {
    pub code: SkillsErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SkillsErrorCode {
    NotFound,
    AlreadyInstalled,
    AlreadyExists,
    InvalidName,
    Io,
    Catalog,
}

impl From<SkillsError> for SkillsCommandError {
    fn from(e: SkillsError) -> Self {
        let message = e.to_string();
        let code = match e {
            SkillsError::NotFound(_) => SkillsErrorCode::NotFound,
            SkillsError::AlreadyInstalled(_) => SkillsErrorCode::AlreadyInstalled,
            SkillsError::AlreadyExists(_) => SkillsErrorCode::AlreadyExists,
            SkillsError::InvalidName(_) => SkillsErrorCode::InvalidName,
            SkillsError::Io(_) => SkillsErrorCode::Io,
            SkillsError::CatalogParse(_) => SkillsErrorCode::Catalog,
        };
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_catalog(
    service: State<'_, Arc<SkillsService>>,
) -> Result<CatalogIndex, SkillsCommandError> {
    service.get_catalog().map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn skills_refresh_catalog(
    service: State<'_, Arc<SkillsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
) -> Result<CatalogIndex, SkillsCommandError> {
    let catalog = service.refresh_catalog()?;
    manager.broadcast(UiMutationEvent::SkillsCatalogRefreshed);
    Ok(catalog)
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_detail(
    service: State<'_, Arc<SkillsService>>,
    id: String,
) -> Result<Option<CatalogSkill>, SkillsCommandError> {
    service.get_detail(&id).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn skills_get_detected_agents(
    service: State<'_, Arc<SkillsService>>,
) -> Vec<DetectedAgent> {
    service.detected_agents()
}

#[tauri::command]
#[specta::specta]
pub fn skills_install(
    service: State<'_, Arc<SkillsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    skill_id: String,
) -> Result<CatalogSkill, SkillsCommandError> {
    let skill = service.install(&skill_id)?;
    manager.broadcast(UiMutationEvent::SkillInstalled {
        id: skill_id.clone(),
    });
    Ok(skill)
}

#[tauri::command]
#[specta::specta]
pub fn skills_uninstall(
    service: State<'_, Arc<SkillsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    skill_id: String,
) -> Result<(), SkillsCommandError> {
    service.uninstall(&skill_id)?;
    manager.broadcast(UiMutationEvent::SkillUninstalled {
        id: skill_id.clone(),
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn skills_create(
    service: State<'_, Arc<SkillsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    name: String,
    description: String,
    content: Option<String>,
) -> Result<CatalogSkill, SkillsCommandError> {
    let skill = service.create(&name, &description, content.as_deref())?;
    manager.broadcast(UiMutationEvent::SkillInstalled { id: name });
    Ok(skill)
}
