//! `latest.json` schema. The format Tauri's updater plugin consumes
//! and the format our R2 endpoint must serve (wiring lives in the
//! packaging follow-up — EMD-22 — not here).
//!
//! Documented inline so a reviewer can verify the schema without
//! cross-referencing Tauri docs that change between versions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

pub const LATEST_JSON_SCHEMA_DOC: &str = r#"
{
  "version": "<semver of the published release>",
  "notes": "<release notes; rendered as plain text by the renderer>",
  "pub_date": "<ISO-8601 UTC, e.g. 2026-05-15T17:00:00Z>",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<base64 minisign signature of the tarball below>",
      "url": "<HTTPS URL of the .tar.gz / .app.tar.gz>"
    },
    "darwin-x86_64": { "signature": "...", "url": "..." },
    "linux-x86_64":  { "signature": "...", "url": "..." },
    "windows-x86_64":{ "signature": "...", "url": "..." }
  }
}
"#;

/// One `platforms.<target>` entry in `latest.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ManifestPlatform {
    pub signature: String,
    pub url: String,
}

/// The full `latest.json` document. We accept this shape both for
/// validating fixture manifests in tests and for documenting what
/// the eventual production endpoint must serve.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub pub_date: String,
    pub platforms: BTreeMap<String, ManifestPlatform>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_canonical_manifest() {
        let v = json!({
            "version": "0.2.0",
            "notes": "Bug fixes",
            "pub_date": "2026-05-15T17:00:00Z",
            "platforms": {
                "darwin-aarch64": {
                    "signature": "untrusted comment...",
                    "url": "https://example.com/emdash-dev-0.2.0-darwin-aarch64.tar.gz"
                }
            }
        });
        let m: UpdateManifest = serde_json::from_value(v).unwrap();
        assert_eq!(m.version, "0.2.0");
        assert!(m.platforms.contains_key("darwin-aarch64"));
    }

    #[test]
    fn notes_is_optional() {
        let v = json!({
            "version": "0.2.0",
            "pub_date": "2026-05-15T17:00:00Z",
            "platforms": {}
        });
        let m: UpdateManifest = serde_json::from_value(v).unwrap();
        assert!(m.notes.is_none());
    }

    #[test]
    fn rejects_missing_version() {
        let v = json!({ "pub_date": "...", "platforms": {} });
        let r: Result<UpdateManifest, _> = serde_json::from_value(v);
        assert!(r.is_err());
    }
}
