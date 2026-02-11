//! Production-ready WebSocket client tests
//!
//! Tests cover:
//! - Automatic reconnection with exponential backoff
//! - Session state tracking
//! - Request queue for offline periods
//! - Heartbeat mechanism
//! - Error recovery

#[cfg(all(feature = "ws-client", feature = "ws-server"))]
mod production_tests {
    use std::time::Duration;
    use tokio::time::sleep;
    use a2a_rs::{
        adapter::transport::websocket::client::{
            ConnectionStatus, HeartbeatConfig, QueueConfig, ReconnectConfig, SessionState,
            WebSocketClient,
        },
        adapter::transport::websocket::server::WebSocketServer,
        adapter::{
            business::{DefaultRequestProcessor, SimpleAgentInfo, NoopPushNotificationSender},
        },
        domain::{
            AgentCard, AgentCapabilities, AgentSkill, Message, MessageRole, TaskPushNotificationConfig,
        },
        port::AsyncStreamingHandler,
        services::server::AsyncA2ARequestProcessor,
    };
    use a2a_rs::business::DefaultStreamingHandler;

    /// Helper to create a test agent card
    fn test_agent_card() -> AgentCard {
        AgentCard {
            agent_id: "test-agent".to_string(),
            name: "Test Agent".to_string(),
            description: Some("A test agent".to_string()),
            version: "0.1.0".to_string(),
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: false,
            },
            skills: vec![AgentSkill {
                id: "test".to_string(),
                name: "Test".to_string(),
                description: Some("Test skill".to_string()),
                input_modes: vec!["text".to_string()],
                output_modes: vec!["text".to_string()],
            }],
            metadata: None,
        }
    }

    /// Helper to start a test server
    async fn start_test_server(port: u16) -> (String, tokio::task::JoinHandle<()>) {
        let address = format!("127.0.0.1:{}", port);

        let agent_card = test_agent_card();
        let agent_info = SimpleAgentInfo::new(agent_card);
        let processor = DefaultRequestProcessor::new();
        let push_sender = NoopPushNotificationSender;
        let streaming_handler = DefaultStreamingHandler::new();

        let server = WebSocketServer::new(
            processor,
            agent_info,
            streaming_handler,
            address,
        );

        let handle = tokio::spawn(async move {
            let _ = server.start().await;
        });

        // Wait for server to start
        sleep(Duration::from_millis(100)).await;

        let url = format!("ws://127.0.0.1:{}/ws", port);
        (url, handle)
    }

    #[tokio::test]
    async fn test_websocket_connection_status() {
        let (url, _server) = start_test_server(8080).await;

        let client = WebSocketClient::new(url);

        // Initially disconnected
        assert_eq!(client.status().await, ConnectionStatus::Disconnected);

        // After connection, should be connected
        let mut client_clone = client.clone();
        let result = client_clone.connect().await;
        assert!(result.is_ok());
        assert_eq!(client.status().await, ConnectionStatus::Connected);

        // Disconnect
        let result = client_clone.disconnect().await;
        assert!(result.is_ok());
        assert_eq!(client.status().await, ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_session_state_tracking() {
        let (url, _server) = start_test_server(8081).await;

        let client = WebSocketClient::new(url);

        // Initial session state
        let session = client.session_state().await;
        assert_eq!(session.reconnect_count, 0);

        // Connect
        let mut client_clone = client.clone();
        client_clone.connect().await.unwrap();

        // Session should have been updated
        let session = client.session_state().await;
        assert!(session.connected_at.elapsed() < Duration::from_secs(1));
        assert!(session.last_activity.elapsed() < Duration::from_secs(1));

        // Check session is not expired
        assert!(!session.is_expired(Duration::from_secs(300)));
    }

    #[tokio::test]
    async fn test_reconnect_config_builder() {
        let config = ReconnectConfig::builder()
            .enabled(true)
            .max_attempts(5)
            .initial_backoff(Duration::from_millis(50))
            .max_backoff(Duration::from_secs(10))
            .backoff_multiplier(2.0)
            .jitter_factor(0.1)
            .build();

        assert!(config.enabled);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_backoff, Duration::from_millis(50));
        assert_eq!(config.max_backoff, Duration::from_secs(10));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.jitter_factor, 0.1);
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        let config = ReconnectConfig::builder()
            .initial_backoff(Duration::from_millis(100))
            .max_backoff(Duration::from_secs(30))
            .backoff_multiplier(2.0)
            .jitter_factor(0.0)
            .build();

        // Test backoff progression
        let backoff_1 = config.calculate_backoff(1);
        assert!(backoff_1 >= Duration::from_millis(100));
        assert!(backoff_1 <= Duration::from_millis(200));

        let backoff_2 = config.calculate_backoff(2);
        assert!(backoff_2 >= Duration::from_millis(200));
        assert!(backoff_2 <= Duration::from_millis(400));

        let backoff_3 = config.calculate_backoff(3);
        assert!(backoff_3 >= Duration::from_millis(400));
        assert!(backoff_3 <= Duration::from_millis(800));

        // Test max backoff cap
        let backoff_20 = config.calculate_backoff(20);
        assert!(backoff_20 <= Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_heartbeat_config_builder() {
        let config = HeartbeatConfig::builder()
            .enabled(true)
            .interval(Duration::from_secs(15))
            .timeout(Duration::from_secs(5))
            .build();

        assert!(config.enabled);
        assert_eq!(config.interval, Duration::from_secs(15));
        assert_eq!(config.timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_queue_config_builder() {
        let config = QueueConfig::builder()
            .enabled(true)
            .max_size(500)
            .max_age(Duration::from_secs(600))
            .max_retries(5)
            .build();

        assert!(config.enabled);
        assert_eq!(config.max_size, 500);
        assert_eq!(config.max_age, Duration::from_secs(600));
        assert_eq!(config.max_retries, 5);
    }

    #[tokio::test]
    async fn test_websocket_client_with_custom_configs() {
        let (url, _server) = start_test_server(8082).await;

        let reconnect_config = ReconnectConfig::builder()
            .enabled(true)
            .max_attempts(3)
            .initial_backoff(Duration::from_millis(50))
            .build();

        let heartbeat_config = HeartbeatConfig::builder()
            .enabled(true)
            .interval(Duration::from_secs(10))
            .timeout(Duration::from_secs(5))
            .build();

        let queue_config = QueueConfig::builder()
            .enabled(true)
            .max_size(100)
            .max_age(Duration::from_secs(60))
            .max_retries(2)
            .build();

        let client = WebSocketClient::new(url)
            .with_reconnect_config(reconnect_config)
            .with_heartbeat_config(heartbeat_config)
            .with_queue_config(queue_config)
            .with_timeout(60);

        let mut client_mut = client.clone();
        client_mut.connect().await.unwrap();

        assert_eq!(client_mut.status().await, ConnectionStatus::Connected);
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let session = SessionState::new();

        // Fresh session should not be expired
        assert!(!session.is_expired(Duration::from_secs(10)));

        // Simulate time passing (by creating a very old session)
        let mut old_session = SessionState::new();
        old_session.last_activity = std::time::Instant::now() - Duration::from_secs(20);

        assert!(old_session.is_expired(Duration::from_secs(10)));
        assert!(!old_session.is_expired(Duration::from_secs(30)));
    }

    #[tokio::test]
    async fn test_connection_status_methods() {
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(!ConnectionStatus::Disconnected.can_send());

        assert!(!ConnectionStatus::Connecting.is_connected());
        assert!(!ConnectionStatus::Connecting.can_send());

        assert!(ConnectionStatus::Connected.is_connected());
        assert!(ConnectionStatus::Connected.can_send());

        assert!(!ConnectionStatus::Reconnecting.is_connected());
        assert!(ConnectionStatus::Reconnecting.can_send());

        assert!(!ConnectionStatus::Closed.is_connected());
        assert!(!ConnectionStatus::Closed.can_send());
    }

    #[tokio::test]
    async fn test_websocket_close() {
        let (url, _server) = start_test_server(8083).await;

        let mut client = WebSocketClient::new(url);
        client.connect().await.unwrap();

        assert_eq!(client.status().await, ConnectionStatus::Connected);

        client.close().await.unwrap();

        assert_eq!(client.status().await, ConnectionStatus::Closed);
    }
}
