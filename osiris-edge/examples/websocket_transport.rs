//! Example demonstrating WebSocket transport with bidirectional streaming and reconnection.
//!
//! Run with: cargo run --example websocket_transport --features ws

#[cfg(feature = "ws")]
use chrono::Utc;
#[cfg(feature = "ws")]
use osiris_edge::{
    WebSocketTransport,
    domain::{Attendee, EventType, PacketContext, PacketPayload, PacketSource, TypedPacket},
    port::{Transport, TransportConfig},
};
#[cfg(feature = "ws")]
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "ws")]
    {
        // Initialize tracing
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();

        println!("=== WebSocket Transport Example ===\n");

        // Configure transport
        let config = TransportConfig::new("ws://echo.websocket.org")
            .with_ping_interval(Duration::from_secs(15))
            .with_pong_timeout(Duration::from_secs(5))
            .with_reconnect_config(Duration::from_millis(100), Duration::from_secs(10), 2.0)
            .with_max_reconnect_attempts(Some(5));

        println!("Configuration:");
        println!("  URL: {}", config.url);
        println!("  Ping interval: {:?}", config.ping_interval);
        println!("  Pong timeout: {:?}", config.pong_timeout);
        println!(
            "  Max reconnect attempts: {:?}",
            config.max_reconnect_attempts
        );
        println!();

        // Create transport
        let mut transport = WebSocketTransport::new(config);

        println!("Initial status: {:?}", transport.status());

        // Example: Create a typed packet to send
        let packet = TypedPacket::new(
            PacketSource::Calendar {
                event_id: "event123".to_string(),
                calendar_id: "calendar@example.com".to_string(),
                status: "confirmed".to_string(),
            },
            PacketPayload::CalendarEvent {
                title: "Team Standup".to_string(),
                description: Some("Daily sync".to_string()),
                start_time: Utc::now(),
                end_time: Utc::now() + chrono::Duration::minutes(30),
                location: Some("Conference Room A".to_string()),
                attendees: vec![
                    Attendee {
                        email: "alice@example.com".to_string(),
                        display_name: Some("Alice".to_string()),
                        response_status: "accepted".to_string(),
                        is_organizer: true,
                        is_optional: false,
                    },
                    Attendee {
                        email: "bob@example.com".to_string(),
                        display_name: Some("Bob".to_string()),
                        response_status: "tentative".to_string(),
                        is_organizer: false,
                        is_optional: false,
                    },
                ],
                meeting_link: Some("https://meet.google.com/abc-defg-hij".to_string()),
            },
            PacketContext {
                user_id: "user123".to_string(),
                workspace_domain: "example.com".to_string(),
                event_type: EventType::Created,
                raw_webhook_data: None,
                metadata: Default::default(),
            },
        );

        println!("Created packet: {} ({})", packet.id, packet.packet_type());
        println!();

        // Demonstrate configuration builder pattern
        println!("=== Configuration Builder Pattern ===");
        let custom_config = TransportConfig::default()
            .with_ping_interval(Duration::from_secs(20))
            .with_pong_timeout(Duration::from_secs(8))
            .with_buffer_size(512);

        println!("Custom config buffer size: {}", custom_config.buffer_size);
        println!();

        // Show transport status
        println!("=== Transport Status ===");
        println!("Is connected: {}", transport.is_connected());
        println!("Status: {:?}", transport.status());
        println!();

        println!("Note: This example demonstrates configuration and packet creation.");
        println!("Full connection would require a WebSocket server endpoint.");
        println!("See the Transport trait for async methods like:");
        println!("  - connect(): Establish connection");
        println!("  - send(packet): Send typed packet");
        println!("  - receive(): Get next packet");
        println!("  - reconnect(): Automatic exponential backoff");
        println!("  - disconnect(): Graceful close");

        Ok(())
    }

    #[cfg(not(feature = "ws"))]
    {
        eprintln!(
            "This example requires the 'ws' feature. Run with:\n  \
             cargo run --example websocket_transport --features ws"
        );
        Ok(())
    }
}
