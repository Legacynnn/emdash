//! `TextClassifier` — runs over raw PTY stdout streams (as opposed
//! to the JSON-body `Classifier` used by Claude Code's hook server).
//!
//! The Electron build's classifiers in `core/agent-hooks/classifiers/*.ts`
//! all share the same pattern: maintain a 4 KiB tail buffer, strip
//! ANSI escapes, run regexes over the tail. This module mirrors that
//! pattern in Rust:
//!
//! - [`BufferedTextClassifier`] holds the per-spawn sliding window
//!   and exposes [`feed`](BufferedTextClassifier::feed). Each call
//!   appends, trims, and invokes the per-agent `match_fn`.
//! - Family-specific implementations live in `text_classifiers/`.
//!
//! Per-spawn state (the buffer) lives in a `parking_lot::Mutex<String>`
//! so the same `Arc<dyn TextClassifier>` is `Send + Sync` and can be
//! shared between the PTY callback thread and any test harness.

use parking_lot::Mutex;

use super::classifier::ClassificationResult;
use super::event::{AgentEventKind, NotificationKind};

/// Max size of the sliding window the classifier keeps. Matches the
/// Electron default in `classifiers/base.ts` (`MAX_BUFFER`).
pub const MAX_BUFFER: usize = 4096;

/// Function signature for the per-agent matcher. Receives the
/// ANSI-stripped buffer (full content, with the tail at the end);
/// returns `Some(result)` if a pattern fired, `None` otherwise.
pub type MatchFn = fn(&str) -> Option<ClassificationResult>;

pub trait TextClassifier: Send + Sync {
    fn name(&self) -> &'static str;
    /// Append `chunk` to the internal buffer (trimming to
    /// [`MAX_BUFFER`]) and run the matcher. Returns the result iff a
    /// pattern fired.
    fn feed(&self, chunk: &str) -> Option<ClassificationResult>;
    /// Drop the buffer (called on session restart).
    fn reset(&self);
}

pub struct BufferedTextClassifier {
    name: &'static str,
    matcher: MatchFn,
    buffer: Mutex<String>,
}

impl BufferedTextClassifier {
    pub const fn new(name: &'static str, matcher: MatchFn) -> Self {
        Self {
            name,
            matcher,
            buffer: Mutex::new(String::new()),
        }
    }
}

impl TextClassifier for BufferedTextClassifier {
    fn name(&self) -> &'static str {
        self.name
    }

    fn feed(&self, chunk: &str) -> Option<ClassificationResult> {
        let mut buf = self.buffer.lock();
        buf.push_str(chunk);
        // Trim from front; keep most recent.
        if buf.len() > MAX_BUFFER {
            let new_start = buf.len() - MAX_BUFFER;
            // Find a char boundary at or after new_start to avoid
            // splitting a multi-byte UTF-8 sequence.
            let safe_start = (new_start..buf.len())
                .find(|&i| buf.is_char_boundary(i))
                .unwrap_or(buf.len());
            buf.replace_range(..safe_start, "");
        }
        let cleaned = strip_ansi(&buf);
        (self.matcher)(&cleaned)
    }

    fn reset(&self) {
        self.buffer.lock().clear();
    }
}

/// Mirrors the TS `stripAnsi` regex pair (CSI + OSC sequences) +
/// `\r` strip. Done in a single pass so we don't allocate
/// intermediate Strings.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\r' {
            i += 1;
            continue;
        }
        if b == 0x1b {
            // ESC. Two cases: CSI ([) ... letter, or OSC (]) ... BEL/ST.
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                // skip until a letter A..Z or a..z appears
                let mut j = i + 2;
                while j < bytes.len() {
                    let c = bytes[j];
                    if c.is_ascii_alphabetic() {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                // OSC: skip until BEL (0x07) or ESC \ (1b 5c)
                let mut j = i + 2;
                while j < bytes.len() {
                    if bytes[j] == 0x07 {
                        j += 1;
                        break;
                    }
                    if bytes[j] == 0x1b && j + 1 < bytes.len() && bytes[j + 1] == b'\\' {
                        j += 2;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        // Push the next char (full UTF-8 sequence).
        let ch_end = (i + 1..=bytes.len())
            .find(|&end| s.is_char_boundary(end))
            .unwrap_or(bytes.len());
        out.push_str(&s[i..ch_end]);
        i = ch_end;
    }
    out
}

/// Helper used by family-A classifiers — they all share the same
/// regex set with one differing pattern (the idle_prompt).
pub fn classify_common(text: &str, idle_prompt_pattern: &str) -> Option<ClassificationResult> {
    use regex::RegexBuilder;
    let tail_start = text.len().saturating_sub(500);
    let tail = &text[tail_start..];

    let case_insensitive = |pat: &str| RegexBuilder::new(pat).case_insensitive(true).build();

    if let Ok(re) = case_insensitive(r"approve|reject|permission|allow|confirm") {
        if re.is_match(tail) {
            return Some(ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::PermissionPrompt,
                },
                message: None,
            });
        }
    }
    if let Ok(re) = case_insensitive(idle_prompt_pattern) {
        if re.is_match(tail) {
            return Some(ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::IdlePrompt,
                },
                message: None,
            });
        }
    }
    if let Ok(re) = case_insensitive(r"Successfully authenticated|Login successful") {
        if re.is_match(text) {
            return Some(ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::AuthSuccess,
                },
                message: None,
            });
        }
    }
    if let Ok(re) = case_insensitive(r"What.*\?|How.*\?|Which.*\?|Please (provide|specify|clarify)")
    {
        if re.is_match(tail) {
            return Some(ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::ElicitationDialog,
                },
                message: None,
            });
        }
    }
    if let Ok(re) = case_insensitive(r"error:|fatal:|exception|failed") {
        if re.is_match(text) {
            return Some(ClassificationResult {
                kind: AgentEventKind::Error,
                message: None,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_removes_csi() {
        let input = "\x1b[31mhello\x1b[0m world";
        assert_eq!(strip_ansi(input), "hello world");
    }

    #[test]
    fn strip_ansi_removes_osc_with_bel() {
        let input = "\x1b]0;title\x07rest";
        assert_eq!(strip_ansi(input), "rest");
    }

    #[test]
    fn strip_ansi_strips_carriage_return() {
        assert_eq!(strip_ansi("foo\r\nbar"), "foo\nbar");
    }

    #[test]
    fn buffer_trims_to_max() {
        let bc = BufferedTextClassifier::new("test", |_| None);
        for _ in 0..1000 {
            bc.feed("0123456789");
        }
        assert!(bc.buffer.lock().len() <= MAX_BUFFER);
    }

    #[test]
    fn reset_clears_buffer() {
        let bc = BufferedTextClassifier::new("test", |_| None);
        bc.feed("hello");
        bc.reset();
        assert!(bc.buffer.lock().is_empty());
    }
}
