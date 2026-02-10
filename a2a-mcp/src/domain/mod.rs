//! Domain types for a2a-mcp
//!
//! Pure domain types with no external dependencies beyond serialization.

pub mod mcp_task;
pub mod session;

pub use mcp_task::{
    McpTask, McpTaskError, McpTaskGetParams, McpTaskResult, McpTaskResultParams, McpTaskState,
};
pub use session::Session;
