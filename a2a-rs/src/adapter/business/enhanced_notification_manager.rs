//! Enhanced notification manager implementation with delivery tracking

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::domain::{
    A2AError, PushNotificationConfig, TaskArtifactUpdateEvent,
    TaskPushNotificationConfig, TaskStatusUpdateEvent,
};
use crate::port::AsyncNotificationManager;
use crate::adapter::business::push_notification::PushNotificationSender;

#[cfg(feature = "http-client")]
use crate::adapter::business::push_notification_enhanced::{
    HttpPushNotificationConfig, InMemoryDeadLetterQueue,
    InMemoryDeliveryTracker,
};

/// Enhanced notification manager with delivery tracking and retries
pub struct EnhancedNotificationManager {
    /// Storage for task push notification configurations
    configs: Arc<Mutex<HashMap<String, TaskPushNotificationConfig>>>,
    /// Optional HTTP sender for actual delivery
    #[cfg(feature = "http-client")]
    sender: Option<Arc<EnhancedHttpNotificationSender>>,
}

impl EnhancedNotificationManager {
    /// Create a new enhanced notification manager without HTTP delivery
    pub fn new() -> Self {
        Self {
            configs: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "http-client")]
            sender: None,
        }
    }

    /// Create a new enhanced notification manager with HTTP delivery
    #[cfg(feature = "http-client")]
    pub fn with_http_sender(
        config: HttpPushNotificationConfig,
    ) -> Self {
        Self {
            configs: Arc::new(Mutex::new(HashMap::new())),
            sender: Some(Arc::new(EnhancedHttpNotificationSender::new(
                config,
            ))),
        }
    }

    /// Get a reference to the HTTP sender if configured
    #[cfg(feature = "http-client")]
    pub fn sender(&self) -> Option<Arc<EnhancedHttpNotificationSender>> {
        self.sender.as_ref().map(Arc::clone)
    }

    /// Get the delivery tracker if sender is configured
    #[cfg(feature = "http-client")]
    pub fn tracker(&self) -> Option<Arc<InMemoryDeliveryTracker>> {
        self.sender.as_ref().map(|s| s.tracker())
    }

    /// Get the dead letter queue if sender is configured
    #[cfg(feature = "http-client")]
    pub fn dead_letter_queue(&self) -> Option<Arc<InMemoryDeadLetterQueue>> {
        self.sender.as_ref().map(|s| s.dead_letter_queue())
    }
}

impl Default for EnhancedNotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncNotificationManager for EnhancedNotificationManager {
    async fn set_task_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let mut configs = self.configs.lock().await;
        let config = config.clone();
        configs.insert(config.task_id.clone(), config.clone());
        Ok(config)
    }

    async fn get_task_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let configs = self.configs.lock().await;
        configs
            .get(task_id)
            .cloned()
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))
    }

    async fn remove_task_notification<'a>(&self, task_id: &'a str) -> Result<(), A2AError> {
        let mut configs = self.configs.lock().await;
        configs
            .remove(task_id)
            .ok_or_else(|| A2AError::TaskNotFound(task_id.to_string()))?;
        Ok(())
    }

    async fn notify_task_status_update<'a>(
        &self,
        task_id: &'a str,
        status_update: &'a TaskStatusUpdateEvent,
    ) -> Result<(), A2AError> {
        // Check if notifications are configured for this task
        let config = {
            let configs = self.configs.lock().await;
            configs.get(task_id).cloned()
        };

        if let Some(task_config) = config {
            #[cfg(feature = "tracing")]
            tracing::info!(
                task_id = %task_id,
                url = %task_config.push_notification_config.url,
                state = ?status_update.status.state,
                "Sending status update notification"
            );

            #[cfg(feature = "http-client")]
            {
                if let Some(sender) = &self.sender {
                    return sender
                        .send_status_update(&task_config.push_notification_config, status_update)
                        .await;
                }
                #[cfg(feature = "tracing")]
                tracing::warn!("No HTTP sender available, notification not sent");
            }

            #[cfg(not(feature = "http-client"))]
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("HTTP client not enabled, notification not sent");
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                task_id = %task_id,
                "No push notification config for task"
            );
        }
        Ok(())
    }

    async fn notify_task_artifact_update<'a>(
        &self,
        task_id: &'a str,
        artifact_update: &'a TaskArtifactUpdateEvent,
    ) -> Result<(), A2AError> {
        // Check if notifications are configured for this task
        let config = {
            let configs = self.configs.lock().await;
            configs.get(task_id).cloned()
        };

        if let Some(task_config) = config {
            #[cfg(feature = "tracing")]
            tracing::info!(
                task_id = %task_id,
                url = %task_config.push_notification_config.url,
                "Sending artifact update notification"
            );

            #[cfg(feature = "http-client")]
            {
                if let Some(sender) = &self.sender {
                    return sender
                        .send_artifact_update(
                            &task_config.push_notification_config,
                            artifact_update,
                        )
                        .await;
                }
                #[cfg(feature = "tracing")]
                tracing::warn!("No HTTP sender available, notification not sent");
                return Ok(());
            }

            #[cfg(not(feature = "http-client"))]
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("HTTP client not enabled, notification not sent");
                return Ok(());
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                task_id = %task_id,
                "No push notification config for task"
            );
            Ok(())
        }
    }
}

/// Wrapper around EnhancedHttpPushNotificationSender that implements AsyncNotificationManager
#[cfg(feature = "http-client")]
pub struct EnhancedHttpNotificationSender {
    sender: crate::adapter::EnhancedHttpPushNotificationSender,
}

#[cfg(feature = "http-client")]
impl EnhancedHttpNotificationSender {
    /// Create a new HTTP notification sender
    pub fn new(config: HttpPushNotificationConfig) -> Self {
        Self {
            sender: crate::adapter::EnhancedHttpPushNotificationSender::with_config(config),
        }
    }

    /// Get the underlying enhanced sender
    pub fn inner(&self) -> &crate::adapter::EnhancedHttpPushNotificationSender {
        &self.sender
    }

    /// Get a reference to the delivery tracker
    pub fn tracker(&self) -> Arc<InMemoryDeliveryTracker> {
        self.sender.tracker()
    }

    /// Get a reference to the dead letter queue
    pub fn dead_letter_queue(&self) -> Arc<InMemoryDeadLetterQueue> {
        self.sender.dead_letter_queue()
    }

    /// Send a status update notification
    pub async fn send_status_update(
        &self,
        config: &PushNotificationConfig,
        event: &TaskStatusUpdateEvent,
    ) -> Result<(), A2AError> {
        self.sender.send_status_update(config, event).await
    }

    /// Send an artifact update notification
    pub async fn send_artifact_update(
        &self,
        config: &PushNotificationConfig,
        event: &TaskArtifactUpdateEvent,
    ) -> Result<(), A2AError> {
        self.sender.send_artifact_update(config, event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_notification_manager_basic() {
        let manager = EnhancedNotificationManager::new();

        // Create a task notification config
        let task_config = TaskPushNotificationConfig {
            task_id: "task-123".to_string(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-1".to_string()),
                url: "https://example.com/webhook".to_string(),
                token: Some("token-123".to_string()),
                authentication: None,
            },
        };

        // Set the config
        manager.set_task_notification(&task_config).await.unwrap();

        // Get it back
        let retrieved = manager.get_task_notification("task-123").await.unwrap();
        assert_eq!(retrieved.task_id, "task-123");
        assert_eq!(
            retrieved.push_notification_config.url,
            "https://example.com/webhook"
        );

        // Remove it
        manager.remove_task_notification("task-123").await.unwrap();

        // Should not exist anymore
        assert!(manager
            .get_task_notification("task-123")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_enhanced_notification_manager_with_http() {
        #[cfg(feature = "http-client")]
        {
            let config = HttpPushNotificationConfig::builder()
                .timeout(10)
                .max_retries(2)
                .enable_tracking(true)
                .build();

            let manager = EnhancedNotificationManager::with_http_sender(config);

            // Verify sender is available
            assert!(manager.sender().is_some());
            assert!(manager.tracker().is_some());
            assert!(manager.dead_letter_queue().is_some());

            // Get the tracker
            let tracker = manager.tracker().unwrap();
            assert_eq!(tracker.get_task_tracking("test-task").await.len(), 0);
        }
    }
}
