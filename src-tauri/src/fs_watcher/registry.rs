//! `WatcherRegistry` — owns one debounced `Watcher` per registered
//! `id` (typically a `project_id`). Drop the registry entry to tear
//! down the underlying watcher.
//!
//! Concurrency: the registry is `Send + Sync` so it can sit in
//! `tauri::State`. Internal state is a `parking_lot::Mutex<HashMap>`.
//!
//! Linux ENOSPC fallback: if `notify::recommended_watcher` returns
//! the inotify ENOSPC variant when we call `watch(...recursive)`,
//! we retry with `RecursiveMode::NonRecursive` to scope to depth 1
//! and record the degradation. The renderer can read the
//! `fallback` field and surface a toast.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{ErrorKind as NotifyErrorKind, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap,
};
use parking_lot::Mutex;

use super::event::{WatchEvent, WatchEventKind, WatcherError, WatcherFallback};

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(50);

pub type EventListener = Arc<dyn Fn(WatchEvent) + Send + Sync>;

/// One live watcher, including its debouncer thread.
pub struct WatcherHandle {
    pub path: PathBuf,
    pub fallback: WatcherFallback,
    _debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap>,
}

#[derive(Default)]
pub struct WatcherRegistry {
    inner: Mutex<HashMap<String, WatcherHandle>>,
}

impl WatcherRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Watch `path` under `id`. Replaces any existing watcher for
    /// the same id (the previous debouncer drops cleanly). Returns
    /// the fallback state so the caller can decide whether to
    /// surface a toast.
    pub fn watch(
        &self,
        id: String,
        path: PathBuf,
        listener: EventListener,
    ) -> Result<WatcherFallback, WatcherError> {
        if !path.exists() {
            return Err(WatcherError::PathMissing(path.display().to_string()));
        }

        // Spawn a single debouncer with the listener; we keep the
        // recursive-vs-non-recursive choice local so the fallback
        // attempt doesn't surface a second listener.
        let listener_cloned = listener.clone();
        let mut debouncer = new_debouncer(
            DEBOUNCE_TIMEOUT,
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    for e in events {
                        if let Some(we) = to_watch_event(&e) {
                            listener_cloned(we);
                        }
                    }
                }
                Err(errors) => {
                    for err in errors {
                        eprintln!("[fs_watcher] debounce error: {err}");
                    }
                }
            },
        )?;

        let (used_recursive, fallback) =
            match debouncer.watcher().watch(&path, RecursiveMode::Recursive) {
                Ok(()) => (true, WatcherFallback::None),
                Err(e) if is_enospc(&e) => {
                    // Fall back to depth-1 scoped watching on Linux when
                    // the inotify watch budget is exhausted.
                    debouncer
                        .watcher()
                        .watch(&path, RecursiveMode::NonRecursive)?;
                    (false, WatcherFallback::InotifyEnospcDepthOne)
                }
                Err(e) => return Err(e.into()),
            };
        let _ = used_recursive;

        let handle = WatcherHandle {
            path: path.clone(),
            fallback,
            _debouncer: debouncer,
        };
        self.inner.lock().insert(id, handle);
        Ok(fallback)
    }

    /// Stop watching the path registered under `id`. Idempotent.
    pub fn unwatch(&self, id: &str) -> bool {
        self.inner.lock().remove(id).is_some()
    }

    pub fn is_watching(&self, id: &str) -> bool {
        self.inner.lock().contains_key(id)
    }

    pub fn watched_count(&self) -> usize {
        self.inner.lock().len()
    }
}

fn is_enospc(err: &notify::Error) -> bool {
    matches!(err.kind, NotifyErrorKind::Io(ref io) if io.raw_os_error() == Some(libc_enospc()))
}

#[cfg(target_os = "linux")]
fn libc_enospc() -> i32 {
    28
}

#[cfg(not(target_os = "linux"))]
fn libc_enospc() -> i32 {
    // ENOSPC only meaningful on Linux's inotify; other platforms
    // can't hit this code path.
    28
}

fn to_watch_event(event: &DebouncedEvent) -> Option<WatchEvent> {
    use notify::event::{EventKind, ModifyKind, RenameMode};
    let paths: Vec<String> = event
        .event
        .paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();
    let kind = match &event.event.kind {
        EventKind::Create(_) => WatchEventKind::Created,
        EventKind::Remove(_) => WatchEventKind::Deleted,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if paths.len() == 2 => {
            // Renames usually come with both endpoints; collapse to
            // the typed variant + clear the paths so the renderer
            // doesn't double-render.
            return Some(WatchEvent {
                paths: Vec::new(),
                event: WatchEventKind::Renamed {
                    from: paths[0].clone(),
                    to: paths[1].clone(),
                },
            });
        }
        EventKind::Modify(_) => WatchEventKind::Modified,
        EventKind::Access(_) | EventKind::Other | EventKind::Any => return None,
    };
    Some(WatchEvent { paths, event: kind })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    fn collecting_listener() -> (Arc<StdMutex<Vec<WatchEvent>>>, EventListener) {
        let log: Arc<StdMutex<Vec<WatchEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = log.clone();
        let listener: EventListener = Arc::new(move |e| sink.lock().unwrap().push(e));
        (log, listener)
    }

    #[test]
    fn watch_missing_path_returns_path_missing() {
        let reg = WatcherRegistry::new();
        let (_, l) = collecting_listener();
        let err = reg
            .watch(
                "id".into(),
                PathBuf::from("/this/path/does/not/exist/q9w8e7"),
                l,
            )
            .unwrap_err();
        assert!(matches!(err, WatcherError::PathMissing(_)));
    }

    #[test]
    fn watch_creates_and_unwatch_removes() {
        let dir = TempDir::new().unwrap();
        let reg = WatcherRegistry::new();
        let (_log, listener) = collecting_listener();
        reg.watch("id".into(), dir.path().to_path_buf(), listener)
            .unwrap();
        assert!(reg.is_watching("id"));
        assert_eq!(reg.watched_count(), 1);
        assert!(reg.unwatch("id"));
        assert!(!reg.is_watching("id"));
    }

    #[test]
    fn writes_under_watched_dir_fire_events() {
        let dir = TempDir::new().unwrap();
        let reg = WatcherRegistry::new();
        let (log, listener) = collecting_listener();
        reg.watch("id".into(), dir.path().to_path_buf(), listener)
            .unwrap();

        // Give the debouncer a beat to register the watch.
        thread::sleep(Duration::from_millis(100));
        std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();

        // Wait long enough for the debounce window + delivery.
        thread::sleep(Duration::from_millis(300));

        let events = log.lock().unwrap();
        assert!(
            !events.is_empty(),
            "expected at least one watch event for the write"
        );
    }
}
