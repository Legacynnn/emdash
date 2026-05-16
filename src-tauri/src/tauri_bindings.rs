//! Specta builder + TypeScript binding exporter. Lives in the lib so
//! `tests/wire_format.rs` can regenerate-and-diff in-memory without `cargo run`.

use std::path::Path;

use specta_typescript::Typescript;
use tauri_specta::{collect_commands, Builder};

use crate::commands;

/// Single source of truth for the command set. Used by `app::run` and
/// `export_bindings_to`.
pub fn build_specta() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::greet::greet,
        commands::path::get_path,
        commands::secrets::set_secret,
        commands::secrets::get_secret,
        commands::pty::pty_spawn,
        commands::pty::pty_write,
        commands::pty::pty_resize,
        commands::pty::pty_kill,
        commands::ui_sync::subscribe_ui_mutations,
        commands::ui_sync::unsubscribe_ui_mutations,
        commands::projects::projects_list,
        commands::projects::projects_add,
        commands::projects::projects_remove,
        commands::fs_watcher::fs_watcher_subscribe,
        commands::fs_watcher::fs_watcher_unsubscribe,
        commands::tasks::tasks_list,
        commands::tasks::tasks_create,
        commands::tasks::tasks_delete,
        commands::telemetry::telemetry_get_enabled,
        commands::telemetry::telemetry_set_enabled,
        commands::telemetry::telemetry_record,
        commands::updater::subscribe_updater_events,
        commands::updater::updater_check,
        commands::updater::updater_simulate_event,
        commands::editor_buffers::editor_buffer_save,
        commands::editor_buffers::editor_buffer_clear,
        commands::editor_buffers::editor_buffer_list,
        commands::view_state::view_state_save,
        commands::view_state::view_state_get,
        commands::view_state::view_state_get_all,
        commands::view_state::view_state_delete,
        commands::view_state::view_state_reset,
        commands::github::github_sign_in_device_flow_start,
        commands::github::github_sign_in_device_flow_poll,
        commands::github::github_sign_in_via_gh_cli,
        commands::github::github_sign_out,
        commands::github::github_me,
        commands::github::github_list_repos,
        commands::github::github_list_pulls,
        commands::github::github_get_pull,
        commands::github::github_get_pull_diff,
        commands::linear::linear_sign_in,
        commands::linear::linear_sign_out,
        commands::linear::linear_me,
        commands::linear::linear_list_teams,
        commands::linear::linear_list_projects,
        commands::linear::linear_list_cycles,
        commands::linear::linear_list_labels,
        commands::linear::linear_list_states,
        commands::linear::linear_list_issues,
        commands::linear::linear_get_issue,
        commands::linear::linear_create_issue,
        commands::linear::linear_update_issue,
        commands::linear::linear_list_comments,
        commands::linear::linear_create_comment,
        commands::agents::agents_start,
        commands::agents::agents_stop,
        commands::agents::agents_list_providers,
        commands::app::app_get_version,
        commands::app::app_get_platform,
        commands::app::app_open_external,
        commands::app::app_open_in,
        commands::app::app_check_installed_apps,
        commands::app::app_list_installed_fonts,
        commands::app::app_open_select_directory_dialog,
        commands::app::app_clipboard_write_text,
        commands::fs::fs_read_file,
        commands::fs::fs_write_file,
        commands::fs::fs_remove_file,
        commands::fs::fs_file_exists,
        commands::fs::fs_stat_file,
        commands::fs::fs_list_files,
        commands::fs::fs_read_image,
        commands::conversations::conversations_list_for_task,
        commands::conversations::conversations_create,
        commands::conversations::conversations_rename,
        commands::conversations::conversations_touch,
        commands::conversations::conversations_delete,
    ])
}

pub const BINDINGS_PATH: &str = "ui/src/bindings.ts";

pub fn export_bindings_to<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn std::error::Error>> {
    build_specta().export(Typescript::default(), path)?;
    Ok(())
}
