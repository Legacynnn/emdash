//! Application menu builder. Lives at the `app.rs` boundary because
//! the menu API is part of the Tauri runtime — the typed menu items
//! reference `AppHandle`, so this can't sit under `lib.rs` without
//! breaking the domain/glue boundary tests.
//!
//! macOS-native roles are used where Tauri 2's `PredefinedMenuItem`
//! supports them. Roles that aren't native (Services submenu, Speech
//! submenu) are documented in ADR-0009 as known gaps rather than
//! patched over with custom implementations.

use tauri::menu::{Menu, MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Runtime};

/// Build the application menu. Returns a `Menu` ready to attach via
/// `app.set_menu(menu)` during setup.
pub fn build<R: Runtime>(handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    // emdash submenu (macOS app menu)
    let app_menu = SubmenuBuilder::new(handle, "emdash - dev")
        .item(&PredefinedMenuItem::about(
            handle,
            Some("About emdash - dev"),
            None,
        )?)
        .separator()
        .item(&PredefinedMenuItem::hide(
            handle,
            Some("Hide emdash - dev"),
        )?)
        .item(&PredefinedMenuItem::hide_others(
            handle,
            Some("Hide Others"),
        )?)
        .item(&PredefinedMenuItem::show_all(handle, Some("Show All"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(
            handle,
            Some("Quit emdash - dev"),
        )?)
        .build()?;

    // File
    let file_menu = SubmenuBuilder::new(handle, "File")
        .item(&PredefinedMenuItem::close_window(
            handle,
            Some("Close Window"),
        )?)
        .build()?;

    // Edit
    let edit_menu = SubmenuBuilder::new(handle, "Edit")
        .item(&PredefinedMenuItem::undo(handle, Some("Undo"))?)
        .item(&PredefinedMenuItem::redo(handle, Some("Redo"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(handle, Some("Cut"))?)
        .item(&PredefinedMenuItem::copy(handle, Some("Copy"))?)
        .item(&PredefinedMenuItem::paste(handle, Some("Paste"))?)
        .item(&PredefinedMenuItem::select_all(handle, Some("Select All"))?)
        .build()?;

    // View — fullscreen + dev tooling (dev only; release builds drop the dev item)
    let view_builder = SubmenuBuilder::new(handle, "View").item(&PredefinedMenuItem::fullscreen(
        handle,
        Some("Toggle Full Screen"),
    )?);
    let view_menu = view_builder.build()?;

    // Window — minimize, zoom, close
    let window_menu = SubmenuBuilder::new(handle, "Window")
        .item(&PredefinedMenuItem::minimize(handle, Some("Minimize"))?)
        .item(&PredefinedMenuItem::maximize(handle, Some("Zoom"))?)
        .separator()
        .item(&PredefinedMenuItem::close_window(
            handle,
            Some("Close Window"),
        )?)
        .build()?;

    // Help (placeholder — populated in a follow-up once we have docs URLs).
    let help_menu = SubmenuBuilder::new(handle, "Help").build()?;

    MenuBuilder::new(handle)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}
