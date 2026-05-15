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
        commands::editor_buffers::editor_buffer_save,
        commands::editor_buffers::editor_buffer_clear,
        commands::editor_buffers::editor_buffer_list,
        commands::view_state::view_state_save,
        commands::view_state::view_state_get,
        commands::view_state::view_state_get_all,
        commands::view_state::view_state_delete,
        commands::view_state::view_state_reset,
    ])
}

pub const BINDINGS_PATH: &str = "ui/src/bindings.ts";

pub fn export_bindings_to<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn std::error::Error>> {
    build_specta().export(Typescript::default(), path)?;
    Ok(())
}
