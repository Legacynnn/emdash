//! `UiSyncManager` — owns the subscriber table for `UiMutationEvent`.
//!
//! Tauri-runtime-free: subscribers are stored as boxed callbacks, not
//! `tauri::ipc::Channel<T>`. The `commands::ui_sync` glue wraps a Channel
//! into the callback shape at registration time.
//!
//! Concurrency: a `parking_lot::RwLock` protects the subscriber map.
//! Broadcasts take a read lock and fire callbacks while holding it; a
//! callback must therefore not call back into `subscribe`/`unsubscribe`
//! synchronously (the Channel<T> callback used by the glue only enqueues,
//! so this is safe by construction).

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use super::event::UiMutationEvent;

pub type SubscriberCallback = Arc<dyn Fn(UiMutationEvent) + Send + Sync>;

#[derive(Default)]
pub struct UiSyncManager {
    subscribers: RwLock<HashMap<String, SubscriberCallback>>,
}

impl UiSyncManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a subscriber under `sub_id`. Replaces any existing entry
    /// for that id — renderer mounts generate a fresh UUID, so collisions
    /// only happen on hot-reload where overwriting is the correct
    /// behavior.
    pub fn subscribe(&self, sub_id: String, callback: SubscriberCallback) {
        self.subscribers.write().insert(sub_id, callback);
    }

    /// Remove a subscriber. Idempotent — unsubscribing an unknown id is a
    /// no-op, since teardown can race with renderer reloads.
    pub fn unsubscribe(&self, sub_id: &str) {
        self.subscribers.write().remove(sub_id);
    }

    /// Fan an event out to every current subscriber. Cheap clone of the
    /// callback `Arc`s so we don't hold the read lock across user code.
    pub fn broadcast(&self, event: UiMutationEvent) {
        let callbacks: Vec<SubscriberCallback> = {
            let guard = self.subscribers.read();
            guard.values().cloned().collect()
        };
        for cb in callbacks {
            cb(event.clone());
        }
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn count_callback() -> (Arc<AtomicUsize>, SubscriberCallback) {
        let counter = Arc::new(AtomicUsize::new(0));
        let cb_counter = counter.clone();
        let callback: SubscriberCallback = Arc::new(move |_event| {
            cb_counter.fetch_add(1, Ordering::SeqCst);
        });
        (counter, callback)
    }

    #[test]
    fn subscribe_then_broadcast_invokes_callback() {
        let mgr = UiSyncManager::new();
        let (counter, cb) = count_callback();
        mgr.subscribe("s1".into(), cb);

        mgr.broadcast(UiMutationEvent::ProjectCreated { id: "p1".into() });
        mgr.broadcast(UiMutationEvent::ProjectDeleted { id: "p1".into() });

        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert_eq!(mgr.subscriber_count(), 1);
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let mgr = UiSyncManager::new();
        let (counter, cb) = count_callback();
        mgr.subscribe("s1".into(), cb);
        mgr.unsubscribe("s1");

        mgr.broadcast(UiMutationEvent::ProjectCreated { id: "p1".into() });
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        assert_eq!(mgr.subscriber_count(), 0);
    }

    #[test]
    fn unsubscribe_unknown_id_is_no_op() {
        let mgr = UiSyncManager::new();
        mgr.unsubscribe("never-registered");
        assert_eq!(mgr.subscriber_count(), 0);
    }

    #[test]
    fn subscribe_same_id_replaces_callback() {
        let mgr = UiSyncManager::new();
        let (counter_a, cb_a) = count_callback();
        let (counter_b, cb_b) = count_callback();
        mgr.subscribe("s1".into(), cb_a);
        mgr.subscribe("s1".into(), cb_b);

        mgr.broadcast(UiMutationEvent::ProjectCreated { id: "p1".into() });
        assert_eq!(counter_a.load(Ordering::SeqCst), 0);
        assert_eq!(counter_b.load(Ordering::SeqCst), 1);
        assert_eq!(mgr.subscriber_count(), 1);
    }

    #[test]
    fn fanout_delivers_to_every_subscriber() {
        let mgr = UiSyncManager::new();
        let events: Arc<Mutex<Vec<UiMutationEvent>>> = Arc::new(Mutex::new(Vec::new()));
        for id in ["a", "b", "c"] {
            let sink = events.clone();
            let cb: SubscriberCallback = Arc::new(move |e| sink.lock().unwrap().push(e));
            mgr.subscribe(id.into(), cb);
        }

        mgr.broadcast(UiMutationEvent::ProjectUpdated { id: "p1".into() });
        assert_eq!(events.lock().unwrap().len(), 3);
    }
}
