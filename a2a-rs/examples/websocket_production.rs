//! Production-ready WebSocket client example
//!
//! This example demonstrates:
//! - Automatic reconnection with exponential backoff
//! - Session state tracking across reconnections
//! - Request queue for offline periods
//! - Heartbeat mechanism for connection health
//! - Complete error recovery
//!
//! Run with: cargo run --example websocket_production --features "ws-client,ws-server,tracing"

use std::time::Duration;
use tokio::time::{sleep, timeout};
use a2a_rs::{
    adapter::transport::websocket::client::{
        ConnectionStatus, HeartbeatConfig, QueueConfig, ReconnectConfig, WebSocketClient,
    },
    adapter::{
        business::{DefaultRequestProcessor, NoopPushNotificationSender, SimpleAgentInfo},
        transport::websocket::server::WebSocketServer,
    },
    domain::{
        AgentCard, AgentCapabilities, AgentSkill, ListTasksParams, Message, MessagePart,
        MessageRole, TaskPushNotificationConfig, TaskSendParams,
    },
    port::AsyncStreamingHandler,
    services::server::AsyncA2ARequestProcessor,
};
use a2a_rs::business::DefaultStreamingHandler;

/// Configuration for the example
struct Config {
    server_port: u16,
    server_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_port: 8084,
            server_url: "ws://127.0.0.1:8084/ws".to_string(),
        }
    }
}

/// Create a test agent card
fn create_agent_card() -> AgentCard {
    AgentCard {
        agent_id: "production-agent".to_string(),
        name: "Production Demo Agent".to_string(),
        description: Some("A production-ready agent demonstrating WebSocket features".to_string()),
        version: "1.0.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            push_notifications: true,
        },
        skills: vec![
            AgentSkill {
                id: "echo".to_string(),
                name: "Echo".to_string(),
                description: Some("Echoes back messages".to_string()),
                input_modes: vec!["text".to_string()],
                output_modes: vec!["text".to_string()],
            },
            AgentSkill {
                id: "summarize".to_string(),
                name: "Summarize".to_string(),
                description: Some("Summarizes text content".to_string()),
                input_modes: vec!["text".to_string()],
                output_modes: vec!["text".to_string()],
            },
        ],
        metadata: None,
    }
}

/// Start the WebSocket server
async fn start_server(port: u16) -> tokio::task::JoinHandle<()> {
    let address = format!("127.0.0.1:{}", port);

    let agent_card = create_agent_card();
    let agent_info = SimpleAgentInfo::new(agent_card);
    let processor = DefaultRequestProcessor::new();
    let push_sender = NoopPushNotificationSender;
    let streaming_handler = DefaultStreamingHandler::new();

    let server = WebSocketServer::new(processor, agent_info, streaming_handler, address);

    tokio::spawn(async move {
        println!("WebSocket server listening on: {}", address);
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    })
}

/// Demonstrate connection status monitoring
async fn monitor_connection_status(client: &WebSocketClient, duration_secs: u64) {
    println!("\n=== Monitoring Connection Status ===");

    for _ in 0..duration_secs {
        let status = client.status().await;
        let session = client.session_state().await;

        println!(
            "Status: {:?} | Session: {} | Reconnects: {} | Last activity: {:?} ago",
            status,
            session.session_id,
            session.reconnect_count,
            session.last_activity.elapsed()
        );

        sleep(Duration::from_secs(1)).await;
    }
}

/// Demonstrate automatic reconnection with exponential backoff
async fn demonstrate_reconnection(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Demonstrating Automatic Reconnection ===");

    // Configure client with aggressive reconnection for demo
    let reconnect_config = ReconnectConfig::builder()
        .enabled(true)
        .max_attempts(5)
        .initial_backoff(Duration::from_millis(100))
        .max_backoff(Duration::from_secs(5))
        .backoff_multiplier(2.0)
        .jitter_factor(0.1)
        .build();

    let mut client = WebSocketClient::new(config.server_url.clone())
        .with_reconnect_config(reconnect_config.clone())
        .with_timeout(10);

    // Start monitoring
    let monitor_client = client.clone();
    let monitor_handle = tokio::spawn(async move {
        monitor_connection_status(&monitor_client, 15).await;
    });

    // Connect initially
    println!("Connecting to server...");
    client.connect().await?;
    println!("Connected successfully!");

    // Show backoff calculation
    println!("\nBackoff schedule:");
    for attempt in 1..=5 {
        let backoff = reconnect_config.calculate_backoff(attempt);
        println!("  Attempt {}: {:?} (range: {:?} - {:?})",
            attempt,
            backoff.saturating_sub(Duration::from_millis(50)),
            backoff.saturating_add(Duration::from_millis(50))
        );
    }

    // Wait a bit
    sleep(Duration::from_secs(3)).await;

    // Simulate server restart (in real scenario, server would crash)
    println!("\nSimulating server restart scenario...");
    println!("In production, client would automatically reconnect when server comes back");

    // Monitor for a while
    sleep(Duration::from_secs(10)).await;

    // Clean shutdown
    client.close().await?;
    monitor_handle.abort();

    println!("Reconnection demonstration complete");
    Ok(())
}

/// Demonstrate session state tracking
async fn demonstrate_session_tracking(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Demonstrating Session State Tracking ===");

    let client = WebSocketClient::new(config.server_url.clone());

    // Show initial state
    println!("Initial session state:");
    let session = client.session_state().await;
    println!("  Session ID: {}", session.session_id);
    println!("  Connected at: {:?}", session.connected_at);
    println!("  Last activity: {:?}", session.last_activity);
    println!("  Reconnect count: {}", session.reconnect_count);

    // Connect
    println!("\nConnecting...");
    let mut client_mut = client.clone();
    client_mut.connect().await?;

    // Show updated state
    println!("\nConnected session state:");
    let session = client.session_state().await;
    println!("  Session ID: {}", session.session_id);
    println!("  Connected at: {:?}", session.connected_at);
    println!("  Last activity: {:?}", session.last_activity);
    println!("  Session age: {:?}", session.connected_at.elapsed());
    println!("  Idle time: {:?}", session.last_activity.elapsed());
    println!("  Reconnect count: {}", session.reconnect_count);

    // Check expiration
    let is_expired = session.is_expired(Duration::from_secs(300));
    println!("  Expired (5min timeout): {}", is_expired);

    // Disconnect
    client_mut.close().await?;

    println!("\nSession tracking demonstration complete");
    Ok(())
}

/// Demonstrate heartbeat mechanism
async fn demonstrate_heartbeat(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Demonstrating Heartbeat Mechanism ===");

    let heartbeat_config = HeartbeatConfig::builder()
        .enabled(true)
        .interval(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build();

    let client = WebSocketClient::new(config.server_url.clone())
        .with_heartbeat_config(heartbeat_config);

    println!("Heartbeat configuration:");
    println!("  Enabled: {}", heartbeat_config.enabled);
    println!("  Interval: {:?}", heartbeat_config.interval);
    println!("  Timeout: {:?}", heartbeat_config.timeout);

    println!("\nConnecting and starting heartbeat...");
    let mut client_mut = client.clone();
    client_mut.connect().await?;

    // Start background tasks (heartbeat)
    client_mut.start_background_tasks().await?;

    println!("Heartbeat is now running in the background");
    println!("Client will send ping every 5 seconds");
    println!("Client will consider connection dead if no activity for 10 seconds");

    // Monitor for a while
    let monitor_client = client.clone();
    let monitor_handle = tokio::spawn(async move {
        for i in 1..=6 {
            sleep(Duration::from_secs(5)).await;
            let status = monitor_client.status().await;
            let session = monitor_client.session_state().await;
            println!("[{}] Status: {:?}, Last activity: {:?} ago",
                i, status, session.last_activity.elapsed());
        }
    });

    timeout(Duration::from_secs(35), monitor_handle).await.ok();
    client_mut.close().await?;

    println!("\nHeartbeat demonstration complete");
    Ok(())
}

/// Demonstrate request queue
async fn demonstrate_request_queue(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Demonstrating Request Queue ===");

    let queue_config = QueueConfig::builder()
        .enabled(true)
        .max_size(100)
        .max_age(Duration::from_secs(300))
        .max_retries(3)
        .build();

    println!("Queue configuration:");
    println!("  Enabled: {}", queue_config.enabled);
    println!("  Max size: {}", queue_config.max_size);
    println!("  Max age: {:?}", queue_config.max_age);
    println!("  Max retries: {}", queue_config.max_retries);

    let client = WebSocketClient::new(config.server_url.clone())
        .with_queue_config(queue_config)
        .with_timeout(10);

    println!("\nConnecting...");
    let mut client_mut = client.clone();
    client_mut.connect().await?;

    // In a real scenario with server disconnection:
    println!("\nNote: In production, if the server disconnects:");
    println!("  1. Client will queue outgoing requests");
    println!("  2. Requests will be held for up to 5 minutes");
    println!("  3. Each request will retry up to 3 times");
    println!("  4. Queue holds max 100 requests");
    println!("  5. Upon reconnection, queued requests are sent automatically");

    client_mut.close().await?;

    println!("\nRequest queue demonstration complete");
    Ok(())
}

/// Demonstrate basic WebSocket operations
async fn demonstrate_basic_operations(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Demonstrating Basic Operations ===");

    let client = WebSocketClient::new(config.server_url.clone())
        .with_timeout(30);

    println!("Connecting to {}", config.server_url);
    let mut client_mut = client.clone();
    client_mut.connect().await?;

    // Check connection status
    let status = client.status().await;
    println!("Connection status: {:?}", status);

    let session = client.session_state().await;
    println!("Session ID: {}", session.session_id);

    // List tasks (should be empty initially)
    println!("\nListing tasks...");
    match client.list_tasks(&ListTasksParams::default()).await {
        Ok(result) => {
            println!("Found {} tasks", result.tasks.len());
        }
        Err(e) => {
            println!("Error listing tasks: {}", e);
        }
    }

    client_mut.close().await?;

    println!("\nBasic operations demonstration complete");
    Ok(())
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Production WebSocket Client Example ===\n");

    let config = Config::default();

    // Start server
    println!("Starting WebSocket server...");
    let server_handle = start_server(config.server_port).await;
    sleep(Duration::from_millis(500)).await; // Wait for server to be ready

    // Run demonstrations
    demonstrate_basic_operations(config.clone()).await?;
    demonstrate_session_tracking(config.clone()).await?;
    demonstrate_reconnection(config.clone()).await?;
    demonstrate_heartbeat(config.clone()).await?;
    demonstrate_request_queue(config.clone()).await?;

    println!("\n=== All demonstrations complete ===");

    // Shutdown server
    server_handle.abort();
    sleep(Duration::from_millis(100)).await;

    Ok(())
}

/// Tips for production use:
///
/// 1. Reconnection Configuration:
///    - Start with `initial_backoff` of 100-500ms
///    - Use `backoff_multiplier` of 2.0 for exponential backoff
///    - Set `max_backoff` to 30-60 seconds to avoid long waits
///    - Enable `jitter_factor` (0.1) to prevent thundering herd
///    - Set `max_attempts` based on your tolerance (5-10 is typical)
///
/// 2. Heartbeat Configuration:
///    - Set `interval` to 15-30 seconds for most applications
///    - Set `timeout` to 2-3x the interval
///    - Shorter intervals = faster failure detection but more overhead
///    - Longer intervals = less overhead but slower failure detection
///
/// 3. Queue Configuration:
///    - Set `max_size` based on memory constraints (100-1000)
///    - Set `max_age` based on request freshness needs (60-600 seconds)
///    - Set `max_retries` based on idempotency (1-3 for mutations, more for queries)
///
/// 4. Session Timeout:
///    - Set `session_timeout` longer than heartbeat timeout (2-3x)
///    - This allows for temporary network issues without reconnection
///
/// 5. Error Handling:
///    - Monitor `ConnectionStatus` for connection state changes
///    - Handle `WebSocketClientError::ReconnectionFailed` explicitly
///    - Implement circuit breakers if reconnections fail repeatedly
///    - Log all errors with sufficient context for debugging
///
/// 6. Production Best Practices:
///    - Always start background tasks with `start_background_tasks()`
///    - Monitor session state periodically
///    - Implement graceful shutdown with `close()`
///    - Use connection pooling for multiple clients
///    - Add metrics for connection health and performance
