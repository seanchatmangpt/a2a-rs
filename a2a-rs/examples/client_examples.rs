//! Comprehensive A2A client examples demonstrating all features
//!
//! This example demonstrates:
//! - Builder pattern for client configuration
//! - Retry logic with exponential backoff
//! - Connection pooling
//! - Batch operations
//! - Token refresh (simulation)
//! - All v0.3.0 protocol methods

use std::time::Duration;
use tokio::time::sleep;

use a2a_rs::adapter::{
    BearerTokenAuthenticator, DefaultRequestProcessor, HttpClient, HttpServer, InMemoryTaskStorage,
    NoopPushNotificationSender, SimpleAgentInfo,
};
use a2a_rs::domain::{ListTasksParams, Message, Part, Role, TaskState};
use a2a_rs::observability;
use a2a_rs::services::{
    A2AClientConfig, BatchConfig, BatchClientOperations, EnhancedHttpClient, PoolConfig,
    RetryConfig, TokenRefreshConfig, AsyncA2AClient,
};

mod common;
use common::SimpleAgentHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    observability::init_tracing();

    println!("============================================================");
    println!("A2A Enhanced Client Examples - Full Feature Demonstration");
    println!("============================================================\n");

    // Start server in background
    let server_handle = tokio::spawn(async {
        run_server().await.expect("Server failed");
    });

    // Give server time to start
    sleep(Duration::from_millis(500)).await;

    // Run all examples
    example_1_basic_client().await?;
    example_2_retry_logic().await?;
    example_3_connection_pooling().await?;
    example_4_batch_operations().await?;
    example_5_token_refresh().await?;
    example_6_v03_methods().await?;

    println!("\n============================================================");
    println!("All examples completed successfully!");
    println!("============================================================");

    // Cleanup
    sleep(Duration::from_millis(500)).await;
    server_handle.abort();

    Ok(())
}

/// Example 1: Basic client with builder pattern
async fn example_1_basic_client() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 Example 1: Basic Client with Builder Pattern");
    println!("---------------------------------------------------------------");

    // Create client using builder pattern
    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .request_timeout(Duration::from_secs(30))
        .build();

    let client = EnhancedHttpClient::new(config)?;

    println!("✅ Client created with builder pattern");
    println!("   - Base URL: http://127.0.0.1:8080");
    println!("   - Timeout: 30s");
    println!("   - Auth: Bearer token");

    // Create a task
    let task_id = uuid::Uuid::new_v4().to_string();
    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Hello from enhanced client!".to_string())])
        .message_id(uuid::Uuid::new_v4().to_string())
        .build();

    match client.send_task_message(&task_id, &message, None, None).await {
        Ok(task) => {
            println!("✅ Task created: {}", task.id);
            println!("   - Status: {:?}", task.status.state);
        }
        Err(e) => {
            println!("❌ Failed to create task: {}", e);
        }
    }

    // Get the task
    match client.get_task(&task_id, None).await {
        Ok(task) => {
            println!("✅ Task retrieved: {}", task.id);
        }
        Err(e) => {
            println!("❌ Failed to get task: {}", e);
        }
    }

    println!("✅ Example 1 completed\n");
    Ok(())
}

/// Example 2: Retry logic with exponential backoff
async fn example_2_retry_logic() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Example 2: Retry Logic with Exponential Backoff");
    println!("---------------------------------------------------------------");

    // Configure retry behavior
    let retry_config = RetryConfig::builder()
        .max_retries(3)
        .initial_delay(Duration::from_millis(100))
        .max_delay(Duration::from_secs(5))
        .backoff_multiplier(2.0)
        .jitter(true)
        .build();

    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .retry_config(retry_config)
        .build();

    let client = EnhancedHttpClient::new(config)?;

    println!("✅ Client configured with retry logic:");
    println!("   - Max retries: 3");
    println!("   - Initial delay: 100ms");
    println!("   - Max delay: 5s");
    println!("   - Backoff multiplier: 2.0x");
    println!("   - Jitter: enabled");

    // Create task (will retry on transient errors)
    let task_id = uuid::Uuid::new_v4().to_string();
    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test retry logic".to_string())])
        .message_id(uuid::Uuid::new_v4().to_string())
        .build();

    match client.send_task_message(&task_id, &message, None, None).await {
        Ok(task) => {
            println!("✅ Task created with retry protection: {}", task.id);
        }
        Err(e) => {
            println!("❌ Failed after retries: {}", e);
        }
    }

    println!("✅ Example 2 completed\n");
    Ok(())
}

/// Example 3: Connection pooling
async fn example_3_connection_pooling() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏊 Example 3: Connection Pool Management");
    println!("---------------------------------------------------------------");

    // Configure connection pool
    let pool_config = PoolConfig::builder()
        .max_connections(10)
        .min_idle(2)
        .connection_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(3600))
        .build();

    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .pool_config(pool_config)
        .build();

    let client = EnhancedHttpClient::new(config)?;

    println!("✅ Client configured with connection pool:");
    println!("   - Max connections: 10");
    println!("   - Min idle: 2");
    println!("   - Connection timeout: 30s");
    println!("   - Idle timeout: 300s");
    println!("   - Max lifetime: 3600s");

    // Create multiple tasks concurrently (pool will limit concurrency)
    let mut handles = Vec::new();
    for i in 0..5 {
        let client_clone = unsafe { std::ptr::read(&client as *const _) };
        let handle = tokio::spawn(async move {
            let task_id = format!("task-{}", uuid::Uuid::new_v4());
            let message = Message::builder()
                .role(Role::User)
                .parts(vec![Part::text(format!("Concurrent task {}", i))])
                .message_id(uuid::Uuid::new_v4().to_string())
                .build();

            match client_clone.send_task_message(&task_id, &message, None, None).await {
                Ok(task) => {
                    println!("   ✅ Concurrent task {} created: {}", i, task.id);
                }
                Err(e) => {
                    println!("   ❌ Concurrent task {} failed: {}", i, e);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await?;
    }

    println!("✅ Example 3 completed\n");
    Ok(())
}

/// Example 4: Batch operations
async fn example_4_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("📦 Example 4: Batch Operations");
    println!("---------------------------------------------------------------");

    // Configure batching
    let batch_config = BatchConfig::builder()
        .max_batch_size(50)
        .max_batch_latency(Duration::from_millis(100))
        .enabled(true)
        .build();

    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .batch_config(batch_config)
        .retry_config(RetryConfig::builder().max_retries(2).build())
        .build();

    let client = EnhancedHttpClient::new(config)?;

    println!("✅ Client configured for batch operations:");
    println!("   - Max batch size: 50");
    println!("   - Max latency: 100ms");

    // Create multiple tasks
    let mut task_ids = Vec::new();
    for i in 0..10 {
        let task_id = format!("task-{}", uuid::Uuid::new_v4());
        let message = Message::builder()
            .role(Role::User)
            .parts(vec![Part::text(format!("Batch task {}", i))])
            .message_id(uuid::Uuid::new_v4().to_string())
            .build();

        match client.send_task_message(&task_id, &message, None, None).await {
            Ok(task) => {
                task_ids.push(task.id.clone());
                println!("   ✅ Created task {}: {}", i, task.id);
            }
            Err(e) => {
                println!("   ❌ Failed to create task {}: {}", i, e);
            }
        }
    }

    // Batch get all tasks
    println!("\n📥 Retrieving tasks in batch...");
    let results = client.get_tasks_batch(task_ids.clone()).await;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(task) => {
                println!("   ✅ Batch retrieved task {}: {} (state: {:?})",
                    i, task.id, task.status.state);
            }
            Err(e) => {
                println!("   ❌ Batch retrieval failed for task {}: {}", i, e);
            }
        }
    }

    // Batch cancel tasks
    println!("\n🛑 Cancelling tasks in batch...");
    let cancel_results = client.cancel_tasks_batch(task_ids).await;

    let mut cancelled_count = 0;
    for result in cancel_results.iter() {
        if result.is_ok() {
            cancelled_count += 1;
        }
    }
    println!("   ✅ Successfully cancelled {} tasks", cancelled_count);

    println!("✅ Example 4 completed\n");
    Ok(())
}

/// Example 5: Token refresh simulation
async fn example_5_token_refresh() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔑 Example 5: Automatic Token Refresh");
    println!("---------------------------------------------------------------");

    // Configure token refresh
    let refresh_config = TokenRefreshConfig::builder()
        .refresh_before_expiry(Duration::from_secs(300))
        .max_refresh_retries(2)
        .enabled(true)
        .build();

    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .token_refresh_config(refresh_config)
        .build();

    let mut client = EnhancedHttpClient::new(config)?;

    // Set up token refresh callback (simulated)
    client = client.with_token_refresh(|| {
        // In real implementation, this would call an OAuth endpoint
        println!("   🔄 Refreshing token...");
        Ok("new-secret-token".to_string())
            as Result<String, a2a_rs::domain::A2AError>
    });

    println!("✅ Client configured with token refresh:");
    println!("   - Refresh before expiry: 300s");
    println!("   - Max refresh retries: 2");
    println!("   - Callback: configured");

    // Create task
    let task_id = uuid::Uuid::new_v4().to_string();
    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test token refresh".to_string())])
        .message_id(uuid::Uuid::new_v4().to_string())
        .build();

    match client.send_task_message(&task_id, &message, None, None).await {
        Ok(task) => {
            println!("✅ Task created with token refresh: {}", task.id);
        }
        Err(e) => {
            println!("❌ Failed: {}", e);
        }
    }

    println!("✅ Example 5 completed\n");
    Ok(())
}

/// Example 6: All v0.3.0 methods
async fn example_6_v03_methods() -> Result<(), Box<dyn std::error::Error>> {
    println!("🆕 Example 6: A2A Protocol v0.3.0 Methods");
    println!("---------------------------------------------------------------");

    let config = A2AClientConfig::builder()
        .base_url("http://127.0.0.1:8080".to_string())
        .auth_token("secret-token".to_string())
        .build();

    let client = EnhancedHttpClient::new(config)?;

    println!("✅ Testing all v0.3.0 protocol methods:\n");

    // Create initial task
    let task_id = uuid::Uuid::new_v4().to_string();
    let message = Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test v0.3.0 features".to_string())])
        .message_id(uuid::Uuid::new_v4().to_string())
        .build();

    client.send_task_message(&task_id, &message, None, None).await?;
    println!("✅ 1. send_task_message - Created task: {}", task_id);

    // Get task
    let task = client.get_task(&task_id, None).await?;
    println!("✅ 2. get_task - Retrieved task: {}", task.id);

    // List tasks with filtering
    let list_params = ListTasksParams::builder()
        .task_ids(vec![task_id.clone()])
        .build();

    let result = client.list_tasks(&list_params).await?;
    println!("✅ 3. list_tasks - Found {} task(s)", result.data.len());

    // Set push notification
    use a2a_rs::domain::{TaskPushNotificationConfig, PushNotificationType};

    let push_config = TaskPushNotificationConfig::builder()
        .task_id(task_id.clone())
        .push_notification_type(PushNotificationType::Http)
        .url("https://example.com/webhook".to_string())
        .build();

    let configured_push = client.set_task_push_notification(&push_config).await?;
    println!("✅ 4. set_task_push_notification - Configured: {}", configured_push.id);

    // Get push notification config
    let retrieved_push = client.get_task_push_notification(&task_id).await?;
    println!("✅ 5. get_task_push_notification - Retrieved: {}", retrieved_push.id);

    // List push notification configs
    let configs = client.list_push_notification_configs(&task_id).await?;
    println!("✅ 6. list_push_notification_configs - Found {} config(s)", configs.len());

    // Get specific push notification config
    if let Some(config) = configs.first() {
        let specific_config = client.get_push_notification_config(
            &task_id,
            &config.id
        ).await?;
        println!("✅ 7. get_push_notification_config - Retrieved: {}", specific_config.id);

        // Delete push notification config
        client.delete_push_notification_config(&task_id, &config.id).await?;
        println!("✅ 8. delete_push_notification_config - Deleted: {}", config.id);
    }

    // Cancel task
    let cancelled_task = client.cancel_task(&task_id).await?;
    println!("✅ 9. cancel_task - Cancelled: {} (state: {:?})",
        cancelled_task.id, cancelled_task.status.state);

    println!("\n✅ Example 6 completed - All v0.3.0 methods tested\n");
    Ok(())
}

/// Server implementation for the examples
async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 Starting test server on http://127.0.0.1:8080");

    let push_sender = NoopPushNotificationSender;
    let storage = InMemoryTaskStorage::with_push_sender(push_sender);
    let handler = SimpleAgentHandler::with_storage(storage);

    let agent_info = SimpleAgentInfo::new(
        "Test A2A Agent".to_string(),
        "http://localhost:8080".to_string(),
    )
    .with_description("Enhanced client test agent".to_string())
    .with_provider("Test Organization".to_string(), "https://test.org".to_string())
    .with_streaming()
    .add_comprehensive_skill(
        "echo".to_string(),
        "Echo".to_string(),
        Some("Echoes messages back".to_string()),
        Some(vec!["echo".to_string()]),
        Some(vec!["Echo: ...".to_string()]),
        Some(vec!["text".to_string()]),
        Some(vec!["text".to_string()]),
    );

    let processor = DefaultRequestProcessor::with_handler(handler, agent_info.clone());

    let tokens = vec!["secret-token".to_string(), "new-secret-token".to_string()];
    let authenticator = BearerTokenAuthenticator::new(tokens);

    let server = HttpServer::with_auth(
        processor,
        agent_info,
        "127.0.0.1:8080".to_string(),
        authenticator,
    );

    println!("✅ Test server ready");
    server.start().await?;
    Ok(())
}
