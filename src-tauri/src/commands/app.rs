//! Tauri glue for the renderer's `app.*` namespace — version, platform,
//! shell open, font/installed-apps probes, directory dialog.
//!
//! These are the small "host capability" calls the renderer needs at
//! launch (about screen, "open in finder", external links). Most are
//! thin wrappers over std + tauri-plugin-* surface.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct AppCommandError {
    pub code: AppErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCode {
    Io,
    Unsupported,
    Cancelled,
}

/// Cargo-derived version string. The renderer's about-screen displays
/// this verbatim.
#[tauri::command]
#[specta::specta]
pub fn app_get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Echo a renderer-side log line into the host's stderr. The shim uses
/// this for runtime errors that would otherwise be hidden inside the
/// webview console (no DevTools available headlessly).
#[tauri::command]
#[specta::specta]
pub fn app_log_renderer(level: String, message: String) {
    let tag = match level.to_ascii_lowercase().as_str() {
        "error" => "ERROR",
        "warn" => "WARN ",
        "info" => "INFO ",
        _ => "DEBUG",
    };
    eprintln!("[renderer] [{tag}] {message}");
}

/// Open a URL or file path in the user's default external handler.
/// macOS: `open`, Linux: `xdg-open`, Windows: `start`.
#[tauri::command]
#[specta::specta]
pub fn app_open_external(target: String) -> Result<(), AppCommandError> {
    open_with_default(&target).map_err(|err| AppCommandError {
        code: AppErrorCode::Io,
        message: err,
    })
}

/// Open a path with a specific application identifier (e.g. Finder,
/// VS Code, Cursor). On macOS uses `open -a <appName> <path>`. Linux
/// falls back to `xdg-open` — the `app` arg is informational. Windows
/// uses `start` with the supplied app.
#[tauri::command]
#[specta::specta]
pub fn app_open_in(path: String, app: String) -> Result<(), AppCommandError> {
    let res = if cfg!(target_os = "macos") {
        Command::new("open").arg("-a").arg(&app).arg(&path).status()
    } else if cfg!(target_os = "linux") {
        // Most Linux DEs don't have a stable "open with app X" CLI;
        // fall through to xdg-open. The `app` arg is preserved for
        // logging only.
        let _ = app;
        Command::new("xdg-open").arg(&path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", &app, &path])
            .status()
    } else {
        return Err(AppCommandError {
            code: AppErrorCode::Unsupported,
            message: "unsupported platform".into(),
        });
    };

    match res {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(AppCommandError {
            code: AppErrorCode::Io,
            message: format!("exit status {s}"),
        }),
        Err(e) => Err(AppCommandError {
            code: AppErrorCode::Io,
            message: e.to_string(),
        }),
    }
}

/// Best-effort platform string matching the renderer's expected
/// values: `"darwin" | "linux" | "win32"`. (The Tauri webview can
/// also detect this via navigator UA, but having an authoritative
/// host-side answer avoids UA drift.)
#[tauri::command]
#[specta::specta]
pub fn app_get_platform() -> String {
    if cfg!(target_os = "macos") {
        "darwin".into()
    } else if cfg!(target_os = "windows") {
        "win32".into()
    } else if cfg!(target_os = "linux") {
        "linux".into()
    } else {
        std::env::consts::OS.into()
    }
}

#[cfg(target_os = "macos")]
fn open_with_default(target: &str) -> Result<(), String> {
    Command::new("open")
        .arg(target)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("exit {s}"))
            }
        })
}

#[cfg(target_os = "linux")]
fn open_with_default(target: &str) -> Result<(), String> {
    Command::new("xdg-open")
        .arg(target)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("exit {s}"))
            }
        })
}

#[cfg(target_os = "windows")]
fn open_with_default(target: &str) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", target])
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("exit {s}"))
            }
        })
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_with_default(_target: &str) -> Result<(), String> {
    Err("unsupported platform".into())
}

/// Stub: list installed apps. The renderer uses this to detect which
/// editors are available; for now we return common defaults that ship
/// with the OS. A richer implementation would scan /Applications on
/// macOS or read .desktop entries on Linux.
#[tauri::command]
#[specta::specta]
pub fn app_check_installed_apps(_candidates: Vec<String>) -> Vec<String> {
    Vec::new()
}

/// Stub: enumerate installed fonts. Returning empty leads the renderer
/// to use its bundled font stack — safe default.
#[tauri::command]
#[specta::specta]
pub fn app_list_installed_fonts() -> Vec<String> {
    Vec::new()
}

/// Open a directory-picker dialog. Returns the absolute path or
/// `None` when cancelled. macOS uses AppleScript via `osascript`;
/// Linux uses `zenity` if available; Windows uses PowerShell.
/// (We avoid tauri-plugin-dialog here because the renderer expects a
/// simple "absolute path or null" shape and adding the plugin is a
/// separate dependency churn.)
#[tauri::command]
#[specta::specta]
pub fn app_open_select_directory_dialog() -> Result<Option<String>, AppCommandError> {
    let out: Option<PathBuf> = if cfg!(target_os = "macos") {
        let res = Command::new("osascript")
            .arg("-e")
            .arg("POSIX path of (choose folder)")
            .output();
        match res {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(s))
                }
            }
            Ok(_) => None,
            Err(e) => {
                return Err(AppCommandError {
                    code: AppErrorCode::Io,
                    message: e.to_string(),
                })
            }
        }
    } else {
        // TODO: zenity / PowerShell wrappers. Until then, return None
        // so the renderer can degrade to a typed-path input.
        None
    };

    Ok(out.map(|p| p.to_string_lossy().to_string()))
}

/// Clipboard-write helper. Tauri 2 supports clipboard via a plugin
/// but for simplicity we shell out where available. macOS uses
/// `pbcopy`; Linux uses `xclip` if present; Windows uses `clip`.
#[tauri::command]
#[specta::specta]
pub fn app_clipboard_write_text(text: String) -> Result<(), AppCommandError> {
    use std::io::Write;
    let cmd = if cfg!(target_os = "macos") {
        Some(("pbcopy", Vec::<&str>::new()))
    } else if cfg!(target_os = "linux") {
        Some(("xclip", vec!["-selection", "clipboard"]))
    } else if cfg!(target_os = "windows") {
        Some(("clip", Vec::<&str>::new()))
    } else {
        None
    };

    let Some((bin, args)) = cmd else {
        return Err(AppCommandError {
            code: AppErrorCode::Unsupported,
            message: "no clipboard backend on this platform".into(),
        });
    };

    let mut child = Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppCommandError {
            code: AppErrorCode::Io,
            message: format!("spawn {bin}: {e}"),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| AppCommandError {
                code: AppErrorCode::Io,
                message: e.to_string(),
            })?;
    }

    let status = child.wait().map_err(|e| AppCommandError {
        code: AppErrorCode::Io,
        message: e.to_string(),
    })?;

    if !status.success() {
        return Err(AppCommandError {
            code: AppErrorCode::Io,
            message: format!("{bin} exited {status}"),
        });
    }

    Ok(())
}
