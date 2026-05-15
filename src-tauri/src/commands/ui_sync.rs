//! Tauri glue for the `ui_sync` domain. Adapts `tauri::ipc::Channel<UiMutationEvent>`
//! into the `SubscriberCallback` shape used by the runtime-free
//! `UiSyncManager`. Domain code never touches `Channel<T>` directly.
//!
//! `subscribe_ui_mutations` is the **only** subscription channel for
//! renderer state sync. The ESLint rule in `ui/eslint.config.ts` blocks
//! ad-hoc `listen(...)` / `app.emit(...)` usage so cache-invalidation
//! policy stays in one place.

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::ui_sync::{SubscriberCallback, UiMutationEvent, UiSyncManager};

#[tauri::command]
#[specta::specta]
pub async fn subscribe_ui_mutations(
    manager: State<'_, Arc<UiSyncManager>>,
    sub_id: String,
    on_event: Channel<UiMutationEvent>,
) -> Result<(), ()> {
    let callback: SubscriberCallback = Arc::new(move |event| {
        // Channel::send is fire-and-forget — same loss-tolerance contract as
        // the PTY data channel (see ADR-0003). The renderer reconciles on
        // reconnect via an explicit refresh of the affected store.
        let _ = on_event.send(event);
    });
    manager.subscribe(sub_id, callback);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn unsubscribe_ui_mutations(
    manager: State<'_, Arc<UiSyncManager>>,
    sub_id: String,
) -> Result<(), ()> {
    manager.unsubscribe(&sub_id);
    Ok(())
}
