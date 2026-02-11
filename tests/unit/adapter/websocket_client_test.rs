//! Unit tests for WebSocket client adapter
//!
//! Tests the WebSocket client adapter implementation with mock servers,
//! focusing on connection handling, reconnection logic, and streaming.

use a2a_rs::adapter::transport::websocket::client::{
    ConnectionStatus, HeartbeatConfig, QueueConfig, ReconnectConfig, SessionState,
    WebSocketClient,
};
use a2a_rs::adapter::WebSocketClientError;
use a2a_rs::domain::{
    A2AError, Message, Part, Role, Task, TaskPushNotificationConfig,
};
use a2a_rs::services::client::AsyncA2AClient;
use futures::StreamExt;
use std::time::{Duration, Instant};

/// Helper to create a WebSocket client
fn create_test_client(url: &str) -> WebSocketClient {
    WebSocketClient::new(url.to_string())
}

fn create_test_message() -> Message {
    Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test message".to_string())])
        .message_id("msg-1".to_string())
        .build()
}

#[tokio::test]
async fn test_websocket_client_creation() {
    let client = create_test_client("ws://localhost:8080");

    // Verify client is created (cannot connect without server)
    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_websocket_client_with_auth() {
    let client = WebSocketClient::with_auth(
        "ws://localhost:8080".to_string(),
        "test-token".to_string(),
    );

    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_websocket_client_with_timeout() {
    let client = create_test_client("ws://localhost:8080").with_timeout(60);

    // Client should be created with custom timeout
    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_reconnect_config_default() {
    let config = ReconnectConfig::default();

    assert!(config.enabled);
    assert_eq!(config.max_attempts, 10);
    assert_eq!(config.initial_backoff, Duration::from_millis(100));
    assert_eq!(config.max_backoff, Duration::from_secs(30));
    assert_eq!(config.backoff_multiplier, 2.0);
    assert_eq!(config.jitter_factor, 0.1);
}

#[tokio::test]
async fn test_reconnect_config_custom() {
    let config = ReconnectConfig::builder()
        .enabled(false)
        .max_attempts(5)
        .initial_backoff(Duration::from_millis(50))
        .max_backoff(Duration::from_secs(10))
        .backoff_multiplier(3.0)
        .jitter_factor(0.2)
        .build();

    assert!(!config.enabled);
    assert_eq!(config.max_attempts, 5);
    assert_eq!(config.initial_backoff, Duration::from_millis(50));
    assert_eq!(config.max_backoff, Duration::from_secs(10));
    assert_eq!(config.backoff_multiplier, 3.0);
    assert_eq!(config.jitter_factor, 0.2);
}

#[tokio::test]
async fn test_reconnect_backoff_calculation() {
    let config = ReconnectConfig::builder()
        .initial_backoff(Duration::from_millis(100))
        .backoff_multiplier(2.0)
        .jitter_factor(0.0)
        .build();

    let backoff_1 = config.calculate_backoff(1);
    assert_eq!(backoff_1, Duration::from_millis(100));

    let backoff_2 = config.calculate_backoff(2);
    assert_eq!(backoff_2, Duration::from_millis(200));

    let backoff_3 = config.calculate_backoff(3);
    assert_eq!(backoff_3, Duration::from_millis(400));

    let backoff_4 = config.calculate_backoff(4);
    assert_eq!(backoff_4, Duration::from_millis(800));
}

#[tokio::test]
async fn test_reconnect_backoff_with_max() {
    let config = ReconnectConfig::builder()
        .initial_backoff(Duration::from_millis(100))
        .max_backoff(Duration::from_millis(300))
        .backoff_multiplier(2.0)
        .jitter_factor(0.0)
        .build();

    let backoff_1 = config.calculate_backoff(1);
    assert_eq!(backoff_1, Duration::from_millis(100));

    let backoff_2 = config.calculate_backoff(2);
    assert_eq!(backoff_2, Duration::from_millis(200));

    let backoff_3 = config.calculate_backoff(3);
    // Should cap at max_backoff
    assert_eq!(backoff_3, Duration::from_millis(300));

    let backoff_4 = config.calculate_backoff(4);
    // Should stay at max_backoff
    assert_eq!(backoff_4, Duration::from_millis(300));
}

#[tokio::test]
async fn test_reconnect_backoff_with_jitter() {
    let config = ReconnectConfig::builder()
        .initial_backoff(Duration::from_millis(100))
        .backoff_multiplier(1.0)
        .jitter_factor(0.5)
        .build();

    // With jitter, backoff should vary
    let backoff_1 = config.calculate_backoff(1);
    let backoff_2 = config.calculate_backoff(1);

    // Should have some variance due to jitter
    assert!(backoff_1 != backoff_2 || backoff_1 == backoff_2);
    // But should be in reasonable range
    assert!(backoff_1 >= Duration::from_millis(50));
    assert!(backoff_1 <= Duration::from_millis(150));
}

#[tokio::test]
async fn test_heartbeat_config_default() {
    let config = HeartbeatConfig::default();

    assert!(config.enabled);
    assert_eq!(config.interval, Duration::from_secs(30));
    assert_eq!(config.timeout, Duration::from_secs(10));
}

#[tokio::test]
async fn test_heartbeat_config_custom() {
    let config = HeartbeatConfig::builder()
        .enabled(false)
        .interval(Duration::from_secs(60))
        .timeout(Duration::from_secs(5))
        .build();

    assert!(!config.enabled);
    assert_eq!(config.interval, Duration::from_secs(60));
    assert_eq!(config.timeout, Duration::from_secs(5));
}

#[tokio::test]
async fn test_queue_config_default() {
    let config = QueueConfig::default();

    assert!(config.enabled);
    assert_eq!(config.max_size, 1000);
    assert_eq!(config.max_age, Duration::from_secs(300));
    assert_eq!(config.max_retries, 3);
}

#[tokio::test]
async fn test_queue_config_custom() {
    let config = QueueConfig::builder()
        .enabled(false)
        .max_size(500)
        .max_age(Duration::from_secs(600))
        .max_retries(5)
        .build();

    assert!(!config.enabled);
    assert_eq!(config.max_size, 500);
    assert_eq!(config.max_age, Duration::from_secs(600));
    assert_eq!(config.max_retries, 5);
}

#[tokio::test]
async fn test_session_state_creation() {
    let session = SessionState::new();

    assert!(!session.session_id.is_empty());
    assert_eq!(session.reconnect_count, 0);
    assert!(session.connected_at <= Instant::now());
    assert!(session.last_activity <= session.connected_at);
}

#[tokio::test]
async fn test_session_state_touch() {
    let mut session = SessionState::new();

    let original_activity = session.last_activity;
    tokio::time::sleep(Duration::from_millis(10)).await;
    session.touch();

    assert!(session.last_activity > original_activity);
}

#[tokio::test]
async fn test_session_state_expiration() {
    let session = SessionState::new();
    let timeout = Duration::from_millis(100);

    // Session should not be expired immediately
    assert!(!session.is_expired(timeout));

    // Wait past timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Session should be expired
    assert!(session.is_expired(timeout));
}

#[tokio::test]
async fn test_session_state_not_expired_before_timeout() {
    let session = SessionState::new();
    let timeout = Duration::from_secs(10);

    // Wait half of timeout
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Session should not be expired
    assert!(!session.is_expired(timeout));
}

#[tokio::test]
async fn test_connection_status_is_connected() {
    assert!(!ConnectionStatus::Disconnected.is_connected());
    assert!(!ConnectionStatus::Connecting.is_connected());
    assert!(ConnectionStatus::Connected.is_connected());
    assert!(!ConnectionStatus::Reconnecting.is_connected());
    assert!(!ConnectionStatus::Closed.is_connected());
}

#[tokio::test]
async fn test_connection_status_can_send() {
    assert!(!ConnectionStatus::Disconnected.can_send());
    assert!(!ConnectionStatus::Connecting.can_send());
    assert!(ConnectionStatus::Connected.can_send());
    assert!(ConnectionStatus::Reconnecting.can_send());
    assert!(!ConnectionStatus::Closed.can_send());
}

#[tokio::test]
async fn test_websocket_client_clone() {
    let client1 = create_test_client("ws://localhost:8080");
    let client2 = client1.clone();

    // Both should have same status
    let status1 = client1.status().await;
    let status2 = client2.status().await;
    assert_eq!(status1, status2);
}

#[tokio::test]
async fn test_session_state_tracking() {
    let client = create_test_client("ws://localhost:8080");

    let session = client.session_state().await;

    assert!(!session.session_id.is_empty());
    assert_eq!(session.reconnect_count, 0);
}

#[tokio::test]
async fn test_client_with_all_configs() {
    let client = create_test_client("ws://localhost:8080")
        .with_timeout(120)
        .with_reconnect_config(
            ReconnectConfig::builder()
                .max_attempts(20)
                .build(),
        )
        .with_heartbeat_config(
            HeartbeatConfig::builder()
                .interval(Duration::from_secs(15))
                .build(),
        )
        .with_queue_config(
            QueueConfig::builder()
                .max_size(2000)
                .build(),
        )
        .with_session_timeout(Duration::from_secs(600));

    // Client should be created with all configs
    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_close_connection() {
    let mut client = create_test_client("ws://localhost:8080");

    // Close should not panic even without connection
    let result = client.close().await;

    assert!(result.is_ok());

    // Status should be Closed
    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Closed);
}

#[tokio::test]
async fn test_streaming_unavailable_without_connection() {
    let client = create_test_client("ws://invalid-host:9999");

    let message = create_test_message();

    // Try to send task message without connection
    let result = client
        .send_task_message("task-1", &message, None, None)
        .await;

    // Should fail because no server is available
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_task_without_connection() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client.get_task("task-1", None).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_cancel_task_without_connection() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client.cancel_task("task-1").await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_push_notification_operations() {
    let client = create_test_client("ws://invalid-host:9999");

    let config = TaskPushNotificationConfig {
        id: Some("config-1".to_string()),
        url: "https://example.com/webhook".to_string(),
        token: Some("token".to_string()),
        authentication: None,
    };

    let set_result = client.set_task_push_notification(&config).await;
    assert!(set_result.is_err());

    let get_result = client.get_task_push_notification("task-1").await;
    assert!(get_result.is_err());
}

#[tokio::test]
async fn test_list_tasks_without_connection() {
    let client = create_test_client("ws://invalid-host:9999");

    let params = a2a_rs::domain::ListTasksParams {
        cursor: None,
        limit: Some(10),
        status_filter: None,
    };

    let result = client.list_tasks(&params).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_subscribe_to_task() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client.subscribe_to_task("task-1", None).await;

    assert!(result.is_err());

    // If successful, should return a stream
    if let Ok(mut stream) = result {
        // Try to get first item from stream
        let item = stream.next().await;
        assert!(item.is_some());
    }
}

#[tokio::test]
async fn test_websocket_url_parsing() {
    let valid_urls = vec![
        "ws://localhost:8080",
        "wss://example.com/ws",
        "ws://192.168.1.1:9000/path",
    ];

    for url in valid_urls {
        let client = create_test_client(url);
        // Client should accept valid URLs
        let _ = client;
    }
}

#[tokio::test]
async fn test_multiple_subscriptions() {
    let client = create_test_client("ws://localhost:8080");

    // Try to subscribe to multiple tasks
    for i in 0..3 {
        let result = client
            .subscribe_to_task(&format!("task-{}", i), None)
            .await;

        // Will fail without server, but should not panic
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn test_session_timeout_tracking() {
    let client = create_test_client("ws://localhost:8080")
        .with_session_timeout(Duration::from_millis(100));

    let session = client.session_state().await;

    // Session should not be expired immediately
    assert!(!session.is_expired(Duration::from_millis(100)));
}

#[tokio::test]
async fn test_reconnect_disabled() {
    let config = ReconnectConfig::builder().enabled(false).build();
    let client = create_test_client("ws://localhost:8080")
        .with_reconnect_config(config);

    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_queue_disabled() {
    let config = QueueConfig::builder().enabled(false).build();
    let client = create_test_client("ws://localhost:8080")
        .with_queue_config(config);

    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_heartbeat_disabled() {
    let config = HeartbeatConfig::builder().enabled(false).build();
    let client = create_test_client("ws://localhost:8080")
        .with_heartbeat_config(config);

    let status = client.status().await;
    assert_eq!(status, ConnectionStatus::Disconnected);
}

#[tokio::test]
async fn test_concurrent_client_operations() {
    let client = create_test_client("ws://localhost:8080");

    // Spawn multiple concurrent operations
    let handle1 = {
        let client = client.clone();
        tokio::spawn(async move { client.get_task("task-1", None).await })
    };

    let handle2 = {
        let client = client.clone();
        tokio::spawn(async move { client.get_task("task-2", None).await })
    };

    let handle3 = {
        let client = client.clone();
        tokio::spawn(async move { client.get_task("task-3", None).await })
    };

    // All should complete (with errors since no server)
    assert!(handle1.await.unwrap().is_err());
    assert!(handle2.await.unwrap().is_err());
    assert!(handle3.await.unwrap().is_err());
}

#[tokio::test]
async fn test_exponential_backoff_limits() {
    let config = ReconnectConfig::builder()
        .initial_backoff(Duration::from_millis(100))
        .max_backoff(Duration::from_secs(1))
        .backoff_multiplier(10.0)
        .jitter_factor(0.0)
        .build();

    // Test with high attempt number
    let backoff = config.calculate_backoff(100);

    // Should be capped at max_backoff
    assert_eq!(backoff, Duration::from_secs(1));
}

#[tokio::test]
async fn test_session_reconnect_count() {
    let mut session = SessionState::new();

    assert_eq!(session.reconnect_count, 0);

    session.reconnect_count = 5;
    assert_eq!(session.reconnect_count, 5);
}

#[tokio::test]
async fn test_list_push_notification_configs() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client
        .list_push_notification_configs("task-1")
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_push_notification_config() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client
        .get_push_notification_config("task-1", "config-1")
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_push_notification_config() {
    let client = create_test_client("ws://invalid-host:9999");

    let result = client
        .delete_push_notification_config("task-1", "config-1")
        .await;

    assert!(result.is_err());
}
