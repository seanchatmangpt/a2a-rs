//! MCP Streamable HTTP Server Example
//!
//! Demonstrates how to run an Axum HTTP server with MCP protocol support,
//! including:
//! - Origin guard middleware for DNS rebinding defense
//! - Session middleware for request scoping
//! - SSE streaming support with Last-Event-ID resumption
//! - JSON-RPC 2.0 request/response handling

use a2a_mcp::{
    InMemorySessionManager, McpTaskHandler, OriginGuard, OriginValidator, StreamableHttpServer,
    TaskWrapper,
};
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (if tracing feature is enabled)
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber;
        tracing_subscriber::fmt::init();
    }

    // Create task wrapper (in-memory task management)
    let task_wrapper = Arc::new(TaskWrapper::new());

    // Create MCP task handler
    let handler = Arc::new(McpTaskHandler::new(task_wrapper));

    // Create session manager
    let session_manager = Arc::new(InMemorySessionManager::new());

    // Create origin guard with localhost-only origins
    let origin_guard = Arc::new(OriginGuard::localhost_only());

    // Create server
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    let server = StreamableHttpServer::new(addr, handler, origin_guard, session_manager);

    println!("Starting MCP Streamable HTTP server on {}", addr);
    println!("Endpoints:");
    println!("  POST   /mcp              - JSON-RPC 2.0 request/response");
    println!("  GET    /mcp              - Server-Sent Events streaming");
    println!();
    println!("Example client requests:");
    println!("  curl -X POST http://localhost:3000/mcp \\");
    println!("    -H 'Content-Type: application/json' \\");
    println!("    -H 'Origin: http://localhost:3000' \\");
    println!(
        "    -d '{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"tasks/list\", \"params\": null}'"
    );
    println!();
    println!("  curl http://localhost:3000/mcp \\");
    println!("    -H 'Origin: http://localhost:3000'");
    println!();

    // Run the server
    server.start().await?;

    Ok(())
}
