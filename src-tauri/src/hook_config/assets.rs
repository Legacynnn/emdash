//! Embedded plugin assets sourced verbatim from the Electron build.
//!
//! These files are written into the user's worktree as-is when
//! `hook_config` configures pi / opencode. Bytes match the TS
//! sources exactly so the two builds stay in lockstep — bumping the
//! TS file means copying it back here.

pub const PI_EXTENSION: &str = include_str!("assets/pi-emdash-extension.ts");
pub const OPENCODE_PLUGIN: &str = include_str!("assets/opencode-notifications-plugin.js");
