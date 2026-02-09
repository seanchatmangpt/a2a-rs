//! Ports (interfaces) for the a2a-mcp integration
//!
//! Ports define the interfaces that our application needs, independent of implementation details.
//! They represent the "what" - what operations our application needs to perform.

pub mod mcp_task_manager;
pub mod origin_validator;
pub mod session_manager;

// Re-export port interfaces
pub use mcp_task_manager::{McpTaskManager, mcp_task_error, mcp_task_error_with_data};
pub use origin_validator::OriginValidator;
pub use session_manager::SessionManager;
