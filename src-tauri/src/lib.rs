//! emdash-dev — Tauri 2 + Rust rewrite of emdash.
//!
//! Modules split into `DOMAIN_MODULES` (tauri-runtime-free) and
//! `TAURI_GLUE_MODULES` (intentionally tauri-aware), enforced by
//! `tests/domain_boundaries.rs`. See ADR-0001 for the rationale.

pub mod agent_hooks;
pub mod bindings_parser;
pub mod claude_trust;
pub mod commands;
pub mod db;
pub mod editor_buffers;
pub mod fs_watcher;
pub mod git;
pub mod greeting;
pub mod hook_config;
pub mod projects;
pub mod providers;
pub mod pty;
pub mod secrets;
pub mod shell_env;
pub mod ssh;
pub mod tasks;
pub mod tauri_bindings;
pub mod telemetry;
pub mod ui_sync;
pub mod updater;
pub mod view_state;
