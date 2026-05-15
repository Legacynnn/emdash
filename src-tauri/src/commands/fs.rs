//! Tauri glue for the renderer's `fs.*` namespace.
//!
//! These commands take absolute paths and shell out to std::fs. The
//! renderer's call sites pass `(projectId, workspaceId, filePath, ...)`
//! tuples; the shim resolves projectId/workspaceId into an absolute
//! path (see `src-tauri/ui/src/shim/route-table.ts`) before invoking
//! these commands.
//!
//! All errors funnel through a `FsCommandError` envelope so the
//! renderer can match by code (matching the renderer's existing
//! `{ type: 'fs_error' | 'not_found', ... }` shape).

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct FsCommandError {
    pub code: FsErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FsErrorCode {
    NotFound,
    PermissionDenied,
    NotFile,
    NotDirectory,
    TooLarge,
    Io,
}

#[derive(Debug, Serialize, Type)]
pub struct FsListEntry {
    pub name: String,
    pub path: String,
    pub kind: FsEntryKind,
    /// Byte size; serialized as `f64` to stay within JS Number range
    /// without triggering specta's BigInt-forbidden check on `u64`.
    pub size: Option<f64>,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FsEntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Serialize, Type)]
pub struct FsImageData {
    pub mime: String,
    pub base64: String,
    pub bytes: f64,
}

fn map_io_error(e: std::io::Error) -> FsCommandError {
    let code = match e.kind() {
        std::io::ErrorKind::NotFound => FsErrorCode::NotFound,
        std::io::ErrorKind::PermissionDenied => FsErrorCode::PermissionDenied,
        _ => FsErrorCode::Io,
    };
    FsCommandError {
        code,
        message: e.to_string(),
    }
}

fn validate_absolute(path: &str) -> Result<PathBuf, FsCommandError> {
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err(FsCommandError {
            code: FsErrorCode::Io,
            message: format!("path is not absolute: {path}"),
        });
    }
    Ok(p)
}

const MAX_DEFAULT_BYTES: u64 = 5 * 1024 * 1024;

#[tauri::command]
#[specta::specta]
pub fn fs_read_file(path: String, max_bytes: Option<f64>) -> Result<String, FsCommandError> {
    let abs = validate_absolute(&path)?;
    let meta = fs::metadata(&abs).map_err(map_io_error)?;
    if !meta.is_file() {
        return Err(FsCommandError {
            code: FsErrorCode::NotFile,
            message: format!("not a file: {path}"),
        });
    }
    let cap = max_bytes
        .map(|v| v.max(0.0) as u64)
        .unwrap_or(MAX_DEFAULT_BYTES);
    if meta.len() > cap {
        return Err(FsCommandError {
            code: FsErrorCode::TooLarge,
            message: format!("file exceeds {cap}-byte limit"),
        });
    }
    fs::read_to_string(&abs).map_err(map_io_error)
}

#[tauri::command]
#[specta::specta]
pub fn fs_write_file(path: String, content: String) -> Result<(), FsCommandError> {
    let abs = validate_absolute(&path)?;
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(map_io_error)?;
    }
    fs::write(&abs, content.as_bytes()).map_err(map_io_error)
}

#[tauri::command]
#[specta::specta]
pub fn fs_remove_file(path: String) -> Result<(), FsCommandError> {
    let abs = validate_absolute(&path)?;
    let meta = fs::metadata(&abs).map_err(map_io_error)?;
    if meta.is_dir() {
        fs::remove_dir_all(&abs).map_err(map_io_error)
    } else {
        fs::remove_file(&abs).map_err(map_io_error)
    }
}

#[tauri::command]
#[specta::specta]
pub fn fs_file_exists(path: String) -> Result<bool, FsCommandError> {
    let abs = validate_absolute(&path)?;
    Ok(abs.exists())
}

#[tauri::command]
#[specta::specta]
pub fn fs_stat_file(path: String) -> Result<Option<FsListEntry>, FsCommandError> {
    let abs = validate_absolute(&path)?;
    let meta = match fs::metadata(&abs) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(map_io_error(e)),
    };
    Ok(Some(entry_for(&abs, &meta)))
}

#[tauri::command]
#[specta::specta]
pub fn fs_list_files(
    path: String,
    include_hidden: Option<bool>,
) -> Result<Vec<FsListEntry>, FsCommandError> {
    let abs = validate_absolute(&path)?;
    let meta = fs::metadata(&abs).map_err(map_io_error)?;
    if !meta.is_dir() {
        return Err(FsCommandError {
            code: FsErrorCode::NotDirectory,
            message: format!("not a directory: {path}"),
        });
    }

    let show_hidden = include_hidden.unwrap_or(false);

    let mut out = Vec::new();
    for entry in fs::read_dir(&abs).map_err(map_io_error)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        let p = entry.path();
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        out.push(entry_for(&p, &md));
    }

    out.sort_by(|a, b| match (a.kind_sort_key(), b.kind_sort_key()) {
        (l, r) if l != r => l.cmp(&r),
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(out)
}

fn entry_for(p: &Path, meta: &fs::Metadata) -> FsListEntry {
    let kind = if meta.is_dir() {
        FsEntryKind::Dir
    } else if meta.is_file() {
        FsEntryKind::File
    } else if meta.file_type().is_symlink() {
        FsEntryKind::Symlink
    } else {
        FsEntryKind::Other
    };
    FsListEntry {
        name: p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        path: p.to_string_lossy().to_string(),
        kind,
        size: meta.is_file().then(|| meta.len() as f64),
    }
}

impl FsListEntry {
    fn kind_sort_key(&self) -> u8 {
        match self.kind {
            FsEntryKind::Dir => 0,
            FsEntryKind::Symlink => 1,
            FsEntryKind::File => 2,
            FsEntryKind::Other => 3,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn fs_read_image(path: String) -> Result<FsImageData, FsCommandError> {
    let abs = validate_absolute(&path)?;
    let meta = fs::metadata(&abs).map_err(map_io_error)?;
    if !meta.is_file() {
        return Err(FsCommandError {
            code: FsErrorCode::NotFile,
            message: format!("not a file: {path}"),
        });
    }

    let bytes = fs::read(&abs).map_err(map_io_error)?;
    let mime = guess_mime(&abs);
    let base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(FsImageData {
        mime,
        base64,
        bytes: meta.len() as f64,
    })
}

fn guess_mime(p: &Path) -> String {
    match p
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
    .into()
}
