//! Quick compilation test for WebSocket client

#[cfg(all(feature = "ws-client"))]
mod quick_test {
    use a2a_rs::adapter::transport::websocket::client::{
        ConnectionStatus, HeartbeatConfig, QueueConfig, ReconnectConfig, WebSocketClient,
    };

    #[test]
    fn test_config_builders() {
        let reconnect = ReconnectConfig::builder()
            .enabled(true)
            .max_attempts(5)
            .build();

        assert!(reconnect.enabled);
        assert_eq!(reconnect.max_attempts, 5);

        let heartbeat = HeartbeatConfig::builder()
            .enabled(true)
            .interval(std::time::Duration::from_secs(30))
            .build();

        assert!(heartbeat.enabled);

        let queue = QueueConfig::builder()
            .enabled(true)
            .max_size(100)
            .build();

        assert!(queue.enabled);
        assert_eq!(queue.max_size, 100);

        // Test connection status
        assert!(!ConnectionStatus::Disconnected.is_connected());
        assert!(ConnectionStatus::Connected.is_connected());
    }
}
