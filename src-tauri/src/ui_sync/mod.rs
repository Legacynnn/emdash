//! Renderer state-sync spine.
//!
//! Domain side of the `UiMutationEvent` bridge described in EMD-7 / ADR-0004.
//! This module stays Tauri-runtime-free (enforced by
//! `tests/domain_boundaries.rs`); the Tauri glue in `commands::ui_sync`
//! adapts `Channel<UiMutationEvent>` to the [`SubscriberCallback`] shape
//! used here.
//!
//! Discipline contract: every domain mutation that should refresh renderer
//! state calls [`UiSyncManager::broadcast`]. Renderer code never listens to
//! ad-hoc `app.emit(...)` events — the ESLint rule in `ui/eslint.config.ts`
//! enforces that at lint time.

pub mod event;
pub mod manager;

pub use event::UiMutationEvent;
pub use manager::{SubscriberCallback, UiSyncManager};
