//! Provider integrations (GitHub + Linear today; GitLab / Forgejo /
//! Jira / Plain are explicit v1.x follow-ups). Each provider owns
//! its own auth, types, and command surface.

pub mod github;
pub mod linear;
