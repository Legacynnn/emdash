//! Projects domain. Owns the `projects` table from the collapsed bootstrap
//! migration (see `db::migrations`). Exposes a minimal CRUD surface used
//! by the Tauri glue in `commands::projects`.
//!
//! Tauri-runtime-free: this module deals in `Db`, `String`, and domain
//! errors only. Broadcasting `UiMutationEvent`s lives at the glue layer.

pub mod model;
pub mod service;

pub use model::{Project, ProjectsError};
pub use service::ProjectsService;
