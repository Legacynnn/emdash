//! Conversations domain. Each task can host multiple conversation
//! threads (one per agent provider / branch of work). The renderer
//! displays them as tabs inside a task view.
//!
//! Tauri-runtime-free: deals in `Db` and domain types only. Broadcasts
//! live at the glue layer in `commands::conversations`.

pub mod model;
pub mod service;

pub use model::{Conversation, ConversationsError, NewConversationInput};
pub use service::ConversationsService;
