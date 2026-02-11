//! Transport adapters for A2A and RMCP protocols

pub mod a2a_to_rmcp;
pub mod rmcp_to_a2a;
pub mod streamable_http;

pub use a2a_to_rmcp::{A2aToRmcpTransport, A2aToRmcpHandler};
pub use rmcp_to_a2a::{RmcpToA2aTransport, RmcpToA2aHandler};
pub use streamable_http::{
    McpError, McpMessageHandler, McpRequest, McpResponse, StreamableHttpConfig,
    StreamableHttpServer,
};
