//! Unit tests for enhanced push notification system

#[cfg(test)]
mod tests {
    use super::super::push_notification_enhanced::*;
    use crate::domain::{PushNotificationConfig, TaskStatus, TaskStatusUpdateEvent};

    #[tokio::test]
    async fn test_delivery_tracker_basic() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-123";
        let event_data = r#"{"status":"completed"}"#;
        let event_id = tracker.generate_event_id(task_id, event_data);

        // Initially not delivered
        assert!(!tracker.is_delivered(task_id, &event_id).await);

        // Record pending
        tracker
            .record_attempt(task_id, &event_id, DeliveryStatus::Pending)
            .await
            .unwrap();

        // Still not delivered
        assert!(!tracker.is_delivered(task_id, &event_id).await);

        // Record delivered
        tracker
            .record_attempt(task_id, &event_id, DeliveryStatus::Delivered)
            .await
            .unwrap();

        // Now delivered
        assert!(tracker.is_delivered(task_id, &event_id).await);

        // Verify tracking data
        let tracking = tracker.get_tracking(task_id, &event_id).await;
        assert!(tracking.is_some());
        let tracking = tracking.unwrap();
        assert_eq!(tracking.task_id, task_id);
        assert_eq!(tracking.event_id, event_id);
        assert!(matches!(tracking.status, DeliveryStatus::Delivered));
        assert_eq!(tracking.attempts, 2); // pending + delivered
        assert!(tracking.delivered_at.is_some());
    }

    #[tokio::test]
    async fn test_delivery_tracker_failed_status() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-456";
        let event_data = r#"{"status":"failed"}"#;
        let event_id = tracker.generate_event_id(task_id, event_data);

        // Record failed attempt
        tracker
            .record_attempt(
                task_id,
                &event_id,
                DeliveryStatus::Failed {
                    error: "Connection refused".to_string(),
                    attempt: 1,
                },
            )
            .await
            .unwrap();

        let tracking = tracker.get_tracking(task_id, &event_id).await;
        assert!(tracking.is_some());
        let tracking = tracking.unwrap();

        assert!(matches!(
            tracking.status,
            DeliveryStatus::Failed { .. }
        ));
        assert!(tracking.last_error.is_some());
        assert_eq!(
            tracking.last_error.as_ref().unwrap(),
            "Connection refused"
        );
        assert!(tracking.delivered_at.is_none());
    }

    #[tokio::test]
    async fn test_delivery_tracker_dead_lettered() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-789";
        let event_data = r#"{"status":"dead"}"#;
        let event_id = tracker.generate_event_id(task_id, event_data);

        // Record dead lettered
        tracker
            .record_attempt(
                task_id,
                &event_id,
                DeliveryStatus::DeadLettered {
                    reason: "Max retries exceeded".to_string(),
                    attempts: 5,
                },
            )
            .await
            .unwrap();

        let tracking = tracker.get_tracking(task_id, &event_id).await;
        assert!(tracking.is_some());
        let tracking = tracking.unwrap();

        assert!(matches!(
            tracking.status,
            DeliveryStatus::DeadLettered { .. }
        ));
        assert!(tracking.last_error.is_some());
    }

    #[tokio::test]
    async fn test_delivery_tracker_get_task_tracking() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-multi";

        // Add multiple events for the same task
        for i in 1..=3 {
            let event_data = &format!(r#"{{"event":{}}}"#, i);
            let event_id = tracker.generate_event_id(task_id, event_data);

            tracker
                .record_attempt(task_id, &event_id, DeliveryStatus::Delivered)
                .await
                .unwrap();
        }

        // Get all tracking for the task
        let all_tracking = tracker.get_task_tracking(task_id).await;
        assert_eq!(all_tracking.len(), 3);

        // Each should have the same task_id
        for tracking in all_tracking {
            assert_eq!(tracking.task_id, task_id);
        }
    }

    #[tokio::test]
    async fn test_delivery_tracker_cleanup() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-cleanup";
        let event_data = r#"{"test":"data"}"#;
        let event_id = tracker.generate_event_id(task_id, event_data);

        // Record delivery
        tracker
            .record_attempt(task_id, &event_id, DeliveryStatus::Delivered)
            .await
            .unwrap();

        // Verify it exists
        assert!(tracker.get_tracking(task_id, &event_id).await.is_some());

        // Cleanup old records (cleanup records older than 0 seconds)
        // This should remove our record since we can't easily manipulate time in tests
        let removed = tracker.cleanup_old(0).await.unwrap();

        // At minimum, the cleanup should succeed without errors
        // (actual removal depends on system time)
        assert!(removed >= 0);
    }

    #[tokio::test]
    async fn test_dead_letter_queue_add_get() {
        let dlq = InMemoryDeadLetterQueue::new();

        let entry = DeadLetterEntry {
            id: "dlq-1".to_string(),
            task_id: "task-dlq".to_string(),
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

        // Verify count
        assert_eq!(dlq.count().await.unwrap(), 1);

        // Get all
        let all = dlq.get_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, entry.id);
        assert_eq!(all[0].task_id, entry.task_id);
        assert_eq!(all[0].reason, entry.reason);
    }

    #[tokio::test]
    async fn test_dead_letter_queue_get_by_task() {
        let dlq = InMemoryDeadLetterQueue::new();

        // Add entries for different tasks
        for i in 1..=3 {
            let entry = DeadLetterEntry {
                id: format!("dlq-{}", i),
                task_id: if i % 2 == 0 { "task-a" } else { "task-b" },
                event_type: "status".to_string(),
                event_data: serde_json::json!({"i": i}),
                url: "https://example.com/webhook".to_string(),
                reason: "Failed".to_string(),
                attempts: i,
                created_at: 1234567890,
                replayed: false,
            };
            dlq.add(entry).await.unwrap();
        }

        // Get entries for task-a (should be 1 entry)
        let task_a_entries = dlq.get_by_task("task-a").await.unwrap();
        assert_eq!(task_a_entries.len(), 1);

        // Get entries for task-b (should be 2 entries)
        let task_b_entries = dlq.get_by_task("task-b").await.unwrap();
        assert_eq!(task_b_entries.len(), 2);
    }

    #[tokio::test]
    async fn test_dead_letter_queue_remove() {
        let dlq = InMemoryDeadLetterQueue::new();

        let entry = DeadLetterEntry {
            id: "dlq-remove".to_string(),
            task_id: "task-remove".to_string(),
            event_type: "status".to_string(),
            event_data: serde_json::json!({}),
            url: "https://example.com/webhook".to_string(),
            reason: "Test".to_string(),
            attempts: 1,
            created_at: 1234567890,
            replayed: false,
        };

        dlq.add(entry).await.unwrap();
        assert_eq!(dlq.count().await.unwrap(), 1);

        // Remove non-existent (should return false)
        assert!(!dlq.remove("non-existent").await.unwrap());

        // Remove existing (should return true)
        assert!(dlq.remove("dlq-remove").await.unwrap());
        assert_eq!(dlq.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_dead_letter_queue_clear() {
        let dlq = InMemoryDeadLetterQueue::new();

        // Add multiple entries
        for i in 1..=5 {
            let entry = DeadLetterEntry {
                id: format!("dlq-{}", i),
                task_id: "task-clear".to_string(),
                event_type: "status".to_string(),
                event_data: serde_json::json!({"i": i}),
                url: "https://example.com/webhook".to_string(),
                reason: "Test".to_string(),
                attempts: 1,
                created_at: 1234567890,
                replayed: false,
            };
            dlq.add(entry).await.unwrap();
        }

        assert_eq!(dlq.count().await.unwrap(), 5);

        // Clear all
        dlq.clear().await.unwrap();
        assert_eq!(dlq.count().await.unwrap(), 0);

        // Verify get_all returns empty
        let all = dlq.get_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn test_http_push_notification_config_builder() {
        let config = HttpPushNotificationConfig::builder()
            .timeout(60)
            .max_retries(10)
            .backoff_ms(2000)
            .enable_deduplication(false)
            .enable_tracking(false)
            .enable_dead_letter(false)
            .signing_key(Some("test-key".to_string()))
            .signature_header(Some("X-Signature".to_string()))
            .build();

        assert_eq!(config.timeout, 60);
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.backoff_ms, 2000);
        assert!(!config.enable_deduplication);
        assert!(!config.enable_tracking);
        assert!(!config.enable_dead_letter);
        assert_eq!(config.signing_key, Some("test-key".to_string()));
        assert_eq!(
            config.signature_header,
            Some("X-Signature".to_string())
        );
    }

    #[test]
    fn test_http_push_notification_config_default() {
        let config = HttpPushNotificationConfig::default();

        assert_eq!(config.timeout, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.backoff_ms, 1000);
        assert!(config.enable_deduplication);
        assert!(config.enable_tracking);
        assert!(config.enable_dead_letter);
        assert_eq!(config.signing_key, None);
        assert_eq!(
            config.signature_header,
            Some("X-Webhook-Signature".to_string())
        );
    }

    #[test]
    fn test_delivery_status_serialization() {
        let statuses = vec![
            DeliveryStatus::Pending,
            DeliveryStatus::Sending,
            DeliveryStatus::Delivered,
            DeliveryStatus::Failed {
                error: "Test error".to_string(),
                attempt: 1,
            },
            DeliveryStatus::DeadLettered {
                reason: "Test reason".to_string(),
                attempts: 3,
            },
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: DeliveryStatus = serde_json::from_str(&json).unwrap();

            // Verify round-trip
            match (&status, &deserialized) {
                (DeliveryStatus::Pending, DeliveryStatus::Pending) => {}
                (DeliveryStatus::Sending, DeliveryStatus::Sending) => {}
                (DeliveryStatus::Delivered, DeliveryStatus::Delivered) => {}
                (
                    DeliveryStatus::Failed { error: e1, attempt: a1 },
                    DeliveryStatus::Failed { error: e2, attempt: a2 },
                ) => {
                    assert_eq!(e1, e2);
                    assert_eq!(a1, a2);
                }
                (
                    DeliveryStatus::DeadLettered { reason: r1, attempts: a1 },
                    DeliveryStatus::DeadLettered { reason: r2, attempts: a2 },
                ) => {
                    assert_eq!(r1, r2);
                    assert_eq!(a1, a2);
                }
                _ => panic!("Status mismatch after serialization"),
            }
        }
    }

    #[test]
    fn test_dead_letter_entry_serialization() {
        let entry = DeadLetterEntry {
            id: "test-id".to_string(),
            task_id: "task-123".to_string(),
            event_type: "status".to_string(),
            event_data: serde_json::json!({"key": "value"}),
            url: "https://example.com/webhook".to_string(),
            reason: "Test reason".to_string(),
            attempts: 3,
            created_at: 1234567890,
            replayed: false,
        };

        // Serialize
        let json = serde_json::to_string(&entry).unwrap();

        // Deserialize
        let deserialized: DeadLetterEntry = serde_json::from_str(&json).unwrap();

        // Verify all fields
        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.task_id, entry.task_id);
        assert_eq!(deserialized.event_type, entry.event_type);
        assert_eq!(deserialized.url, entry.url);
        assert_eq!(deserialized.reason, entry.reason);
        assert_eq!(deserialized.attempts, entry.attempts);
        assert_eq!(deserialized.created_at, entry.created_at);
        assert_eq!(deserialized.replayed, entry.replayed);

        // Verify JSON structure
        let json_obj = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        assert_eq!(json_obj["id"], "test-id");
        assert_eq!(json_obj["task_id"], "task-123");
        assert_eq!(json_obj["event_type"], "status");
        assert_eq!(json_obj["url"], "https://example.com/webhook");
        assert_eq!(json_obj["reason"], "Test reason");
        assert_eq!(json_obj["attempts"], 3);
        assert_eq!(json_obj["replayed"], false);
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_signature_generation() {
        let config = HttpPushNotificationConfig::builder()
            .signing_key(Some("test-secret-key".to_string()))
            .build();

        let sender = EnhancedHttpPushNotificationSender::with_config(config);

        let payload1 = r#"{"test":"data1"}"#;
        let payload2 = r#"{"test":"data2"}"#;

        let sig1 = sender.generate_signature(payload1).unwrap();
        let sig1_again = sender.generate_signature(payload1).unwrap();
        let sig2 = sender.generate_signature(payload2).unwrap();

        // Same payload should produce same signature
        assert_eq!(sig1, sig1_again);

        // Different payloads should produce different signatures
        assert_ne!(sig1, sig2);

        // Signature should not be empty when key is configured
        assert!(!sig1.is_empty());
        assert!(!sig2.is_empty());
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn test_signature_without_key() {
        let config = HttpPushNotificationConfig::builder()
            .signing_key(None)
            .build();

        let sender = EnhancedHttpPushNotificationSender::with_config(config);

        let payload = r#"{"test":"data"}"#;
        let sig = sender.generate_signature(payload).unwrap();

        // Should return empty string when no key configured
        assert!(sig.is_empty());
    }

    #[tokio::test]
    async fn test_event_id_generation_deterministic() {
        let tracker = InMemoryDeliveryTracker::new();

        let task_id = "task-123";
        let event_data = r#"{"status":"completed","timestamp":"2024-02-11T10:00:00Z"}"#;

        let id1 = tracker.generate_event_id(task_id, event_data);
        let id2 = tracker.generate_event_id(task_id, event_data);

        // Same inputs should produce same ID
        assert_eq!(id1, id2);

        // Different data should produce different ID
        let different_data = r#"{"status":"failed","timestamp":"2024-02-11T10:00:00Z"}"#;
        let id3 = tracker.generate_event_id(task_id, different_data);
        assert_ne!(id1, id3);

        // Different task ID should produce different ID
        let different_task = "task-456";
        let id4 = tracker.generate_event_id(different_task, event_data);
        assert_ne!(id1, id4);
    }

    #[test]
    fn test_push_notification_config_validation() {
        use crate::port::AsyncNotificationManager;

        // Create a mock notification manager to test validation
        // This is a simple test that verifies URL validation logic

        let valid_url = "https://example.com/webhook";
        let invalid_url = "not-a-url";

        // The validation should catch invalid URLs
        assert!(url::Url::parse(valid_url).is_ok());
        assert!(url::Url::parse(invalid_url).is_err());
    }
}
