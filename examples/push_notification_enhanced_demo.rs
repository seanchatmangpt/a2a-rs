//! Enhanced Push Notification Delivery Example
//!
//! Demonstrates the complete push notification delivery system with:
//! - HTTP webhook delivery with retries
//! - HMAC-SHA256 signature generation
//! - Dead letter queue for failed notifications
//! - Delivery status tracking
//! - Event deduplication

use a2a_rs::adapter::{
    DeadLetterEntry, DeliveryStatus, EnhancedHttpPushNotificationSender,
    HttpPushNotificationConfig, InMemoryDeadLetterQueue, InMemoryDeliveryTracker,
    PushNotificationSender,
};
use a2a_rs::domain::{PushNotificationConfig, TaskStatus, TaskStatusUpdateEvent};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    tracing::info!("Starting Enhanced Push Notification Demo");

    // ============================================================
    // 1. Basic Setup
    // ============================================================

    // Create an enhanced push notification sender with custom config
    let config = HttpPushNotificationConfig::builder()
        .timeout(30) // 30 second timeout
        .max_retries(5) // Retry up to 5 times
        .backoff_ms(1000) // Start with 1 second backoff
        .enable_deduplication(true) // Prevent duplicate notifications
        .enable_tracking(true) // Track all delivery attempts
        .enable_dead_letter(true) // Use dead letter queue
        .signing_key(Some("my-webhook-secret-key".to_string())) // Sign webhooks
        .signature_header(Some("X-Webhook-Signature".to_string()))
        .build();

    let sender = EnhancedHttpPushNotificationSender::with_config(config);

    // Get references to tracker and DLQ for inspection
    let tracker = sender.tracker();
    let dlq = sender.dead_letter_queue();

    tracing::info!("Enhanced sender configured with retries and tracking");

    // ============================================================
    // 2. Push Notification Configuration
    // ============================================================

    // Configure push notification for a task
    let push_config = PushNotificationConfig {
        id: Some("webhook-config-1".to_string()),
        url: "https://httpbin.org/post".to_string(), // Echo service for testing
        token: Some("bearer-token-123".to_string()),
        authentication: None,
    };

    tracing::info!(
        url = %push_config.url,
        "Push notification configuration created"
    );

    // ============================================================
    // 3. Send Status Update Notification
    // ============================================================

    let status_event = TaskStatusUpdateEvent {
        task_id: "task-abc123".to_string(),
        context_id: "ctx-456".to_string(),
        kind: "status-update".to_string(),
        status: TaskStatus {
            state: "completed".to_string(),
            timestamp: Some("2024-02-11T10:00:00Z".to_string()),
            message: Some("Task completed successfully".to_string()),
        },
        final_: true,
        metadata: Some(serde_json::Map::from_iter(vec![
            ("progress".to_string(), json!("100")),
            ("duration".to_string(), json!("5.2s")),
        ])),
    };

    tracing::info!(
        task_id = %status_event.task_id,
        state = %status_event.status.state,
        "Sending task status update notification"
    );

    match sender
        .send_status_update(&push_config, &status_event)
        .await
    {
        Ok(()) => {
            tracing::info!("Notification delivered successfully!");
        }
        Err(e) => {
            tracing::error!(error = %e, "Notification delivery failed");
        }
    }

    // ============================================================
    // 4. Inspect Delivery Tracking
    // ============================================================

    // Get all delivery tracking for the task
    let task_tracking = tracker.get_task_tracking("task-abc123").await;

    tracking::info!(
        count = task_tracking.len(),
        "Retrieved delivery tracking records"
    );

    for tracking in task_tracking {
        tracing::info!(
            delivery_id = %tracking.delivery_id,
            event_id = %tracking.event_id,
            status = ?tracking.status,
            attempts = tracking.attempts,
            "Delivery tracking record"
        );

        if let Some(delivered_at) = tracking.delivered_at {
            tracing::info!(
                delivered_at = delivered_at,
                "Notification was delivered at"
            );
        }
    }

    // ============================================================
    // 5. Demonstrate Deduplication
    // ============================================================

    tracing::info!("Testing event deduplication (sending same event again)");

    // Send the same event again - should be deduplicated
    match sender
        .send_status_update(&push_config, &status_event)
        .await
    {
        Ok(()) => {
            tracing::info!("Duplicate event detected and skipped");
        }
        Err(e) => {
            tracing::error!(error = %e, "Unexpected error");
        }
    }

    // ============================================================
    // 6. Dead Letter Queue Example
    // ============================================================

    // Manually create a dead letter entry to demonstrate DLQ functionality
    let dlq_entry = DeadLetterEntry {
        id: "dlq-demo-1".to_string(),
        task_id: "task-failed-123".to_string(),
        event_type: "status".to_string(),
        event_data: json!({
            "taskId": "task-failed-123",
            "status": {"state": "failed"}
        }),
        url: "https://invalid-endpoint.example.com/webhook".to_string(),
        reason: "Connection refused after 5 attempts".to_string(),
        attempts: 5,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        replayed: false,
    };

    dlq.add(dlq_entry.clone()).await?;

    tracing::info!(
        id = %dlq_entry.id,
        task_id = %dlq_entry.task_id,
        "Added entry to dead letter queue"
    );

    // List all dead letter entries
    let all_dlq = dlq.get_all().await?;
    tracing::info!(count = all_dlq.len(), "Total dead letter entries");

    for entry in all_dlq {
        tracing::info!(
            id = %entry.id,
            task_id = %entry.task_id,
            event_type = %entry.event_type,
            reason = %entry.reason,
            attempts = entry.attempts,
            "Dead letter entry"
        );
    }

    // Get entries for a specific task
    let task_dlq = dlq.get_by_task("task-failed-123").await?;
    tracing::info!(
        task_id = "task-failed-123",
        count = task_dlq.len(),
        "Dead letter entries for task"
    );

    // ============================================================
    // 7. Configuration Builder Examples
    // ============================================================

    tracing::info!("Example: Different configuration patterns");

    // Minimal config (all defaults)
    let minimal_config = HttpPushNotificationConfig::builder().build();
    tracing::info!(
        timeout = minimal_config.timeout,
        max_retries = minimal_config.max_retries,
        "Minimal configuration"
    );

    // Production config with all features
    let production_config = HttpPushNotificationConfig::builder()
        .timeout(60)
        .max_retries(10)
        .backoff_ms(2000)
        .signing_key(Some("production-secret-key".to_string()))
        .signature_header(Some("X-Webhook-Signature".to_string()))
        .enable_deduplication(true)
        .enable_tracking(true)
        .enable_dead_letter(true)
        .build();

    tracing::info!(
        timeout = production_config.timeout,
        max_retries = production_config.max_retries,
        backoff_ms = production_config.backoff_ms,
        "Production configuration"
    );

    // ============================================================
    // 8. Cleanup and Maintenance
    // ============================================================

    // Clean up old delivery records (older than 1 hour)
    let removed = tracker.cleanup_old(3600).await?;
    tracing::info!(removed = removed, "Cleaned up old delivery records");

    // Clear dead letter queue if needed
    // dlq.clear().await?;

    tracing::info!("Demo completed successfully");

    Ok(())
}
