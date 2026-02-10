//! Unified protocol server example
//!
//! Demonstrates a single server exposing both MCP and A2A protocols with bidirectional bridging.
//!
//! # Usage
//!
//! Start the server:
//! ```bash
//! cargo run --example unified_protocol_server
//! ```
//!
//! Test A2A endpoint:
//! ```bash
//! curl http://localhost:3000/.well-known/agent-card
//! curl -X POST http://localhost:3000/tasks/send \
//!   -H "Content-Type: application/json" \
//!   -d '{"message": {"role": "user", "parts": [{"text": "Hello"}]}}'
//! ```
//!
//! Test MCP endpoint:
//! ```bash
//! curl -X POST http://localhost:3000/mcp \
//!   -H "Content-Type: application/json" \
//!   -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'
//! ```
//!
//! Test unified endpoint (auto-detects protocol):
//! ```bash
//! curl -X POST http://localhost:3000/api \
//!   -H "Content-Type: application/json" \
//!   -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'
//! ```

use a2a_rs::domain::agent::{AgentCard, Authentication, Capabilities, Skill};
use osiris_edge::{BridgeConfig, UnifiedServer, UnifiedServerConfig};
use rmcp::{Server as RmcpServer, Tool, ToolCall, ToolResponse};
use serde_json::json;
use std::net::SocketAddr;
use tracing::{Level, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("Starting unified MCP + A2A protocol server");

    // Create MCP server with example tools
    let mut rmcp_server = RmcpServer::new("example-mcp-server", "1.0.0");

    // Register MCP tools
    let calculator_tool = Tool {
        name: "calculator".to_string(),
        description: "Performs basic arithmetic operations".to_string(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The arithmetic operation to perform"
                },
                "a": {
                    "type": "number",
                    "description": "First operand"
                },
                "b": {
                    "type": "number",
                    "description": "Second operand"
                }
            },
            "required": ["operation", "a", "b"]
        })),
    };

    let echo_tool = Tool {
        name: "echo".to_string(),
        description: "Echoes back the input text".to_string(),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to echo"
                }
            },
            "required": ["text"]
        })),
    };

    rmcp_server.register_tool(calculator_tool.clone());
    rmcp_server.register_tool(echo_tool.clone());

    // Set up tool handlers
    rmcp_server.set_tool_handler(Box::new(|call: ToolCall| {
        Box::pin(async move {
            match call.method.as_str() {
                "calculator" => {
                    // Parse parameters
                    let operation = call
                        .params
                        .get("operation")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| rmcp::Error::InvalidParams("Missing operation".into()))?;
                    let a = call
                        .params
                        .get("a")
                        .and_then(|v| v.as_f64())
                        .ok_or_else(|| rmcp::Error::InvalidParams("Missing a".into()))?;
                    let b = call
                        .params
                        .get("b")
                        .and_then(|v| v.as_f64())
                        .ok_or_else(|| rmcp::Error::InvalidParams("Missing b".into()))?;

                    let result = match operation {
                        "add" => a + b,
                        "subtract" => a - b,
                        "multiply" => a * b,
                        "divide" => {
                            if b == 0.0 {
                                return Err(rmcp::Error::InvalidParams("Division by zero".into()));
                            }
                            a / b
                        }
                        _ => return Err(rmcp::Error::InvalidParams("Invalid operation".into())),
                    };

                    Ok(ToolResponse {
                        result: json!({
                            "operation": operation,
                            "a": a,
                            "b": b,
                            "result": result
                        }),
                    })
                }
                "echo" => {
                    let text = call
                        .params
                        .get("text")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| rmcp::Error::InvalidParams("Missing text".into()))?;

                    Ok(ToolResponse {
                        result: json!({
                            "echoed": text
                        }),
                    })
                }
                _ => Err(rmcp::Error::MethodNotFound(call.method.clone())),
            }
        })
    }));

    // Create A2A agent card
    let agent_card = AgentCard {
        name: "Data Processing Agent".to_string(),
        description: "Processes and analyzes data using various methods".to_string(),
        url: "http://localhost:3000".to_string(),
        version: "1.0.0".to_string(),
        capabilities: Capabilities {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        },
        authentication: Authentication {
            schemes: vec!["Bearer".to_string()],
        },
        default_input_modes: vec!["text".to_string(), "data".to_string()],
        default_output_modes: vec!["text".to_string(), "data".to_string()],
        skills: vec![
            Skill {
                name: "analyze_data".to_string(),
                description: "Analyzes structured data and returns insights".to_string(),
                inputs: None,
                outputs: None,
                input_modes: Some(vec!["data".to_string()]),
                output_modes: Some(vec!["text".to_string(), "data".to_string()]),
                metadata: None,
            },
            Skill {
                name: "summarize_text".to_string(),
                description: "Summarizes long text into key points".to_string(),
                inputs: None,
                outputs: None,
                input_modes: Some(vec!["text".to_string()]),
                output_modes: Some(vec!["text".to_string()]),
                metadata: None,
            },
        ],
        metadata: None,
    };

    // Configure unified server
    let bridge_config = BridgeConfig {
        enable_mcp_to_a2a: true,
        enable_a2a_to_mcp: true,
        a2a_agent_url: Some("http://localhost:3000".to_string()),
        mcp_server_url: Some("http://localhost:3000/mcp".to_string()),
        max_concurrent_bridges: 100,
    };

    let server_config = UnifiedServerConfig {
        address: SocketAddr::from(([127, 0, 0, 1], 3000)),
        bridge_config,
        log_detection: true,
    };

    // Create unified server
    let server = UnifiedServer::new(server_config).with_rmcp_server(rmcp_server);

    // Register MCP tools to be exposed as A2A capabilities
    info!("Registering MCP tools for A2A bridging");
    server.register_mcp_tool(calculator_tool).await;
    server.register_mcp_tool(echo_tool).await;

    // Register A2A agent to be exposed as MCP tools
    info!("Registering A2A agent for MCP bridging");
    server
        .register_a2a_agent("http://localhost:3000".to_string(), agent_card)
        .await;

    info!("=".repeat(80));
    info!("🚀 Unified Protocol Server Started!");
    info!("=".repeat(80));
    info!("");
    info!("Server running on: http://localhost:3000");
    info!("");
    info!("Available endpoints:");
    info!("  GET  /health              - Health check");
    info!("  GET  /info                - Server information");
    info!("  GET  /stats               - Bridge statistics");
    info!("");
    info!("A2A Protocol endpoints:");
    info!("  GET  /.well-known/agent-card  - Get agent card");
    info!("  POST /tasks/send              - Send task");
    info!("  GET  /tasks/get               - Get task status");
    info!("");
    info!("MCP Protocol endpoints:");
    info!("  POST /mcp                 - MCP JSON-RPC requests");
    info!("  GET  /mcp/sse             - MCP SSE streaming");
    info!("");
    info!("Unified endpoint (auto-detects protocol):");
    info!("  ANY  /api                 - Unified API endpoint");
    info!("");
    info!("Example MCP request:");
    info!("  curl -X POST http://localhost:3000/mcp \\");
    info!("    -H 'Content-Type: application/json' \\");
    info!("    -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}}'");
    info!("");
    info!("Example A2A request:");
    info!("  curl -X POST http://localhost:3000/tasks/send \\");
    info!("    -H 'Content-Type: application/json' \\");
    info!("    -d '{{\"message\":{{\"role\":\"user\",\"parts\":[{{\"text\":\"Hello\"}}]}}}}'");
    info!("");
    info!("Example unified request (auto-detects as MCP):");
    info!("  curl -X POST http://localhost:3000/api \\");
    info!("    -H 'Content-Type: application/json' \\");
    info!("    -d '{{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\",\"id\":1}}'");
    info!("");
    info!("=".repeat(80));

    // Start server
    server.start().await?;

    Ok(())
}
