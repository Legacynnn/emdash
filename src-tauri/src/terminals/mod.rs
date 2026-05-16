//! Terminals domain. Siblings of conversations under a workspace — each
//! row is a free-form shell tab the user has open. Same DB pattern;
//! a separate table because terminals have different metadata (no
//! agent/provider, just a `name` + an `ssh` flag).

pub mod model;
pub mod service;

pub use model::{NewTerminalInput, Terminal, TerminalsError};
pub use service::TerminalsService;
