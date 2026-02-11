//! Enhanced push notification delivery with retries, signatures, and dead letter queue

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bon::Builder;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

#[cfg(feature = "crypto")]
use hmac::{Hmac, Mac};
#[cfg(feature = "crypto")]
use sha2::Sha256;

#[cfg(feature = "http-client")]
use reqwest::{
    header::{CONTENT_TYPE, HeaderMap, HeaderValue},
    Client,
};

use crate::adapter::business::push_notification::PushNotificationSender;
use crate::domain::{
    A2AError, PushNotificationConfig, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};

/// Delivery status for a push notification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatus {
    /// Notification is pending delivery
    Pending,
    /// Notification is currently being sent
    Sending,
    /// Notification was successfully delivered
    Delivered,
    /// Notification delivery failed (will retry)
    Failed { error: String, attempt: u32 },
    /// Notification moved to dead letter queue (exceeded retries)
    DeadLettered { reason: String, attempts: u32 },
}

/// Dead letter queue entry for failed notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// Unique ID for this dead letter entry
    pub id: String,
    /// Task ID
    pub task_id: String,
    /// Event type (status or artifact)
    pub event_type: String,
    /// Serialized event data
    pub event_data: serde_json::Value,
    /// Push notification config URL
    pub url: String,
    /// Original failure reason
    pub reason: String,
    /// Number of delivery attempts made
    pub attempts: u32,
    /// Timestamp when entry was created
    pub created_at: u64,
    /// Whether this entry has been replayed
    pub replayed: bool,
}

/// Delivery tracking information for a notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryTracking {
    /// Unique delivery ID
    pub delivery_id: String,
    /// Task ID
    pub task_id: String,
    /// Event type identifier
    pub event_id: String,
    /// Current delivery status
    pub status: DeliveryStatus,
    /// Number of delivery attempts
    pub attempts: u32,
    /// Timestamp of first attempt
    pub first_attempt_at: u64,
    /// Timestamp of last attempt
    pub last_attempt_at: u64,
    /// Timestamp when delivered (if successful)
    pub delivered_at: Option<u64>,
    /// Last error message (if failed)
    pub last_error: Option<String>,
}

/// In-memory dead letter queue for failed notifications
pub struct InMemoryDeadLetterQueue {
    entries: Arc<Mutex<Vec<DeadLetterEntry>>>,
}

impl InMemoryDeadLetterQueue {
    /// Create a new dead letter queue
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add an entry to the dead letter queue
    pub async fn add(&self, entry: DeadLetterEntry) -> Result<(), A2AError> {
        let mut entries = self.entries.lock().await;
        entries.push(entry);
        Ok(())
    }

    /// Get all entries
    pub async fn get_all(&self) -> Result<Vec<DeadLetterEntry>, A2AError> {
        let entries = self.entries.lock().await;
        Ok(entries.clone())
    }

    /// Get entries by task ID
    pub async fn get_by_task(&self, task_id: &str) -> Result<Vec<DeadLetterEntry>, A2AError> {
        let entries = self.entries.lock().await;
        Ok(entries
            .iter()
            .filter(|e| e.task_id == task_id)
            .cloned()
            .collect())
    }

    /// Remove an entry by ID
    pub async fn remove(&self, id: &str) -> Result<bool, A2AError> {
        let mut entries = self.entries.lock().await;
        let original_len = entries.len();
        entries.retain(|e| e.id != id);
        Ok(entries.len() < original_len)
    }

    /// Clear all entries
    pub async fn clear(&self) -> Result<(), A2AError> {
        let mut entries = self.entries.lock().await;
        entries.clear();
        Ok(())
    }

    /// Get count of entries
    pub async fn count(&self) -> Result<usize, A2AError> {
        let entries = self.entries.lock().await;
        Ok(entries.len())
    }
}

impl Default for InMemoryDeadLetterQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory delivery tracking store
pub struct InMemoryDeliveryTracker {
    /// Track deliveries by task_id -> event_id -> DeliveryTracking
    deliveries: Arc<RwLock<HashMap<String, HashMap<String, DeliveryTracking>>>>,
}

impl InMemoryDeliveryTracker {
    /// Create a new delivery tracker
    pub fn new() -> Self {
        Self {
            deliveries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a unique event ID for deduplication
    pub fn generate_event_id(&self, task_id: &str, event_data: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        task_id.hash(&mut hasher);
        event_data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Check if an event has already been delivered
    pub async fn is_delivered(&self, task_id: &str, event_id: &str) -> bool {
        let deliveries = self.deliveries.read().await;
        deliveries
            .get(task_id)
            .and_then(|task_events| task_events.get(event_id))
            .map(|tracking| tracking.status == DeliveryStatus::Delivered)
            .unwrap_or(false)
    }

    /// Record a delivery attempt
    pub async fn record_attempt(
        &self,
        task_id: &str,
        event_id: &str,
        status: DeliveryStatus,
    ) -> Result<(), A2AError> {
        let mut deliveries = self.deliveries.write().await;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| A2AError::Internal(format!("Time error: {}", e)))?
            .as_secs();

        let task_deliveries = deliveries.entry(task_id.to_string()).or_default();

        let tracking = task_deliveries.entry(event_id.to_string()).or_insert_with(|| {
            DeliveryTracking {
                delivery_id: uuid::Uuid::new_v4().to_string(),
                task_id: task_id.to_string(),
                event_id: event_id.to_string(),
                status: DeliveryStatus::Pending,
                attempts: 0,
                first_attempt_at: now,
                last_attempt_at: now,
                delivered_at: None,
                last_error: None,
            }
        });

        tracking.last_attempt_at = now;
        tracking.attempts += 1;
        tracking.status = status.clone();

        match status {
            DeliveryStatus::Delivered => {
                tracking.delivered_at = Some(now);
                tracking.last_error = None;
            }
            DeliveryStatus::Failed { error, .. } | DeliveryStatus::DeadLettered { .. } => {
                tracking.last_error = Some(error);
            }
            _ => {}
        }

        Ok(())
    }

    /// Get delivery tracking for a specific event
    pub async fn get_tracking(
        &self,
        task_id: &str,
        event_id: &str,
    ) -> Option<DeliveryTracking> {
        let deliveries = self.deliveries.read().await;
        deliveries
            .get(task_id)
            .and_then(|task_events| task_events.get(event_id))
            .cloned()
    }

    /// Get all delivery tracking for a task
    pub async fn get_task_tracking(&self, task_id: &str) -> Vec<DeliveryTracking> {
        let deliveries = self.deliveries.read().await;
        deliveries
            .get(task_id)
            .map(|events| events.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Clean up old delivery records (optional maintenance)
    pub async fn cleanup_old(&self, older_than_secs: u64) -> Result<usize, A2AError> {
        let mut deliveries = self.deliveries.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| A2AError::Internal(format!("Time error: {}", e)))?
            .as_secs();

        let mut removed = 0;
        for (_task_id, events) in deliveries.iter_mut() {
            events.retain(|_event_id, tracking| {
                let should_keep = now - tracking.last_attempt_at < older_than_secs;
                if !should_keep {
                    removed += 1;
                }
                should_keep
            });
        }

        Ok(removed)
    }
}

impl Default for InMemoryDeliveryTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the enhanced HTTP push notification sender
#[derive(Debug, Clone, Builder)]
pub struct HttpPushNotificationConfig {
    /// HTTP client timeout in seconds
    #[builder(default = 30)]
    timeout: u64,

    /// Maximum number of retry attempts
    #[builder(default = 3)]
    max_retries: u32,

    /// Initial backoff delay in milliseconds
    #[builder(default = 1000)]
    backoff_ms: u64,

    /// Whether to enable event deduplication
    #[builder(default = true)]
    enable_deduplication: bool,

    /// Whether to enable delivery tracking
    #[builder(default = true)]
    enable_tracking: bool,

    /// Whether to move failed notifications to dead letter queue
    #[builder(default = true)]
    enable_dead_letter: bool,

    /// HMAC secret key for webhook signature (optional)
    #[builder(default = None)]
    signing_key: Option<String>,

    /// Signature header name (default: X-Webhook-Signature)
    #[builder(default = Some("X-Webhook-Signature".to_string()))]
    signature_header: Option<String>,
}

impl Default for HttpPushNotificationConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Enhanced HTTP push notification sender with retries, signatures, and tracking
#[cfg(feature = "http-client")]
pub struct EnhancedHttpPushNotificationSender {
    /// HTTP client for sending notifications
    client: Client,
    /// Configuration for the sender
    config: HttpPushNotificationConfig,
    /// Delivery tracker
    tracker: Arc<InMemoryDeliveryTracker>,
    /// Dead letter queue
    dead_letter: Arc<InMemoryDeadLetterQueue>,
}

#[cfg(feature = "http-client")]
impl EnhancedHttpPushNotificationSender {
    /// Create a new enhanced push notification sender with default config
    pub fn new() -> Self {
        Self::with_config(HttpPushNotificationConfig::default())
    }

    /// Create a new enhanced push notification sender with custom config
    pub fn with_config(config: HttpPushNotificationConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(config.timeout))
                .build()
                .unwrap_or_else(|_| Client::new()),
            config,
            tracker: Arc::new(InMemoryDeliveryTracker::new()),
            dead_letter: Arc::new(InMemoryDeadLetterQueue::new()),
        }
    }

    /// Get a reference to the delivery tracker
    pub fn tracker(&self) -> Arc<InMemoryDeliveryTracker> {
        Arc::clone(&self.tracker)
    }

    /// Get a reference to the dead letter queue
    pub fn dead_letter_queue(&self) -> Arc<InMemoryDeadLetterQueue> {
        Arc::clone(&self.dead_letter)
    }

    /// Generate HMAC signature for webhook payload
    #[cfg(feature = "crypto")]
    fn generate_signature(&self, payload: &str) -> Result<String, A2AError> {
        if let Some(key) = &self.config.signing_key {
            type HmacSha256 = Hmac<Sha256>;

            let mut mac =
                HmacSha256::new_from_slice(key.as_bytes()).map_err(|e| {
                    A2AError::Internal(format!("Invalid HMAC key: {}", e))
                })?;

            mac.update(payload.as_bytes());
            let signature = mac.finalize().into_bytes();
            Ok(hex::encode(signature))
        } else {
            Ok(String::new())
        }
    }

    /// Get headers for the webhook request including signature
    fn get_headers(&self, config: &PushNotificationConfig, payload: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // Add webhook signature if configured
        #[cfg(feature = "crypto")]
        if let Some(key) = &self.config.signing_key {
            if let Ok(signature) = self.generate_signature(payload) {
                let header_name = self
                    .config
                    .signature_header
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("X-Webhook-Signature");

                if let Ok(header_value) = HeaderValue::from_str(&format!("sha256={}", signature)) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Add Bearer token if provided
        if let Some(token) = &config.token {
            if let Ok(header_value) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                headers.insert("Authorization", header_value);
            }
        }

        // Add additional authentication if provided
        if let Some(auth) = &config.authentication {
            if let Some(credentials) = &auth.credentials {
                if !auth.schemes.is_empty() {
                    let scheme = &auth.schemes[0];

                    let auth_value = if scheme.to_lowercase() == "basic" {
                        format!("Basic {}", credentials)
                    } else if scheme.to_lowercase() == "bearer" {
                        format!("Bearer {}", credentials)
                    } else {
                        credentials.clone()
                    };

                    if let Ok(header_value) = HeaderValue::from_str(&auth_value) {
                        headers.insert("Authorization", header_value);
                    }
                }
            }
        }

        headers
    }

    /// Send notification with retry logic and tracking
    async fn send_with_retry<T: Serialize>(
        &self,
        config: &PushNotificationConfig,
        event: &T,
        event_type: &str,
    ) -> Result<(), A2AError> {
        let task_id = match event_type {
            "status" => {
                let event: &TaskStatusUpdateEvent =
                    unsafe { &*(event as *const T as *const TaskStatusUpdateEvent) };
                event.task_id.clone()
            }
            "artifact" => {
                let event: &TaskArtifactUpdateEvent =
                    unsafe { &*(event as *const T as *const TaskArtifactUpdateEvent) };
                event.task_id.clone()
            }
            _ => return Err(A2AError::Internal("Invalid event type".to_string())),
        };

        // Serialize event for deduplication and sending
        let payload = serde_json::to_string(event).map_err(|e| {
            A2AError::Internal(format!("Failed to serialize event: {}", e))
        })?;

        // Generate event ID for deduplication
        let event_id = self.tracker.generate_event_id(&task_id, &payload);

        // Check deduplication
        if self.config.enable_deduplication {
            if self.tracker.is_delivered(&task_id, &event_id).await {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    task_id = %task_id,
                    event_id = %event_id,
                    "Event already delivered, skipping"
                );
                return Ok(());
            }
        }

        // Record pending delivery
        if self.config.enable_tracking {
            self.tracker
                .record_attempt(&task_id, &event_id, DeliveryStatus::Pending)
                .await?;
        }

        // Attempt delivery with retries
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            // Record sending status
            if self.config.enable_tracking && attempt > 0 {
                self.tracker
                    .record_attempt(
                        &task_id,
                        &event_id,
                        DeliveryStatus::Failed {
                            error: last_error.clone().unwrap_or_default(),
                            attempt,
                        },
                    )
                    .await?;
            }

            // Exponential backoff for retries
            if attempt > 0 {
                let backoff = self.config.backoff_ms * (1 << (attempt - 1).min(10));
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    task_id = %task_id,
                    attempt = attempt,
                    backoff_ms = backoff,
                    "Retrying webhook delivery after backoff"
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }

            // Send the webhook
            match self
                .client
                .post(&config.url)
                .headers(self.get_headers(config, &payload))
                .body(payload.clone())
                .send()
                .await
            {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        #[cfg(feature = "tracing")]
                        tracing::info!(
                            task_id = %task_id,
                            event_id = %event_id,
                            status = %status,
                            attempt = attempt,
                            "Webhook delivered successfully"
                        );

                        // Record successful delivery
                        if self.config.enable_tracking {
                            self.tracker
                                .record_attempt(&task_id, &event_id, DeliveryStatus::Delivered)
                                .await?;
                        }

                        return Ok(());
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        let error = format!("HTTP {}: {}", status, body);

                        #[cfg(feature = "tracing")]
                        tracing::warn!(
                            task_id = %task_id,
                            event_id = %event_id,
                            status = %status,
                            attempt = attempt,
                            error = %error,
                            "Webhook delivery failed"
                        );

                        last_error = Some(error.clone());

                        // Don't retry on client errors (4xx)
                        if status.is_client_error() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let error = format!("Request error: {}", e);

                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        task_id = %task_id,
                        event_id = %event_id,
                        attempt = attempt,
                        error = %error,
                        "Webhook request failed"
                    );

                    last_error = Some(error);
                }
            }
        }

        // All retries exhausted - move to dead letter queue
        let final_error = last_error.unwrap_or_else(|| "Unknown error".to_string());

        if self.config.enable_dead_letter {
            let dead_entry = DeadLetterEntry {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: task_id.clone(),
                event_type: event_type.to_string(),
                event_data: serde_json::to_value(event).unwrap_or(serde_json::Value::Null),
                url: config.url.clone(),
                reason: final_error.clone(),
                attempts: self.config.max_retries + 1,
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                replayed: false,
            };

            self.dead_letter.add(dead_entry).await?;

            if self.config.enable_tracking {
                self.tracker
                    .record_attempt(
                        &task_id,
                        &event_id,
                        DeliveryStatus::DeadLettered {
                            reason: final_error.clone(),
                            attempts: self.config.max_retries + 1,
                        },
                    )
                    .await?;
            }

            #[cfg(feature = "tracing")]
            tracing::error!(
                task_id = %task_id,
                event_id = %event_id,
                error = %final_error,
                "Webhook moved to dead letter queue after all retries exhausted"
            );
        }

        Err(A2AError::Internal(format!(
            "Webhook delivery failed after {} attempts: {}",
            self.config.max_retries + 1,
            final_error
        )))
    }
}

#[cfg(feature = "http-client")]
impl Default for EnhancedHttpPushNotificationSender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "http-client")]
#[async_trait]
impl PushNotificationSender for EnhancedHttpPushNotificationSender {
    async fn send_status_update(
        &self,
        config: &PushNotificationConfig,
        event: &TaskStatusUpdateEvent,
    ) -> Result<(), A2AError> {
        self.send_with_retry(config, event, "status").await
    }

    async fn send_artifact_update(
        &self,
        config: &PushNotificationConfig,
        event: &TaskArtifactUpdateEvent,
    ) -> Result<(), A2AError> {
        self.send_with_retry(config, event, "artifact").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delivery_status_serialization() {
        let status = DeliveryStatus::Delivered;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Delivered"));
    }

    #[test]
    fn test_dead_letter_entry_serialization() {
        let entry = DeadLetterEntry {
            id: "test-id".to_string(),
            task_id: "task-123".to_string(),
            event_type: "status".to_string(),
            event_data: serde_json::json!({"test": "data"}),
            url: "https://example.com/webhook".to_string(),
            reason: "Connection refused".to_string(),
            attempts: 3,
            created_at: 1234567890,
            replayed: false,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: DeadLetterEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.task_id, entry.task_id);
    }

    #[test]
    fn test_http_push_notification_config_builder() {
        let config = HttpPushNotificationConfig::builder()
            .timeout(60)
            .max_retries(5)
            .backoff_ms(2000)
            .signing_key(Some("test-secret".to_string()))
            .build();

        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.backoff_ms, 2000);
        assert_eq!(config.signing_key, Some("test-secret".to_string()));
    }

    #[tokio::test]
    async fn test_delivery_tracker_deduplication() {
        let tracker = InMemoryDeliveryTracker::new();
        let task_id = "task-123";
        let event_data = r#"{"status":"completed"}"#;
        let event_id = tracker.generate_event_id(task_id, event_data);

        // Not delivered initially
        assert!(!tracker.is_delivered(task_id, &event_id).await);

        // Record successful delivery
        tracker
            .record_attempt(task_id, &event_id, DeliveryStatus::Delivered)
            .await
            .unwrap();

        // Now marked as delivered
        assert!(tracker.is_delivered(task_id, &event_id).await);

        // Verify tracking data
        let tracking = tracker.get_tracking(task_id, &event_id).await;
        assert!(tracking.is_some());
        let tracking = tracking.unwrap();
        assert_eq!(tracking.status, DeliveryStatus::Delivered);
        assert_eq!(tracking.attempts, 1);
    }

    #[tokio::test]
    async fn test_dead_letter_queue_operations() {
        let dlq = InMemoryDeadLetterQueue::new();

        let entry = DeadLetterEntry {
            id: "dlq-1".to_string(),
            task_id: "task-123".to_string(),
            event_type: "status".to_string(),
            event_data: serde_json::json!({"test": "data"}),
            url: "https://example.com/webhook".to_string(),
            reason: "Connection refused".to_string(),
            attempts: 3,
            created_at: 1234567890,
            replayed: false,
        };

        // Add entry
        dlq.add(entry.clone()).await.unwrap();
        assert_eq!(dlq.count().await.unwrap(), 1);

        // Get all
        let all = dlq.get_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "dlq-1");

        // Get by task
        let by_task = dlq.get_by_task("task-123").await.unwrap();
        assert_eq!(by_task.len(), 1);

        // Remove entry
        assert!(dlq.remove("dlq-1").await.unwrap());
        assert_eq!(dlq.count().await.unwrap(), 0);
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_signature_generation() {
        let config = HttpPushNotificationConfig::builder()
            .signing_key(Some("test-secret-key".to_string()))
            .build();

        let sender = EnhancedHttpPushNotificationSender::with_config(config);
        let payload = r#"{"test":"data"}"#;

        let signature = sender.generate_signature(payload).unwrap();
        assert!(!signature.is_empty());

        // Signature should be deterministic
        let signature2 = sender.generate_signature(payload).unwrap();
        assert_eq!(signature, signature2);
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_signature_different_payloads() {
        let config = HttpPushNotificationConfig::builder()
            .signing_key(Some("test-secret-key".to_string()))
            .build();

        let sender = EnhancedHttpPushNotificationSender::with_config(config);

        let sig1 = sender.generate_signature(r#"{"test":"data1"}"#).unwrap();
        let sig2 = sender.generate_signature(r#"{"test":"data2"}"#).unwrap();

        assert_ne!(sig1, sig2);
    }
}
