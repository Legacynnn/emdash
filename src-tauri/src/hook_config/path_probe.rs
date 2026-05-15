//! Cheap "is `<cmd>` on PATH?" probe used by `hook_config` to skip
//! writing configs for providers the user doesn't have installed.
//!
//! Doesn't use the `which` crate to avoid pulling another dep just
//! for one function — `PATH` parsing is 10 lines.

use std::ffi::OsStr;
use std::path::PathBuf;

pub fn command_on_path(cmd: &str) -> bool {
    let path_var = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    let exe_suffixes: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    for dir in std::env::split_paths(&path_var) {
        for suffix in exe_suffixes {
            let mut candidate = PathBuf::from(&dir);
            candidate.push(format!("{cmd}{suffix}"));
            if candidate.is_file() {
                return true;
            }
        }
    }
    let _ = OsStr::new(""); // silence unused-import warning on non-windows
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_command_returns_false() {
        assert!(!command_on_path("definitely-not-installed-anywhere-12345"));
    }

    #[test]
    fn standard_unix_tool_returns_true_on_unix() {
        // `sh` is on every unix box; on Windows we skip the assertion
        // since it isn't guaranteed.
        #[cfg(unix)]
        {
            assert!(command_on_path("sh"));
        }
    }
}
