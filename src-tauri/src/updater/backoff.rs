//! Exponential backoff for retry-after-failure on update checks.
//!
//! - **Manual** checks: no backoff at all. The user just clicked
//!   "Check for updates"; making them wait for an internal timer to
//!   reset would be hostile.
//! - **Passive** checks (Startup / Resume / Focus / Interval): grow
//!   from 30 s up to `max_delay` (default 30 min), doubling on each
//!   failure. After `max_retries` consecutive failures, the manager
//!   gives up until the user opens the menu (treating it as a
//!   `Manual` check resets the counter).
//!
//! See ADR-0008 for the numeric rationale.

use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct BackoffConfig {
    pub initial: Duration,
    pub max_delay: Duration,
    pub multiplier: u32,
    pub max_retries: u32,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(30),
            max_delay: Duration::from_secs(30 * 60),
            multiplier: 2,
            max_retries: 8,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Backoff {
    config: BackoffConfig,
    attempts: u32,
}

impl Backoff {
    pub fn new(config: BackoffConfig) -> Self {
        Self {
            config,
            attempts: 0,
        }
    }

    /// Compute the next delay and increment the attempt counter.
    /// Returns `None` once `max_retries` is exhausted — the manager
    /// translates this into "give up until next manual check."
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.config.max_retries {
            return None;
        }
        let multiplier_pow = self.config.multiplier.saturating_pow(self.attempts);
        let scaled = self.config.initial.saturating_mul(multiplier_pow.max(1));
        let bounded = scaled.min(self.config.max_delay);
        self.attempts += 1;
        Some(bounded)
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_adr() {
        let c = BackoffConfig::default();
        assert_eq!(c.initial, Duration::from_secs(30));
        assert_eq!(c.max_delay, Duration::from_secs(1800));
        assert_eq!(c.multiplier, 2);
        assert_eq!(c.max_retries, 8);
    }

    #[test]
    fn next_delay_doubles_until_max() {
        let mut b = Backoff::new(BackoffConfig::default());
        assert_eq!(b.next_delay(), Some(Duration::from_secs(30)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(60)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(120)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(240)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(480)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(960)));
        // 1920 > max_delay (1800), so it clamps.
        assert_eq!(b.next_delay(), Some(Duration::from_secs(1800)));
        assert_eq!(b.next_delay(), Some(Duration::from_secs(1800)));
        // 8 attempts consumed; the next call returns None.
        assert_eq!(b.next_delay(), None);
    }

    #[test]
    fn reset_clears_counter() {
        let mut b = Backoff::new(BackoffConfig::default());
        let _ = b.next_delay();
        let _ = b.next_delay();
        assert_eq!(b.attempts(), 2);
        b.reset();
        assert_eq!(b.attempts(), 0);
        assert_eq!(b.next_delay(), Some(Duration::from_secs(30)));
    }
}
