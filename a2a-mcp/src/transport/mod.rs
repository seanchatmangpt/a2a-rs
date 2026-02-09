//! Transport adapters for A2A and RMCP protocols

mod rmcp_to_a2a;
mod a2a_to_rmcp;
pub mod streamable_http;

pub use rmcp_to_a2a::RmcpToA2aTransport;
pub use a2a_to_rmcp::A2aToRmcpTransport;
pub use streamable_http::{
    StreamableHttpServer,
    StreamableHttpConfig,
    McpMessageHandler,
    McpRequest,
    McpResponse,
    McpError,
};