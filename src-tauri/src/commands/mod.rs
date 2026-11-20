//! Tauri commands: thin wrappers that take the signed-in user from `Session`,
//! call a service, and map models to camelCase response DTOs.

pub mod block_commands;
pub mod document_commands;
pub mod session_commands;
pub mod workspace_commands;

pub use block_commands::*;
pub use document_commands::*;
pub use session_commands::*;
pub use workspace_commands::*;
