//! # A2A-RMCP Integration
//!
//! This crate provides integration between the Agent-to-Agent (A2A) protocol
//! and Rusty Model Context Protocol (RMCP), enabling bidirectional
//! communication between these protocols.
//!
//! ## Core Features
//!
//! - Use A2A agents as RMCP tools
//! - Expose RMCP tools as A2A agents
//! - Bidirectional message conversion
//! - State management across protocols
//!
//! ## Architecture
//!
//! The crate follows a bridge pattern with adapter layers:
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │               a2a-mcp Crate                 │
//! ├─────────────┬─────────────┬─────────────────┤
//! │ RMCP Client │ Translation │    A2A Client   │
//! │ Interface   │    Layer    │    Interface    │
//! ├─────────────┼─────────────┼─────────────────┤
//! │ RMCP Server │ Conversion  │    A2A Server   │
//! │ Interface   │    Layer    │    Interface    │
//! └─────────────┴─────────────┴─────────────────┘
//! ```

// For the initial implementation, we'll provide the basic structure
// and a simple example. In a full implementation, this would be expanded
// to include all the modules listed below.

pub mod adapter;
pub mod application;
pub mod domain;
pub mod error;
pub mod port;

/*
mod message;
mod transport;
mod client;
mod server;
mod util;
#[cfg(test)]
mod tests;

// Re-export key components
pub use client::A2aRmcpClient;
pub use server::RmcpA2aServer;
pub use adapter::{AgentToToolAdapter, ToolToAgentAdapter};
pub use message::MessageConverter;
*/

// Re-export error types
pub use error::{Error, Result};

// Re-export domain types
pub use domain::{
    McpTask, McpTaskError, McpTaskGetParams, McpTaskResult, McpTaskResultParams, McpTaskState,
    Session,
};

// Re-export port traits
pub use port::{McpTaskManager, SessionManager, mcp_task_error, mcp_task_error_with_data};

// Re-export adapter implementations
pub use adapter::{InMemorySessionManager, TaskWrapper};

// Re-export application components
pub use application::{JsonRpcRequest, JsonRpcResponse, McpTaskHandler};

// Version information
/// Current crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Simple placeholder for now
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
