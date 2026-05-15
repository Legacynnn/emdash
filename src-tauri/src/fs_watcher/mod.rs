//! File-system watching domain (EMD-11). One `WatcherHandle` per
//! watched path; debounce + coalescing handled by
//! `notify-debouncer-full`. Tauri-runtime-free — the glue in
//! `commands::fs_watcher` adapts to `Channel<WatchEvent>`.
//!
//! See ADR-0020 for the ENOSPC fallback strategy and platform
//! quirks (macOS FSEvents casing, Linux inotify limits).

pub mod event;
pub mod registry;

pub use event::{WatchEvent, WatchEventKind, WatcherError, WatcherFallback};
pub use registry::{WatcherHandle, WatcherRegistry};
