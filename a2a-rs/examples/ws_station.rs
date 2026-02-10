//! WebSocket Station Example with Receipt Chain Generation
//!
//! This example demonstrates:
//! - WebSocket server using Station trait for typed packet processing
//! - Streaming events via EventStream
//! - Receipt chain generation for state transitions
//! - Deterministic state management with OntologyState
//!
//! Run with:
//! ```bash
//! cargo run --example ws_station --features "ws-server,receipts,server"
//! ```
//!
//! Connect with:
//! ```bash
//! wscat -c ws://127.0.0.1:8081
//! ```

use futures::SinkExt;
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use a2a_rs::construct::ontology::OntologyState;
use a2a_rs::construct::station::{Ontology, RefusalReceipt, Station, StationRegistry};
use a2a_rs::construct::types::{JsonRpcId, PacketType};
use a2a_rs::domain::{AgentCard, Message, Part, Role, Task, TaskState};
use a2a_rs::observability;

#[cfg(feature = "receipts")]
use a2a_rs::construct::receipts::ReceiptChain;

#[cfg(feature = "server")]
use a2a_rs::construct::EventStream;

/// WebSocket station server state
struct StationServer {
    /// Station registry for method dispatch
    registry: StationRegistry,
    /// Ontology state (protocol state model)
    ontology: Arc<RwLock<Ontology>>,
    /// Receipt chain for audit trail
    #[cfg(feature = "receipts")]
    receipts: Arc<RwLock<ReceiptChain>>,
    /// Event streams per task
    #[cfg(feature = "server")]
    event_streams: Arc<RwLock<std::collections::HashMap<String, Arc<EventStream>>>>,
    /// Agent card
    agent_card: AgentCard,
}

impl StationServer {
    fn new(agent_card: AgentCard) -> Self {
        Self {
            registry: StationRegistry::new(),
            ontology: Arc::new(RwLock::new(OntologyState::new())),
            #[cfg(feature = "receipts")]
            receipts: Arc::new(RwLock::new(ReceiptChain::new())),
            #[cfg(feature = "server")]
            event_streams: Arc::new(RwLock::new(std::collections::HashMap::new())),
            agent_card,
        }
    }

    /// Process incoming JSON-RPC request
    async fn process_request(&mut self, request_json: &str) -> String {
        tracing::info!("Processing request: {}", request_json);

        // Parse the request to extract method and params
        let request: serde_json::Value = match serde_json::from_str(request_json) {
            Ok(v) => v,
            Err(e) => {
                return self.error_response(None, -32700, format!("Parse error: {}", e), None);
            }
        };

        let method = match request.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                return self.error_response(
                    request.get("id").cloned(),
                    -32600,
                    "Invalid request: missing method".to_string(),
                    None,
                );
            }
        };

        let params = request
            .get("params")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let id = request
            .get("id")
            .and_then(|id| serde_json::from_value::<JsonRpcId>(id.clone()).ok());

        tracing::info!("Method: {}, ID: {:?}", method, id);

        // Special handling for agent/getExtendedCard
        if method == "agent/getExtendedCard" {
            return self.get_extended_card_response(id);
        }

        // Dispatch to station
        let mut ontology = self.ontology.write().await;

        #[cfg(feature = "receipts")]
        let observation = format!("method={}, params={}", method, params);

        match self
            .registry
            .dispatch(method, &mut ontology, params, id.clone())
        {
            Ok(response) => {
                #[cfg(feature = "receipts")]
                {
                    let action = format!("response={}", response);
                    let delta = format!("tasks={}", ontology.task_count());

                    let mut receipts = self.receipts.write().await;
                    let receipt = receipts.add_transition(
                        observation.as_bytes(),
                        action.as_bytes(),
                        delta.as_bytes(),
                    );

                    tracing::info!(
                        "Receipt generated: seq={}, hash={}",
                        receipt.sequence,
                        &receipt.receipt_hash[..16]
                    );
                }

                // Check if this created a new task and set up streaming
                #[cfg(feature = "server")]
                if let Some(task) = response.get("result").and_then(|r| r.get("id")) {
                    if let Some(task_id) = task.as_str() {
                        self.ensure_event_stream(task_id.to_string()).await;
                    }
                }

                serde_json::to_string_pretty(&response).unwrap_or_else(|_| {
                    self.error_response(
                        id,
                        -32603,
                        "Failed to serialize response".to_string(),
                        None,
                    )
                })
            }
            Err(refusal) => {
                tracing::warn!("Request refused: {}", refusal);

                #[cfg(feature = "receipts")]
                {
                    let action = format!("refusal: {}", refusal);
                    let delta = "no state change".to_string();

                    let mut receipts = self.receipts.write().await;
                    receipts.add_transition(
                        observation.as_bytes(),
                        action.as_bytes(),
                        delta.as_bytes(),
                    );
                }

                self.error_response(id, refusal.code, refusal.reason, refusal.data)
            }
        }
    }

    /// Ensure event stream exists for task
    #[cfg(feature = "server")]
    async fn ensure_event_stream(&self, task_id: String) {
        let mut streams = self.event_streams.write().await;
        if !streams.contains_key(&task_id) {
            let stream = Arc::new(EventStream::new(task_id.clone(), 100));
            streams.insert(task_id.clone(), stream.clone());

            // Emit initial event
            let _ = stream.emit_status(TaskState::Working, None).await;
            tracing::info!("Created event stream for task: {}", task_id);
        }
    }

    /// Get extended card response
    fn get_extended_card_response(&self, id: Option<JsonRpcId>) -> String {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": self.agent_card,
        });
        serde_json::to_string_pretty(&response).unwrap()
    }

    /// Create error response
    fn error_response(
        &self,
        id: Option<JsonRpcId>,
        code: i32,
        message: String,
        data: Option<serde_json::Value>,
    ) -> String {
        let mut error = serde_json::json!({
            "code": code,
            "message": message,
        });

        if let Some(d) = data {
            error.as_object_mut().unwrap().insert("data".to_string(), d);
        }

        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": error,
        });

        serde_json::to_string_pretty(&response).unwrap()
    }

    /// Get receipt chain summary
    #[cfg(feature = "receipts")]
    async fn receipt_summary(&self) -> String {
        let receipts = self.receipts.read().await;
        if receipts.is_empty() {
            return "No receipts yet".to_string();
        }

        let latest = receipts.latest().unwrap();
        format!(
            "Receipt chain: {} receipts, latest seq={}, hash={}",
            receipts.len(),
            latest.sequence,
            &latest.receipt_hash[..16]
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    observability::init_tracing();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       WebSocket Station with Receipt Chain Demo            ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Create agent card
    let agent_card = AgentCard::builder()
        .name("Station Demo Agent".to_string())
        .url("ws://0.0.0.0:8081".to_string())
        .description(Some(
            "A2A Station-based WebSocket agent with receipt chain generation".to_string(),
        ))
        .build();

    println!("Agent: {}", agent_card.name);
    println!("URL: {}", agent_card.url);
    println!();

    // Create server
    let server = Arc::new(RwLock::new(StationServer::new(agent_card)));

    // Bind to 0.0.0.0:8081
    let addr = "0.0.0.0:8081";
    let listener = TcpListener::bind(addr).await?;

    println!("🚀 WebSocket station listening on ws://{}", addr);
    println!("   Features:");
    println!("   - Station-based packet processing");
    println!("   - Streaming events via EventStream");
    #[cfg(feature = "receipts")]
    println!("   - Receipt chain generation");
    println!();
    println!("📡 Waiting for connections...\n");

    // Accept connections
    loop {
        let (stream, peer) = listener.accept().await?;
        let server = server.clone();

        tokio::spawn(async move {
            tracing::info!("New connection from {}", peer);
            println!("🔌 Client connected: {}", peer);

            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

                    while let Some(msg) = ws_receiver.next().await {
                        match msg {
                            Ok(WsMessage::Text(text)) => {
                                tracing::debug!("Received: {}", text);

                                let mut server = server.write().await;
                                let response = server.process_request(&text).await;

                                if let Err(e) =
                                    ws_sender.send(WsMessage::Text(response.clone())).await
                                {
                                    tracing::error!("Failed to send response: {}", e);
                                    break;
                                }

                                // Log receipt summary
                                #[cfg(feature = "receipts")]
                                {
                                    let summary = server.receipt_summary().await;
                                    tracing::info!("{}", summary);
                                    println!("📝 {}", summary);
                                }
                            }
                            Ok(WsMessage::Close(_)) => {
                                tracing::info!("Client {} closed connection", peer);
                                println!("👋 Client disconnected: {}", peer);
                                break;
                            }
                            Ok(WsMessage::Ping(data)) => {
                                if let Err(e) = ws_sender.send(WsMessage::Pong(data)).await {
                                    tracing::error!("Failed to send pong: {}", e);
                                    break;
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }

                    // Print final receipt summary
                    #[cfg(feature = "receipts")]
                    {
                        let server = server.read().await;
                        let receipts = server.receipts.read().await;
                        if !receipts.is_empty() {
                            println!("\n📊 Final Receipt Chain Summary:");
                            println!("   Total receipts: {}", receipts.len());

                            if let Ok(_) = receipts.verify_integrity() {
                                println!("   ✓ Chain integrity verified");
                            } else {
                                println!("   ✗ Chain integrity check failed");
                            }

                            if let Some(latest) = receipts.latest() {
                                println!("   Latest sequence: {}", latest.sequence);
                                println!("   Latest hash: {}", &latest.receipt_hash[..32]);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("WebSocket handshake failed: {}", e);
                    println!("❌ Handshake failed for {}: {}", peer, e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_station_server_creation() {
        let agent_card = AgentCard::builder()
            .name("Test Agent".to_string())
            .url("ws://localhost:8081".to_string())
            .build();

        let server = StationServer::new(agent_card);
        assert_eq!(server.agent_card.name, "Test Agent");
    }

    #[tokio::test]
    async fn test_get_extended_card() {
        let agent_card = AgentCard::builder()
            .name("Test Agent".to_string())
            .url("ws://localhost:8081".to_string())
            .build();

        let server = StationServer::new(agent_card);
        let response =
            server.get_extended_card_response(Some(JsonRpcId::from_string("test-1".to_string())));

        assert!(response.contains("Test Agent"));
        assert!(response.contains("jsonrpc"));
    }

    #[cfg(feature = "receipts")]
    #[tokio::test]
    async fn test_receipt_chain_generation() {
        let agent_card = AgentCard::builder()
            .name("Test Agent".to_string())
            .url("ws://localhost:8081".to_string())
            .build();

        let mut server = StationServer::new(agent_card);

        // Send a message request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "test-1",
            "method": "message/send",
            "params": {
                "message": {
                    "role": "user",
                    "parts": [{"text": "Hello"}],
                    "messageId": "msg-1"
                }
            }
        });

        let response = server
            .process_request(&serde_json::to_string(&request).unwrap())
            .await;

        // Check receipt was generated
        let receipts = server.receipts.read().await;
        assert_eq!(receipts.len(), 1);

        // Verify chain integrity
        assert!(receipts.verify_integrity().is_ok());
    }
}
