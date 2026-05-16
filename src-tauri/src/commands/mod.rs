// Tauri command glue — thin wrappers over `crate::*` domain modules.

pub mod agents;
pub mod app;
pub mod conversations;
pub mod editor_buffers;
pub mod fs;
pub mod fs_watcher;
pub mod git;
pub mod github;
pub mod greet;
pub mod linear;
pub mod path;
pub mod dependencies;
pub mod projects;
pub mod pty;
pub mod resource_monitor;
pub mod search;
pub mod secrets;
pub mod tasks;
pub mod telemetry;
pub mod terminals;
pub mod ui_sync;
pub mod updater;
pub mod view_state;
