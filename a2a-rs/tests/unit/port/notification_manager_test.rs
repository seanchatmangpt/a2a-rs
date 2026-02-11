//! Unit tests for AsyncNotificationManager port trait
//!
//! Tests the contract and behavior of the AsyncNotificationManager port trait
//! using mock implementations.

use a2a_rs::domain::core::agent::PushNotificationConfig;
use a2a_rs::domain::core::task::TaskPushNotificationConfig;
use a2a_rs::domain::error::A2AError;
use a2a_rs::port::notification_manager::AsyncNotificationManager;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock implementation of AsyncNotificationManager for testing
#[derive(Debug, Clone)]
struct MockNotificationManager {
    configs: Arc<RwLock<HashMap<String, TaskPushNotificationConfig>>>,
}

impl MockNotificationManager {
    fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn has_any_config(&self) -> bool {
        !self.configs.read().await.is_empty()
    }
}

#[async_trait]
impl AsyncNotificationManager for MockNotificationManager {
    async fn set_task_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let mut configs = self.configs.write().await;
        configs.insert(config.task_id.clone(), config.clone());
        Ok(config.clone())
    }

    async fn get_task_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let configs = self.configs.read().await;
        configs
            .get(task_id)
            .cloned()
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))
    }

    async fn remove_task_notification<'a>(&self, task_id: &'a str) -> Result<(), A2AError> {
        let mut configs = self.configs.write().await;
        configs
            .remove(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;
        Ok(())
    }
}

fn create_test_config(task_id: &str, url: &str) -> TaskPushNotificationConfig {
    TaskPushNotificationConfig {
        task_id: task_id.to_string(),
        push_notification_config: PushNotificationConfig {
            url: url.to_string(),
            token: None,
        },
    }
}

#[tokio::test]
async fn test_set_task_notification() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-1", "https://example.com/webhook");
    let result = manager.set_task_notification(&config).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().task_id, "task-1");
    assert!(manager.has_any_config().await);
}

#[tokio::test]
async fn test_get_task_notification() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-2", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let result = manager.get_task_notification("task-2").await;

    assert!(result.is_ok());
    let retrieved = result.unwrap();
    assert_eq!(retrieved.task_id, "task-2");
    assert_eq!(retrieved.push_notification_config.url, "https://example.com/webhook");
}

#[tokio::test]
async fn test_get_task_notification_not_found() {
    let manager = MockNotificationManager::new();

    let result = manager.get_task_notification("nonexistent").await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_)));
    assert_eq!(result.unwrap_err().to_string(), "Task not found: nonexistent");
}

#[tokio::test]
async fn test_remove_task_notification() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-3", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let result = manager.remove_task_notification("task-3").await;

    assert!(result.is_ok());

    // Verify it's actually removed
    let get_result = manager.get_task_notification("task-3").await;
    assert!(matches!(get_result, Err(A2AError::TaskNotFound(_))));
}

#[tokio::test]
async fn test_remove_task_notification_not_found() {
    let manager = MockNotificationManager::new();

    let result = manager.remove_task_notification("nonexistent").await;

    assert!(matches!(result, Err(A2AError::TaskNotFound(_)));
}

#[tokio::test]
async fn test_has_task_notification_true() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-4", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let result = manager.has_task_notification("task-4").await;

    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[tokio::test]
async fn test_has_task_notification_false() {
    let manager = MockNotificationManager::new();

    let result = manager.has_task_notification("nonexistent").await;

    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_validate_notification_config_success() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "https://example.com/webhook".to_string(),
        token: None,
    };

    let result = manager.validate_notification_config(&config).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_notification_config_empty_url() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "".to_string(),
        token: None,
    };

    let result = manager.validate_notification_config(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "url");
        assert!(message.contains("cannot be empty"));
    }
}

#[tokio::test]
async fn test_validate_notification_config_whitespace_url() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "   ".to_string(),
        token: None,
    };

    let result = manager.validate_notification_config(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_validate_notification_config_invalid_url() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "not-a-valid-url".to_string(),
        token: None,
    };

    let result = manager.validate_notification_config(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "url");
        assert!(message.contains("Invalid webhook URL format"));
    }
}

#[tokio::test]
async fn test_validate_notification_config_valid_urls() {
    let manager = MockNotificationManager::new();

    let valid_urls = vec![
        "https://example.com/webhook",
        "https://example.com:8080/webhook",
        "https://api.example.com/v1/callback",
        "http://localhost:3000/webhook",
    ];

    for url in valid_urls {
        let config = PushNotificationConfig {
            url: url.to_string(),
            token: None,
        };

        let result = manager.validate_notification_config(&config).await;
        assert!(result.is_ok(), "URL {} should be valid", url);
    }
}

#[tokio::test]
async fn test_send_test_notification() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "https://example.com/webhook".to_string(),
        token: None,
    };

    let result = manager.send_test_notification(&config).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_test_notification_invalid_config() {
    let manager = MockNotificationManager::new();

    let config = PushNotificationConfig {
        url: "".to_string(),
        token: None,
    };

    let result = manager.send_test_notification(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_set_task_notification_validated() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-5", "https://example.com/webhook");

    let result = manager.set_task_notification_validated(&config).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_set_task_notification_validated_empty_task_id() {
    let manager = MockNotificationManager::new();

    let mut config = create_test_config("", "https://example.com/webhook");

    let result = manager.set_task_notification_validated(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
    if let Err(A2AError::ValidationError { field, message }) = result {
        assert_eq!(field, "task_id");
        assert!(message.contains("cannot be empty"));
    }
}

#[tokio::test]
async fn test_set_task_notification_validated_invalid_url() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-6", "invalid-url");

    let result = manager.set_task_notification_validated(&config).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_get_task_notification_validated() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-7", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let params = a2a_rs::domain::core::task::TaskIdParams {
        id: "task-7".to_string(),
        metadata: None,
    };

    let result = manager.get_task_notification_validated(&params).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().task_id, "task-7");
}

#[tokio::test]
async fn test_get_task_notification_validated_empty_id() {
    let manager = MockNotificationManager::new();

    let params = a2a_rs::domain::core::task::TaskIdParams {
        id: "".to_string(),
        metadata: None,
    };

    let result = manager.get_task_notification_validated(&params).await;

    assert!(matches!(result, Err(A2AError::ValidationError { .. })));
}

#[tokio::test]
async fn test_notify_task_status_update() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-8", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let status_event = a2a_rs::domain::core::task::TaskStatusUpdateEvent {
        task_id: "task-8".to_string(),
        context_id: "context-8".to_string(),
        status: a2a_rs::domain::core::task::TaskState::Completed,
        final_: true,
        timestamp: Some(chrono::Utc::now()),
    };

    let result = manager.notify_task_status_update("task-8", &status_event).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notify_task_status_update_no_notification() {
    let manager = MockNotificationManager::new();

    // Task doesn't have notification configured
    let status_event = a2a_rs::domain::core::task::TaskStatusUpdateEvent {
        task_id: "task-9".to_string(),
        context_id: "context-9".to_string(),
        status: a2a_rs::domain::core::task::TaskState::Working,
        final_: false,
        timestamp: Some(chrono::Utc::now()),
    };

    let result = manager.notify_task_status_update("task-9", &status_event).await;

    // Should succeed silently (no notification configured)
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notify_task_artifact_update() {
    let manager = MockNotificationManager::new();

    let config = create_test_config("task-10", "https://example.com/webhook");
    manager.set_task_notification(&config).await.unwrap();

    let artifact_event = a2a_rs::domain::core::task::TaskArtifactUpdateEvent {
        task_id: "task-10".to_string(),
        context_id: "context-10".to_string(),
        artifact_id: "artifact-1".to_string(),
        artifact_name: Some("Report".to_string()),
        last_chunk: Some(true),
        append: false,
        timestamp: Some(chrono::Utc::now()),
    };

    let result = manager.notify_task_artifact_update("task-10", &artifact_event).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_notify_task_artifact_update_no_notification() {
    let manager = MockNotificationManager::new();

    let artifact_event = a2a_rs::domain::core::task::TaskArtifactUpdateEvent {
        task_id: "task-11".to_string(),
        context_id: "context-11".to_string(),
        artifact_id: "artifact-2".to_string(),
        artifact_name: Some("Data".to_string()),
        last_chunk: Some(false),
        append: true,
        timestamp: Some(chrono::Utc::now()),
    };

    let result = manager.notify_task_artifact_update("task-11", &artifact_event).await;

    // Should succeed silently (no notification configured)
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_task_notifications() {
    let manager = MockNotificationManager::new();

    // Set up notifications for multiple tasks
    for i in 1..=5 {
        let config = create_test_config(
            &format!("task-multi-{}", i),
            &format!("https://example.com/webhook/{}", i),
        );
        manager.set_task_notification(&config).await.unwrap();
    }

    // Verify all are set
    for i in 1..=5 {
        let task_id = format!("task-multi-{}", i);
        let result = manager.has_task_notification(&task_id).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_update_task_notification() {
    let manager = MockNotificationManager::new();

    // Set initial config
    let config1 = create_test_config("task-12", "https://example.com/webhook1");
    manager.set_task_notification(&config1).await.unwrap();

    // Update with new URL
    let config2 = create_test_config("task-12", "https://example.com/webhook2");
    manager.set_task_notification(&config2).await.unwrap();

    // Verify updated
    let result = manager.get_task_notification("task-12").await.unwrap();
    assert_eq!(result.push_notification_config.url, "https://example.com/webhook2");
}

#[tokio::test]
async fn test_config_with_token() {
    let manager = MockNotificationManager::new();

    let config = TaskPushNotificationConfig {
        task_id: "task-13".to_string(),
        push_notification_config: PushNotificationConfig {
            url: "https://example.com/webhook".to_string(),
            token: Some("secret-token-123".to_string()),
        },
    };

    let result = manager.set_task_notification(&config).await;

    assert!(result.is_ok());

    let retrieved = manager.get_task_notification("task-13").await.unwrap();
    assert_eq!(
        retrieved.push_notification_config.token,
        Some("secret-token-123".to_string())
    );
}
