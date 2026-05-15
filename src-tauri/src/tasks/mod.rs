//! Tasks domain — one task = one git worktree under the project root.
//!
//! Tauri-runtime-free. Broadcast/Tauri glue lives in
//! `commands::tasks`.

pub mod fs_lock;
pub mod model;
pub mod service;
pub mod worktree;

pub use fs_lock::WorkspaceFsMutationLock;
pub use model::{NewTaskInput, Task, TaskSourceBranch, TasksError};
pub use service::TasksService;
