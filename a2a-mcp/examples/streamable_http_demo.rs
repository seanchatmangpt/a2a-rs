//! Demo of MCP Streamable HTTP transport
//!
//! This example demonstrates:
//! - POST endpoint for request/response mode
//! - GET endpoint with SSE for streaming mode
//! - Origin validation
//! - Session management
//!
//! Run with: cargo run -p a2a-mcp --example streamable_http_demo

use std::time::Duration;
use tokio::sync::mpsc;

use a2a_mcp::error::Result;
use a2a_mcp::transport::streamable_http::{
    McpError, McpMessageHandler, McpRequest, McpResponse, StreamableHttpConfig,
    StreamableHttpServer,
};

/// Simple echo handler for demonstration
struct EchoHandler;

#[async_trait::async_trait]
impl McpMessageHandler for EchoHandler {
    async fn handle_request(&self, request: McpRequest) -> Result<McpResponse> {
        println!(
            "Received request: {} (id: {:?})",
            request.method, request.id
        );

        // Echo the request back as the result
        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::json!({
                "echo": request.method,
                "params": request.params,
                "message": "Request processed successfully"
            })),
            error: None,
        })
    }

    async fn handle_streaming_request(
        &self,
        request: McpRequest,
        tx: mpsc::Sender<McpResponse>,
    ) -> Result<()> {
        println!(
            "Received streaming request: {} (id: {:?})",
            request.method, request.id
        );

        // Send multiple responses over time
        for i in 1..=5 {
            let response = McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.clone(),
                result: Some(serde_json::json!({
                    "chunk": i,
                    "message": format!("Streaming response {}/5", i),
                    "method": request.method,
                })),
                error: None,
            };

            if let Err(e) = tx.send(response).await {
                eprintln!("Failed to send response: {}", e);
                return Err(a2a_mcp::error::Error::Server(format!("Send failed: {}", e)));
            }

            // Simulate processing time
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        println!("Streaming request completed");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("\n=== MCP Streamable HTTP Demo ===\n");

    // Configure the server
    let config = StreamableHttpConfig {
        address: "127.0.0.1:3030".to_string(),
        allowed_origins: vec![
            "http://localhost:3030".to_string(),
            "http://127.0.0.1:3030".to_string(),
        ],
        sse_keep_alive: true,
        sse_keep_alive_interval: Duration::from_secs(15),
        max_buffer_size: 100,
    };

    println!("Server configuration:");
    println!("  Address: {}", config.address);
    println!("  Allowed origins: {:?}", config.allowed_origins);
    println!("  SSE keep-alive: {}", config.sse_keep_alive);
    println!();

    // Create handler and server
    let handler = EchoHandler;
    let server = StreamableHttpServer::new(handler, config);

    println!("Starting MCP Streamable HTTP server...");
    println!();
    println!("Try these requests:");
    println!();
    println!("1. POST request (request/response mode):");
    println!(r#"   curl -X POST http://127.0.0.1:3030/mcp \"#);
    println!(r#"     -H "Content-Type: application/json" \"#);
    println!(
        r#"     -d '{{"jsonrpc":"2.0","id":1,"method":"test/echo","params":{{"message":"hello"}}}}'\"#
    );
    println!();
    println!("2. SSE stream (streaming mode):");
    println!(
        r#"   curl -N http://127.0.0.1:3030/mcp/sse?request=%7B%22jsonrpc%22%3A%222.0%22%2C%22id%22%3A2%2C%22method%22%3A%22test%2Fstream%22%7D"#
    );
    println!();
    println!("Press Ctrl+C to stop the server");
    println!();

    // Start the server
    server.start().await?;

    Ok(())
}
