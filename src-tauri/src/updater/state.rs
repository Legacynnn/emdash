//! `UpdateManager` — the state machine that consumes
//! `tauri-plugin-updater` and produces a stream of
//! `UpdateEvent`s for the renderer.
//!
//! Tauri-runtime-free: the plugin glue (creating a `tauri::Updater`
//! handle, registering the `RunEvent::Exit` hook) lives in
//! `commands::updater` / `app.rs`. This module owns the *logic* —
//! state transitions, backoff bookkeeping, progress throttling,
//! event broadcast.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use super::backoff::{Backoff, BackoffConfig};
use super::event::{CheckReason, UpdateError, UpdateEvent};

/// Coalesces `Downloading` progress events. Each download phase
/// starts a fresh ~200 ms ticker; in-between samples are dropped.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking { reason: CheckReason },
    UpToDate,
    Available { version: String },
    Downloading { progress: f64 },
    ReadyToInstall { version: String },
    Failed { code: UpdateError, will_retry: bool },
}

/// Boxed event listener — the Tauri glue passes a closure that
/// forwards events into the renderer's broadcast channel.
pub type EventListener = Arc<dyn Fn(UpdateEvent) + Send + Sync>;

struct Inner {
    state: UpdateState,
    backoff: Backoff,
    last_progress_emit: Option<Instant>,
    listeners: Vec<EventListener>,
}

#[derive(Clone)]
pub struct UpdateManager {
    inner: Arc<Mutex<Inner>>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new(BackoffConfig::default())
    }
}

impl UpdateManager {
    pub fn new(backoff_config: BackoffConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                state: UpdateState::Idle,
                backoff: Backoff::new(backoff_config),
                last_progress_emit: None,
                listeners: Vec::new(),
            })),
        }
    }

    /// Register an event listener. Listeners receive every emitted
    /// `UpdateEvent` in the order they fire. The Tauri glue
    /// registers one listener at startup that fans into the
    /// renderer channel.
    pub fn on_event(&self, listener: EventListener) {
        self.inner.lock().listeners.push(listener);
    }

    pub fn state(&self) -> UpdateState {
        self.inner.lock().state.clone()
    }

    pub fn attempts(&self) -> u32 {
        self.inner.lock().backoff.attempts()
    }

    pub fn begin_check(&self, reason: CheckReason) {
        let mut g = self.inner.lock();
        g.state = UpdateState::Checking { reason };
        let listeners = g.listeners.clone();
        drop(g);
        for l in listeners {
            l(UpdateEvent::Checking { reason });
        }
    }

    pub fn mark_up_to_date(&self) {
        let mut g = self.inner.lock();
        g.state = UpdateState::UpToDate;
        g.backoff.reset();
        let listeners = g.listeners.clone();
        drop(g);
        for l in listeners {
            l(UpdateEvent::UpToDate);
        }
    }

    pub fn mark_available(&self, version: String, notes: Option<String>) {
        let mut g = self.inner.lock();
        g.state = UpdateState::Available {
            version: version.clone(),
        };
        g.backoff.reset();
        g.last_progress_emit = None;
        let listeners = g.listeners.clone();
        drop(g);
        let event = UpdateEvent::Available { version, notes };
        for l in listeners {
            l(event.clone());
        }
    }

    /// Record a progress sample. Coalesces — if the last `Downloading`
    /// event went out less than [`PROGRESS_THROTTLE`] ago, the
    /// sample updates the in-memory state but doesn't broadcast.
    /// A final `progress == 1.0` always fires regardless of the
    /// throttle so the renderer sees the completion edge.
    pub fn record_progress(&self, progress: f64) {
        let mut g = self.inner.lock();
        g.state = UpdateState::Downloading { progress };
        let now = Instant::now();
        let due = match g.last_progress_emit {
            None => true,
            Some(prev) => now.duration_since(prev) >= PROGRESS_THROTTLE,
        };
        let force = progress >= 1.0;
        if !(due || force) {
            return;
        }
        g.last_progress_emit = Some(now);
        let listeners = g.listeners.clone();
        drop(g);
        for l in listeners {
            l(UpdateEvent::Downloading { progress });
        }
    }

    pub fn mark_ready_to_install(&self, version: String) {
        let mut g = self.inner.lock();
        g.state = UpdateState::ReadyToInstall {
            version: version.clone(),
        };
        let listeners = g.listeners.clone();
        drop(g);
        for l in listeners {
            l(UpdateEvent::ReadyToInstall {
                version: version.clone(),
            });
        }
    }

    /// Record a failure. Manual checks never retry automatically;
    /// passive checks consume one backoff slot. Returns the delay
    /// the caller should sleep before the next attempt — `None`
    /// means "stop trying."
    pub fn mark_failed(
        &self,
        code: UpdateError,
        message: String,
        reason: CheckReason,
    ) -> Option<Duration> {
        let mut g = self.inner.lock();
        let next_delay = match reason {
            CheckReason::Manual => None,
            _ => g.backoff.next_delay(),
        };
        let will_retry = next_delay.is_some();
        g.state = UpdateState::Failed { code, will_retry };
        let listeners = g.listeners.clone();
        drop(g);
        let event = UpdateEvent::Error {
            code,
            message,
            will_retry,
        };
        for l in listeners {
            l(event.clone());
        }
        next_delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    fn collecting_listener() -> (Arc<StdMutex<Vec<UpdateEvent>>>, EventListener) {
        let log: Arc<StdMutex<Vec<UpdateEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = log.clone();
        let listener: EventListener = Arc::new(move |e| sink.lock().unwrap().push(e));
        (log, listener)
    }

    #[test]
    fn begin_check_emits_checking_event() {
        let mgr = UpdateManager::default();
        let (log, l) = collecting_listener();
        mgr.on_event(l);
        mgr.begin_check(CheckReason::Manual);
        let events = log.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            UpdateEvent::Checking {
                reason: CheckReason::Manual
            }
        ));
    }

    #[test]
    fn up_to_date_resets_backoff() {
        let mgr = UpdateManager::default();
        // Burn a couple of failures.
        mgr.mark_failed(UpdateError::Network, "x".into(), CheckReason::Startup);
        mgr.mark_failed(UpdateError::Network, "x".into(), CheckReason::Startup);
        assert_eq!(mgr.attempts(), 2);
        mgr.mark_up_to_date();
        assert_eq!(mgr.attempts(), 0);
    }

    #[test]
    fn manual_failure_does_not_schedule_retry() {
        let mgr = UpdateManager::default();
        let next = mgr.mark_failed(UpdateError::Network, "x".into(), CheckReason::Manual);
        assert!(next.is_none());
        assert!(matches!(
            mgr.state(),
            UpdateState::Failed {
                will_retry: false,
                ..
            }
        ));
    }

    #[test]
    fn passive_failure_schedules_retry() {
        let mgr = UpdateManager::default();
        let next = mgr.mark_failed(UpdateError::Network, "x".into(), CheckReason::Interval);
        assert_eq!(next, Some(Duration::from_secs(30)));
        assert!(matches!(
            mgr.state(),
            UpdateState::Failed {
                will_retry: true,
                ..
            }
        ));
    }

    #[test]
    fn progress_throttles_intermediate_samples() {
        let mgr = UpdateManager::default();
        let (log, l) = collecting_listener();
        mgr.on_event(l);

        mgr.record_progress(0.10); // first emit goes through
        mgr.record_progress(0.20); // throttled
        mgr.record_progress(0.30); // throttled

        let events = log.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn progress_one_always_fires() {
        let mgr = UpdateManager::default();
        let (log, l) = collecting_listener();
        mgr.on_event(l);

        mgr.record_progress(0.10);
        mgr.record_progress(1.0); // bypasses throttle

        let events = log.lock().unwrap();
        assert_eq!(events.len(), 2);
        if let UpdateEvent::Downloading { progress } = &events[1] {
            assert!((progress - 1.0).abs() < f64::EPSILON);
        } else {
            panic!("expected Downloading event");
        }
    }

    #[test]
    fn ready_to_install_state_transitions() {
        let mgr = UpdateManager::default();
        mgr.mark_available("0.2.0".into(), Some("notes".into()));
        mgr.record_progress(1.0);
        mgr.mark_ready_to_install("0.2.0".into());
        assert!(matches!(
            mgr.state(),
            UpdateState::ReadyToInstall { ref version } if version == "0.2.0"
        ));
    }
}
