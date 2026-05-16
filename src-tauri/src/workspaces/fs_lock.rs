//! `WorkspaceFsMutationLock` — Helmor-style per-workspace mutex.
//!
//! Serializes worktree create/destroy with the matching DB row update
//! so two concurrent `workspaces.create / workspaces.delete` calls
//! targeting the same workspace can't interleave: one acquires the
//! per-workspace `Mutex` and the other waits.
//!
//! Implementation: a `parking_lot::Mutex<HashMap<String, Arc<Mutex<()>>>>`
//! map. Each `lock_for(id)` call returns an `Arc<Mutex<()>>` that the
//! caller `.lock()`s; entries are never evicted (cheap to keep, avoids
//! a TOCTOU between "I asked for the lock and someone else dropped the
//! map entry").

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

#[derive(Default)]
pub struct WorkspaceFsMutationLock {
    map: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl WorkspaceFsMutationLock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the per-workspace `Mutex`. Caller `.lock()`s and holds
    /// the guard across the entire FS+DB transaction.
    pub fn lock_for(&self, workspace_id: &str) -> Arc<Mutex<()>> {
        let mut guard = self.map.lock();
        guard
            .entry(workspace_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn same_id_returns_same_mutex() {
        let locks = WorkspaceFsMutationLock::new();
        let a = locks.lock_for("ws-1");
        let b = locks.lock_for("ws-1");
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn different_ids_return_different_mutexes() {
        let locks = WorkspaceFsMutationLock::new();
        let a = locks.lock_for("ws-1");
        let b = locks.lock_for("ws-2");
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn serializes_same_id_under_contention() {
        let locks = Arc::new(WorkspaceFsMutationLock::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let max_inside = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let locks = locks.clone();
            let counter = counter.clone();
            let max_inside = max_inside.clone();
            handles.push(thread::spawn(move || {
                let m = locks.lock_for("shared");
                let _guard = m.lock();
                let now = counter.fetch_add(1, Ordering::SeqCst) + 1;
                // Track the maximum observed concurrency inside the lock.
                max_inside.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(5));
                counter.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            max_inside.load(Ordering::SeqCst),
            1,
            "exactly one holder of the per-id lock at any moment"
        );
    }
}
