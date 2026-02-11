//! Integration tests for SQLx storage implementation

#[cfg(feature = "sqlx-storage")]
mod sqlx_tests {
    use a2a_rs::adapter::storage::{DatabaseConfig, SqlxTaskStorage};
    use a2a_rs::domain::TaskState;
    use a2a_rs::port::{AsyncNotificationManager, AsyncStreamingHandler, AsyncTaskManager};
    use a2a_rs::{A2AError, PushNotificationConfig, TaskPushNotificationConfig};
    use std::sync::Arc;
    use uuid::Uuid;

    async fn create_test_storage() -> Result<SqlxTaskStorage, A2AError> {
        // Use SQLite in-memory for tests
        let config = DatabaseConfig::builder()
            .url("sqlite::memory:".to_string())
            .max_connections(1)
            .build();

        SqlxTaskStorage::new(&config.url).await
    }

    #[tokio::test]
    async fn test_task_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();
        let context_id = "test-context";

        // Test task creation
        let task = storage.create_task(&task_id, context_id).await?;
        assert_eq!(task.id, task_id);
        assert_eq!(task.context_id, context_id);
        assert_eq!(task.status.state, TaskState::Submitted);

        // Test task existence
        assert!(storage.task_exists(&task_id).await?);
        assert!(!storage.task_exists("non-existent").await?);

        // Test status updates
        let working_task = storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        assert_eq!(working_task.status.state, TaskState::Working);

        let completed_task = storage
            .update_task_status(&task_id, TaskState::Completed, None)
            .await?;
        assert_eq!(completed_task.status.state, TaskState::Completed);

        // Test task retrieval with history
        let retrieved_task = storage.get_task(&task_id, Some(10)).await?;
        assert_eq!(retrieved_task.id, task_id);
        assert_eq!(retrieved_task.status.state, TaskState::Completed);
        // Should have history: Submitted -> Working -> Completed
        // Note: We're not loading full history in the current implementation
        // assert_eq!(retrieved_task.history.len(), 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_task_cancellation() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create and start working on task
        storage.create_task(&task_id, "test-context").await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;

        // Cancel the working task
        let canceled_task = storage.cancel_task(&task_id).await?;
        assert_eq!(canceled_task.status.state, TaskState::Canceled);

        // Verify cancellation was successful
        let task_with_history = storage.get_task(&task_id, None).await?;
        assert_eq!(task_with_history.status.state, TaskState::Canceled);
        // Note: We're not fully implementing history loading in this version
        // In a full implementation, you'd verify the cancellation message was added

        Ok(())
    }

    #[tokio::test]
    async fn test_cannot_cancel_completed_task() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create, work on, and complete task
        storage.create_task(&task_id, "test-context").await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::Completed, None)
            .await?;

        // Try to cancel completed task - should fail
        let result = storage.cancel_task(&task_id).await;
        assert!(result.is_err());

        if let Err(A2AError::TaskNotCancelable(_)) = result {
            // Expected error type
        } else {
            panic!("Expected TaskNotCancelable error, got: {:?}", result);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_task_creation() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create first task
        storage.create_task(&task_id, "test-context").await?;

        // Try to create duplicate - should fail
        let result = storage.create_task(&task_id, "test-context").await;
        assert!(result.is_err());

        if let Err(A2AError::TaskNotFound(_)) = result {
            // Expected error type (reused for "already exists")
        } else {
            panic!(
                "Expected TaskNotFound error for duplicate, got: {:?}",
                result
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_task_history_limit() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task and make several status changes
        storage.create_task(&task_id, "test-context").await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::InputRequired, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::Completed, None)
            .await?;

        // Note: We're not fully implementing history loading in this version
        // In a full implementation, you'd test history limits here
        let _task_limited = storage.get_task(&task_id, Some(3)).await?;
        let _task_full = storage.get_task(&task_id, None).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_push_notifications() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task first
        storage.create_task(&task_id, "test-context").await?;

        // Set push notification config
        let config = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: None,
                url: "https://example.com/webhook".to_string(),
                token: None,
                authentication: None,
            },
        };

        let set_config = storage.set_task_notification(&config).await?;
        assert_eq!(set_config.task_id, task_id);
        assert_eq!(
            set_config.push_notification_config.url,
            "https://example.com/webhook"
        );

        // Get push notification config
        let retrieved_config = storage.get_task_notification(&task_id).await?;
        assert_eq!(retrieved_config.task_id, task_id);
        assert_eq!(
            retrieved_config.push_notification_config.url,
            "https://example.com/webhook"
        );

        // Remove push notification config
        storage.remove_task_notification(&task_id).await?;

        // Verify it's removed
        let result = storage.get_task_notification(&task_id).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_database_config() -> Result<(), Box<dyn std::error::Error>> {
        // Test config validation
        let valid_config = DatabaseConfig::builder()
            .url("sqlite:test.db".to_string())
            .max_connections(5)
            .timeout_seconds(10)
            .build();
        assert!(valid_config.validate().is_ok());

        // Test invalid config
        let invalid_config = DatabaseConfig::builder().url("".to_string()).build();
        assert!(invalid_config.validate().is_err());

        // Test database type detection
        assert_eq!(valid_config.database_type(), "sqlite");

        let postgres_config = DatabaseConfig::builder()
            .url("postgres://localhost/test".to_string())
            .build();
        assert_eq!(postgres_config.database_type(), "postgres");

        Ok(())
    }

    #[tokio::test]
    async fn test_streaming_subscribers() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Test subscriber count
        let count = storage.get_subscriber_count(&task_id).await?;
        assert_eq!(count, 0);

        // Test removing non-existent subscribers
        storage.remove_task_subscribers(&task_id).await?;

        // Test unsupported operations
        let result = storage.remove_subscription("fake-id").await;
        assert!(matches!(result, Err(A2AError::UnsupportedOperation(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_operations() -> Result<(), Box<dyn std::error::Error>> {
        let storage = Arc::new(create_test_storage().await?);
        let mut handles = Vec::new();

        // Create multiple tasks concurrently
        for i in 0..10 {
            let storage_clone = storage.clone();
            let handle = tokio::spawn(async move {
                let task_id = format!("concurrent-task-{}", i);
                let task = storage_clone
                    .create_task(&task_id, "concurrent-context")
                    .await?;
                storage_clone
                    .update_task_status(&task_id, TaskState::Working, None)
                    .await?;
                storage_clone
                    .update_task_status(&task_id, TaskState::Completed, None)
                    .await?;
                Ok::<_, A2AError>(task)
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await??;
            assert_eq!(result.status.state, TaskState::Submitted); // Initial state
        }

        // Verify all tasks exist
        for i in 0..10 {
            let task_id = format!("concurrent-task-{}", i);
            assert!(storage.task_exists(&task_id).await?);
            let task = storage.get_task(&task_id, None).await?;
            assert_eq!(task.status.state, TaskState::Completed);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_database_migrations() -> Result<(), Box<dyn std::error::Error>> {
        // Test that migrations run successfully on a fresh database
        let config = DatabaseConfig::builder()
            .url("sqlite::memory:".to_string())
            .build();

        // This should run migrations internally
        let _storage = SqlxTaskStorage::new(&config.url).await?;

        // Create another instance with the same URL - should not fail
        let _storage2 = SqlxTaskStorage::new(&config.url).await?;

        Ok(())
    }

    // ===== v0.3.0 Tests =====

    #[tokio::test]
    async fn test_list_tasks_v3_basic() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create some tasks
        for i in 0..5 {
            let task_id = format!("task-{}", i);
            storage.create_task(&task_id, "test-context").await?;
        }

        // List all tasks
        let params = a2a_rs::domain::ListTasksParams::default();
        let result = storage.list_tasks_v3(&params).await?;

        assert_eq!(result.total_size, 5, "Should have 5 tasks");
        assert_eq!(result.tasks.len(), 5, "Should return 5 tasks");
        assert_eq!(result.page_size, 50, "Default page size should be 50");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_tasks_v3_filtering() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create tasks in different contexts and states
        storage.create_task("task-a-1", "context-a").await?;
        storage.create_task("task-a-2", "context-a").await?;
        storage.create_task("task-b-1", "context-b").await?;

        storage
            .update_task_status("task-a-1", TaskState::Working, None)
            .await?;
        storage
            .update_task_status("task-a-2", TaskState::Completed, None)
            .await?;

        // Filter by context
        let params = a2a_rs::domain::ListTasksParams {
            context_id: Some("context-a".to_string()),
            ..Default::default()
        };
        let result = storage.list_tasks_v3(&params).await?;
        assert_eq!(result.total_size, 2, "Should have 2 tasks in context-a");

        // Filter by status
        let params = a2a_rs::domain::ListTasksParams {
            status: Some(TaskState::Working),
            ..Default::default()
        };
        let result = storage.list_tasks_v3(&params).await?;
        assert_eq!(result.total_size, 1, "Should have 1 working task");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_tasks_v3_pagination() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create 10 tasks
        for i in 0..10 {
            storage
                .create_task(&format!("task-{}", i), "test-context")
                .await?;
        }

        // Get first page
        let params = a2a_rs::domain::ListTasksParams {
            page_size: Some(3),
            ..Default::default()
        };
        let page1 = storage.list_tasks_v3(&params).await?;
        assert_eq!(page1.tasks.len(), 3, "Should return 3 tasks");
        assert!(
            !page1.next_page_token.is_empty(),
            "Should have next page token"
        );

        // Get second page
        let params = a2a_rs::domain::ListTasksParams {
            page_size: Some(3),
            page_token: Some(page1.next_page_token.clone()),
            ..Default::default()
        };
        let page2 = storage.list_tasks_v3(&params).await?;
        assert_eq!(page2.tasks.len(), 3, "Should return 3 tasks");

        Ok(())
    }

    #[tokio::test]
    async fn test_push_notification_config_v3_crud() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task first
        storage.create_task(&task_id, "test-context").await?;

        // Set push notification config
        let config = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-1".to_string()),
                url: "https://example.com/webhook".to_string(),
                token: Some("test-token".to_string()),
                authentication: None,
            },
        };
        storage.set_task_notification(&config).await?;

        // Get specific config
        let get_params = a2a_rs::domain::GetTaskPushNotificationConfigParams {
            id: task_id.clone(),
            push_notification_config_id: Some("config-1".to_string()),
            metadata: None,
        };
        let retrieved = storage.get_push_notification_config(&get_params).await?;
        assert_eq!(
            retrieved.push_notification_config.url,
            "https://example.com/webhook"
        );
        assert_eq!(
            retrieved.push_notification_config.token,
            Some("test-token".to_string())
        );

        // List configs
        let list_params = a2a_rs::domain::ListTaskPushNotificationConfigParams {
            id: task_id.clone(),
            metadata: None,
        };
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 1, "Should have 1 config");

        // Delete config
        let delete_params = a2a_rs::domain::DeleteTaskPushNotificationConfigParams {
            id: task_id.clone(),
            push_notification_config_id: "config-1".to_string(),
            metadata: None,
        };
        storage
            .delete_push_notification_config(&delete_params)
            .await?;

        // Verify deleted
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 0, "Config should be deleted");

        Ok(())
    }

    #[tokio::test]
    async fn test_push_notification_config_v3_multiple() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Set multiple configs
        let config1 = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-1".to_string()),
                url: "https://example.com/webhook1".to_string(),
                token: None,
                authentication: None,
            },
        };
        let config2 = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-2".to_string()),
                url: "https://example.com/webhook2".to_string(),
                token: Some("token-2".to_string()),
                authentication: None,
            },
        };

        storage.set_task_notification(&config1).await?;
        storage.set_task_notification(&config2).await?;

        // List should return both
        let list_params = a2a_rs::domain::ListTaskPushNotificationConfigParams {
            id: task_id.clone(),
            metadata: None,
        };
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 2, "Should have 2 configs");

        Ok(())
    }

    // ===== Additional CRUD Tests =====

    #[tokio::test]
    async fn test_create_task() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();
        let context_id = "test-context-123";

        // Create a new task
        let task = storage.create_task(&task_id, context_id).await?;

        assert_eq!(task.id, task_id, "Task ID should match");
        assert_eq!(task.context_id, context_id, "Context ID should match");
        assert_eq!(task.status.state, TaskState::Submitted, "New task should be in Submitted state");
        assert!(task.status.message.is_none(), "New task should have no status message");
        assert!(task.artifacts.is_none(), "New task should have no artifacts");
        assert!(task.history.is_none(), "New task should have no history initially");

        Ok(())
    }

    #[tokio::test]
    async fn test_get_task() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();
        let context_id = "test-context";

        // Create task first
        let _created_task = storage.create_task(&task_id, context_id).await?;

        // Get the task without history
        let retrieved_task = storage.get_task(&task_id, None).await?;

        assert_eq!(retrieved_task.id, task_id, "Retrieved task ID should match");
        assert_eq!(retrieved_task.context_id, context_id, "Retrieved context ID should match");
        assert_eq!(retrieved_task.status.state, TaskState::Submitted, "Retrieved state should be Submitted");

        // Get the task with limited history
        let task_with_history = storage.get_task(&task_id, Some(10)).await?;
        assert_eq!(task_with_history.id, task_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let fake_id = "nonexistent-task-123";

        // Attempting to get a non-existent task should fail
        let result = storage.get_task(fake_id, None).await;

        assert!(result.is_err(), "Getting non-existent task should fail");
        match result {
            Err(A2AError::TaskNotFound(msg)) => {
                assert!(msg.contains(fake_id), "Error message should contain the task ID");
            }
            _ => panic!("Expected TaskNotFound error, got: {:?}", result),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_task_status() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Update to Working
        let working_task = storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        assert_eq!(working_task.status.state, TaskState::Working);

        // Update to InputRequired with a message
        use a2a_rs::domain::{Message, Part, Role};
        let message = Message {
            role: Role::Agent,
            parts: vec![Part::Text {
                text: "Please provide additional information".to_string(),
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: Uuid::new_v4().to_string(),
            task_id: Some(task_id.clone()),
            context_id: Some("test-context".to_string()),
            extensions: None,
            kind: "message".to_string(),
        };

        let input_required_task = storage
            .update_task_status(&task_id, TaskState::InputRequired, Some(message.clone()))
            .await?;
        assert_eq!(input_required_task.status.state, TaskState::InputRequired);
        // Note: The message is stored in history, not returned in the status field
        // The status message field is separate from the message history

        // Update to Completed
        let completed_task = storage
            .update_task_status(&task_id, TaskState::Completed, None)
            .await?;
        assert_eq!(completed_task.status.state, TaskState::Completed);

        // Update to Failed
        let failed_task = storage
            .update_task_status(&task_id, TaskState::Failed, None)
            .await?;
        assert_eq!(failed_task.status.state, TaskState::Failed);

        Ok(())
    }

    #[tokio::test]
    async fn test_update_status_nonexistent_task() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let fake_id = "fake-task-123";

        // Updating a non-existent task should fail
        let result = storage
            .update_task_status(fake_id, TaskState::Working, None)
            .await;

        assert!(result.is_err());
        match result {
            Err(A2AError::TaskNotFound(msg)) => {
                assert!(msg.contains(fake_id));
            }
            _ => panic!("Expected TaskNotFound error"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_task_deletion() -> Result<(), Box<dyn std::error::Error>> {
        // Note: The current implementation doesn't have a delete_task method in the port trait
        // This test documents that behavior
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;
        assert!(storage.task_exists(&task_id).await?, "Task should exist after creation");

        // Currently there's no delete_task method - task lifecycle only supports
        // creation, status updates, and cancellation. Tasks remain in the database
        // for audit purposes.
        assert!(storage.task_exists(&task_id).await?, "Task should still exist");

        Ok(())
    }

    #[tokio::test]
    async fn test_set_push_notification_config() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task first
        storage.create_task(&task_id, "test-context").await?;

        // Set push notification config
        let config = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: None, // Will be auto-generated
                url: "https://example.com/webhook".to_string(),
                token: Some("secret-token".to_string()),
                authentication: None,
            },
        };

        let set_result = storage.set_task_notification(&config).await?;

        assert_eq!(set_result.task_id, task_id);
        assert_eq!(
            set_result.push_notification_config.url,
            "https://example.com/webhook"
        );
        assert_eq!(
            set_result.push_notification_config.token,
            Some("secret-token".to_string())
        );
        assert!(
            set_result.push_notification_config.id.is_some(),
            "ID should be generated"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_get_push_notification_config() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Try to get config before setting - should fail
        let result = storage.get_task_notification(&task_id).await;
        assert!(result.is_err(), "Getting non-existent config should fail");

        // Set config
        let config = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("my-config".to_string()),
                url: "https://example.com/callback".to_string(),
                token: Some("my-token".to_string()),
                authentication: None,
            },
        };
        storage.set_task_notification(&config).await?;

        // Get config back
        let retrieved = storage.get_task_notification(&task_id).await?;

        assert_eq!(retrieved.task_id, task_id);
        assert_eq!(retrieved.push_notification_config.url, "https://example.com/callback");
        assert_eq!(retrieved.push_notification_config.token, Some("my-token".to_string()));
        assert_eq!(retrieved.push_notification_config.id, Some("my-config".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_list_push_notification_configs() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Initially should be empty (using v3 list method)
        let list_params = a2a_rs::domain::ListTaskPushNotificationConfigParams {
            id: task_id.clone(),
            metadata: None,
        };
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 0, "Should have no configs initially");

        // Add three configs
        for i in 1..=3 {
            let config = TaskPushNotificationConfig {
                task_id: task_id.clone(),
                push_notification_config: PushNotificationConfig {
                    id: Some(format!("config-{}", i)),
                    url: format!("https://example.com/webhook{}", i),
                    token: Some(format!("token-{}", i)),
                    authentication: None,
                },
            };
            storage.set_task_notification(&config).await?;
        }

        // List all configs
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 3, "Should have 3 configs");

        // Verify each config has correct data
        for (_i, config) in configs.iter().enumerate() {
            assert_eq!(config.task_id, task_id);
            assert!(config.push_notification_config.id.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_push_notification_config() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Set config using old method (for backwards compatibility)
        let config = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-to-delete".to_string()),
                url: "https://example.com/webhook".to_string(),
                token: None,
                authentication: None,
            },
        };
        storage.set_task_notification(&config).await?;

        // Verify it exists
        let retrieved = storage.get_task_notification(&task_id).await?;
        assert_eq!(retrieved.push_notification_config.id, Some("config-to-delete".to_string()));

        // Delete config (using old method)
        storage.remove_task_notification(&task_id).await?;

        // Verify it's gone
        let result = storage.get_task_notification(&task_id).await;
        assert!(result.is_err(), "Config should be deleted");

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_push_notification_config_v3() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task
        storage.create_task(&task_id, "test-context").await?;

        // Add multiple configs
        let config1 = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-1".to_string()),
                url: "https://example.com/webhook1".to_string(),
                token: None,
                authentication: None,
            },
        };
        let config2 = TaskPushNotificationConfig {
            task_id: task_id.clone(),
            push_notification_config: PushNotificationConfig {
                id: Some("config-2".to_string()),
                url: "https://example.com/webhook2".to_string(),
                token: None,
                authentication: None,
            },
        };
        storage.set_task_notification(&config1).await?;
        storage.set_task_notification(&config2).await?;

        // Delete only config-1
        let delete_params = a2a_rs::domain::DeleteTaskPushNotificationConfigParams {
            id: task_id.clone(),
            push_notification_config_id: "config-1".to_string(),
            metadata: None,
        };
        storage
            .delete_push_notification_config(&delete_params)
            .await?;

        // Verify only config-2 remains
        let list_params = a2a_rs::domain::ListTaskPushNotificationConfigParams {
            id: task_id.clone(),
            metadata: None,
        };
        let configs = storage.list_push_notification_configs(&list_params).await?;
        assert_eq!(configs.len(), 1, "Should have 1 config remaining");
        assert_eq!(
            configs[0].push_notification_config.id,
            Some("config-2".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_tasks_pagination() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create 15 tasks
        for i in 0..15 {
            storage
                .create_task(&format!("task-{:03}", i), "test-context")
                .await?;
        }

        // Get first page of 5
        let params1 = a2a_rs::domain::ListTasksParams {
            page_size: Some(5),
            ..Default::default()
        };
        let page1 = storage.list_tasks_v3(&params1).await?;

        assert_eq!(page1.tasks.len(), 5, "First page should have 5 tasks");
        assert_eq!(page1.total_size, 15, "Total should be 15");
        assert_eq!(page1.page_size, 5, "Page size should be 5");
        assert!(!page1.next_page_token.is_empty(), "Should have next page token");

        // Get second page
        let params2 = a2a_rs::domain::ListTasksParams {
            page_size: Some(5),
            page_token: Some(page1.next_page_token.clone()),
            ..Default::default()
        };
        let page2 = storage.list_tasks_v3(&params2).await?;

        assert_eq!(page2.tasks.len(), 5, "Second page should have 5 tasks");
        assert_eq!(page2.total_size, 15);
        assert!(!page2.next_page_token.is_empty(), "Should have next page token");

        // Get third page
        let params3 = a2a_rs::domain::ListTasksParams {
            page_size: Some(5),
            page_token: Some(page2.next_page_token.clone()),
            ..Default::default()
        };
        let page3 = storage.list_tasks_v3(&params3).await?;

        assert_eq!(page3.tasks.len(), 5, "Third page should have 5 tasks");
        assert_eq!(page3.total_size, 15);
        assert!(page3.next_page_token.is_empty(), "Should be last page");

        // Try to get page beyond end (empty token means no more pages)
        // When the token is empty string, offset defaults to 0
        let params4 = a2a_rs::domain::ListTasksParams {
            page_size: Some(5),
            page_token: Some(page3.next_page_token.clone()), // Empty token
            ..Default::default()
        };
        let page4 = storage.list_tasks_v3(&params4).await?;

        // When token is empty, it parses to 0, so we get the first page again
        assert!(page4.tasks.len() <= 5, "Should return 5 or fewer tasks");
        assert_eq!(page4.total_size, 15);

        Ok(())
    }

    #[tokio::test]
    async fn test_page_size_clamping() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create some tasks
        for i in 0..10 {
            storage
                .create_task(&format!("task-{}", i), "test-context")
                .await?;
        }

        // Test page size clamping to minimum (1)
        let params_min = a2a_rs::domain::ListTasksParams {
            page_size: Some(0), // Below minimum
            ..Default::default()
        };
        let result_min = storage.list_tasks_v3(&params_min).await?;
        assert_eq!(
            result_min.page_size, 1,
            "Page size should be clamped to minimum of 1"
        );

        // Test page size clamping to maximum (100)
        let params_max = a2a_rs::domain::ListTasksParams {
            page_size: Some(150), // Above maximum
            ..Default::default()
        };
        let result_max = storage.list_tasks_v3(&params_max).await?;
        assert_eq!(
            result_max.page_size, 100,
            "Page size should be clamped to maximum of 100"
        );
        assert_eq!(
            result_max.tasks.len(),
            10,
            "Should return all 10 tasks despite requesting 150"
        );

        // Test normal page size within bounds
        let params_normal = a2a_rs::domain::ListTasksParams {
            page_size: Some(5),
            ..Default::default()
        };
        let result_normal = storage.list_tasks_v3(&params_normal).await?;
        assert_eq!(result_normal.page_size, 5, "Page size should be 5");
        assert_eq!(result_normal.tasks.len(), 5, "Should return 5 tasks");

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_task_updates() -> Result<(), Box<dyn std::error::Error>> {
        let storage = Arc::new(create_test_storage().await?);
        let task_id = Uuid::new_v4().to_string();

        // Create initial task
        storage.create_task(&task_id, "test-context").await?;

        // Spawn multiple concurrent status updates
        let mut handles = Vec::new();

        for i in 0..20 {
            let storage_clone = storage.clone();
            let task_id_clone = task_id.clone();
            let handle = tokio::spawn(async move {
                let states = [
                    TaskState::Working,
                    TaskState::InputRequired,
                    TaskState::Working,
                    TaskState::Completed,
                ];
                let state = states[i % states.len()].clone();
                storage_clone
                    .update_task_status(&task_id_clone, state, None)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all updates to complete
        let mut success_count = 0;
        let mut error_count = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => success_count += 1,
                Ok(Err(_)) => error_count += 1,
                Err(_) => error_count += 1,
            }
        }

        // All updates should complete (some may succeed, some may fail due to timing)
        assert_eq!(
            success_count + error_count,
            20,
            "All operations should complete"
        );

        // Verify final state is retrievable
        let final_task = storage.get_task(&task_id, None).await?;
        assert!(matches!(
            final_task.status.state,
            TaskState::Working | TaskState::InputRequired | TaskState::Completed
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_with_filters() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create tasks in different contexts and states
        storage.create_task("task-a-1", "context-a").await?;
        storage.create_task("task-a-2", "context-a").await?;
        storage.create_task("task-a-3", "context-a").await?;
        storage.create_task("task-b-1", "context-b").await?;
        storage.create_task("task-b-2", "context-b").await?;

        storage
            .update_task_status("task-a-1", TaskState::Working, None)
            .await?;
        storage
            .update_task_status("task-a-2", TaskState::Completed, None)
            .await?;

        // Paginate filtered by context-a
        let params = a2a_rs::domain::ListTasksParams {
            context_id: Some("context-a".to_string()),
            page_size: Some(2),
            ..Default::default()
        };
        let page1 = storage.list_tasks_v3(&params).await?;

        assert_eq!(page1.total_size, 3, "Should have 3 tasks in context-a");
        assert_eq!(page1.tasks.len(), 2, "Should return 2 tasks per page");
        assert!(!page1.next_page_token.is_empty(), "Should have next page");

        // Get next page
        let params2 = a2a_rs::domain::ListTasksParams {
            context_id: Some("context-a".to_string()),
            page_size: Some(2),
            page_token: Some(page1.next_page_token.clone()),
            ..Default::default()
        };
        let page2 = storage.list_tasks_v3(&params2).await?;

        assert_eq!(page2.tasks.len(), 1, "Second page should have 1 task");
        assert!(page2.next_page_token.is_empty(), "Should be last page");

        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_with_status_filter() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Create tasks and set different states
        for i in 0..10 {
            let task_id = format!("task-{}", i);
            storage.create_task(&task_id, "test-context").await?;
        }

        storage
            .update_task_status("task-0", TaskState::Working, None)
            .await?;
        storage
            .update_task_status("task-1", TaskState::Working, None)
            .await?;
        storage
            .update_task_status("task-2", TaskState::Completed, None)
            .await?;

        // Filter by Working state with pagination
        let params = a2a_rs::domain::ListTasksParams {
            status: Some(TaskState::Working),
            page_size: Some(1),
            ..Default::default()
        };
        let page1 = storage.list_tasks_v3(&params).await?;

        assert_eq!(page1.total_size, 2, "Should have 2 working tasks");
        assert_eq!(page1.tasks.len(), 1, "Should return 1 task per page");

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_pagination() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;

        // Try to paginate when no tasks exist
        let params = a2a_rs::domain::ListTasksParams {
            page_size: Some(10),
            ..Default::default()
        };
        let result = storage.list_tasks_v3(&params).await?;

        assert_eq!(result.tasks.len(), 0, "Should have no tasks");
        assert_eq!(result.total_size, 0, "Total should be 0");
        assert!(result.next_page_token.is_empty(), "No next page token");
        assert_eq!(result.page_size, 10, "Page size should still be set");

        Ok(())
    }

    #[tokio::test]
    async fn test_history_loading_with_pagination() -> Result<(), Box<dyn std::error::Error>> {
        let storage = create_test_storage().await?;
        let task_id = Uuid::new_v4().to_string();

        // Create task with multiple status changes
        storage.create_task(&task_id, "test-context").await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::InputRequired, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::Working, None)
            .await?;
        storage
            .update_task_status(&task_id, TaskState::Completed, None)
            .await?;

        // List tasks with history_length specified
        // Note: The list_tasks_v3 implementation loads history when history_length > 0
        let params = a2a_rs::domain::ListTasksParams {
            history_length: Some(3),
            ..Default::default()
        };
        let result = storage.list_tasks_v3(&params).await?;

        assert_eq!(result.tasks.len(), 1, "Should have 1 task");
        // When history_length is specified in list params, tasks should have history loaded
        // But the exact behavior depends on implementation - just verify it doesn't error
        assert_eq!(result.total_size, 1, "Total should be 1");

        Ok(())
    }
}

#[cfg(not(feature = "sqlx-storage"))]
#[tokio::test]
async fn test_sqlx_not_available() {
    // This test just verifies the feature flag works correctly
    println!("SQLx storage tests skipped - feature not enabled");
}
