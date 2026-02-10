//! Unified client demo
//!
//! Demonstrates interacting with the unified protocol server using both MCP and A2A protocols.
//!
//! # Prerequisites
//!
//! Start the unified server first:
//! ```bash
//! cargo run --example unified_protocol_server
//! ```
//!
//! Then run this client:
//! ```bash
//! cargo run --example unified_client_demo
//! ```

use serde_json::{Value, json};
use tracing::{Level, error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    let base_url = "http://localhost:3000";
    let client = reqwest::Client::new();

    info!("=".repeat(80));
    info!("🔌 Unified Protocol Client Demo");
    info!("=".repeat(80));
    info!("");

    // Test 1: Health check
    info!("Test 1: Health check");
    info!("-".repeat(40));
    match client.get(format!("{}/health", base_url)).send().await {
        Ok(response) => {
            info!("✓ Health check: {}", response.status());
        }
        Err(e) => {
            error!("✗ Health check failed: {}", e);
            return Ok(());
        }
    }
    info!("");

    // Test 2: Get server info
    info!("Test 2: Server information");
    info!("-".repeat(40));
    match client.get(format!("{}/info", base_url)).send().await {
        Ok(response) => {
            let info: Value = response.json().await?;
            info!("Server info: {}", serde_json::to_string_pretty(&info)?);
        }
        Err(e) => {
            error!("✗ Failed to get server info: {}", e);
        }
    }
    info!("");

    // Test 3: Get bridge statistics
    info!("Test 3: Bridge statistics");
    info!("-".repeat(40));
    match client.get(format!("{}/stats", base_url)).send().await {
        Ok(response) => {
            let stats: Value = response.json().await?;
            info!("Bridge stats: {}", serde_json::to_string_pretty(&stats)?);
        }
        Err(e) => {
            error!("✗ Failed to get bridge stats: {}", e);
        }
    }
    info!("");

    // Test 4: A2A - Get agent card
    info!("Test 4: A2A Protocol - Get agent card");
    info!("-".repeat(40));
    match client
        .get(format!("{}/.well-known/agent-card", base_url))
        .send()
        .await
    {
        Ok(response) => {
            let agent_card: Value = response.json().await?;
            info!("Agent card: {}", serde_json::to_string_pretty(&agent_card)?);
        }
        Err(e) => {
            error!("✗ Failed to get agent card: {}", e);
        }
    }
    info!("");

    // Test 5: MCP - List tools
    info!("Test 5: MCP Protocol - List tools");
    info!("-".repeat(40));
    let mcp_list_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    match client
        .post(format!("{}/mcp", base_url))
        .json(&mcp_list_request)
        .send()
        .await
    {
        Ok(response) => {
            let mcp_response: Value = response.json().await?;
            info!(
                "MCP tools list: {}",
                serde_json::to_string_pretty(&mcp_response)?
            );
        }
        Err(e) => {
            error!("✗ Failed to list MCP tools: {}", e);
        }
    }
    info!("");

    // Test 6: MCP - Call calculator tool (via unified endpoint)
    info!("Test 6: MCP Protocol - Call calculator tool (add 42 + 8)");
    info!("-".repeat(40));
    let mcp_calc_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "method": "calculator",
            "params": {
                "operation": "add",
                "a": 42,
                "b": 8
            }
        }
    });

    match client
        .post(format!("{}/api", base_url))
        .json(&mcp_calc_request)
        .send()
        .await
    {
        Ok(response) => {
            let mcp_response: Value = response.json().await?;
            info!(
                "Calculator result: {}",
                serde_json::to_string_pretty(&mcp_response)?
            );
        }
        Err(e) => {
            error!("✗ Failed to call calculator: {}", e);
        }
    }
    info!("");

    // Test 7: MCP - Call echo tool
    info!("Test 7: MCP Protocol - Call echo tool");
    info!("-".repeat(40));
    let mcp_echo_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "method": "echo",
            "params": {
                "text": "Hello from unified protocol server!"
            }
        }
    });

    match client
        .post(format!("{}/mcp", base_url))
        .json(&mcp_echo_request)
        .send()
        .await
    {
        Ok(response) => {
            let mcp_response: Value = response.json().await?;
            info!(
                "Echo result: {}",
                serde_json::to_string_pretty(&mcp_response)?
            );
        }
        Err(e) => {
            error!("✗ Failed to call echo: {}", e);
        }
    }
    info!("");

    // Test 8: A2A - Send task (bridged to MCP)
    info!("Test 8: A2A Protocol - Send task (will be bridged to MCP)");
    info!("-".repeat(40));
    let a2a_task_request = json!({
        "message": {
            "role": "user",
            "parts": [
                {
                    "text": "Call tool: calculator"
                },
                {
                    "data": {
                        "operation": "multiply",
                        "a": 7,
                        "b": 6
                    },
                    "mimeType": "application/json"
                }
            ]
        }
    });

    match client
        .post(format!("{}/tasks/send", base_url))
        .json(&a2a_task_request)
        .send()
        .await
    {
        Ok(response) => {
            let task_response: Value = response.json().await?;
            info!(
                "Task result: {}",
                serde_json::to_string_pretty(&task_response)?
            );
        }
        Err(e) => {
            error!("✗ Failed to send A2A task: {}", e);
        }
    }
    info!("");

    // Test 9: Auto-detection - Send A2A task via unified endpoint
    info!("Test 9: Protocol Auto-Detection - A2A task via /api endpoint");
    info!("-".repeat(40));
    match client
        .post(format!("{}/api", base_url))
        .json(&a2a_task_request)
        .send()
        .await
    {
        Ok(response) => {
            let task_response: Value = response.json().await?;
            info!(
                "Auto-detected A2A task: {}",
                serde_json::to_string_pretty(&task_response)?
            );
        }
        Err(e) => {
            error!("✗ Failed auto-detected A2A task: {}", e);
        }
    }
    info!("");

    info!("=".repeat(80));
    info!("✓ Demo completed successfully!");
    info!("=".repeat(80));

    Ok(())
}
