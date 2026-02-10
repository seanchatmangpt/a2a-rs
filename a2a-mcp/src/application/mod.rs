//! Application layer for a2a-mcp
//!
//! JSON-RPC handlers and request routing.

pub mod mcp_task_handlers;

pub use mcp_task_handlers::{JsonRpcRequest, JsonRpcResponse, McpTaskHandler};
