//! Production-ready HTTP server example with middleware, health checks, and OpenAPI
//!
//! This example demonstrates:
//! - CORS configuration
//! - Rate limiting
//! - Request validation
//! - Response compression
//! - Health check endpoints
//! - OpenAPI spec generation
//! - Graceful shutdown
//!
//! Run with:
//!   cargo run --example production_http_server --features "http-server,server,tracing"

use a2a_rs::{
    adapter::transport::http::{
        CorsConfig, CompressionConfig, HttpServer, RateLimitConfig,
        ValidationConfig, HealthChecker, HealthStatus, OpenApiBuilder,
    },
    domain::{AgentCard, AgentCapabilities, AgentSkill},
    port::{AsyncMessageHandler, AsyncTaskManager},
    services::server::{
        DefaultRequestProcessor, SimpleAgentInfo, InMemoryTaskStorage, NoopPushNotificationSender,
    },
    A2AError, Message, Role, Task, TaskState,
};

use std::{sync::Arc, time::Duration};

/// Simple message handler that echoes messages
#[derive(Clone)]
struct EchoMessageHandler;

#[async_trait::async_trait]
impl AsyncMessageHandler for EchoMessageHandler {
    async fn handle_message(&self, task_id: &str, message: &Message) -> Result<Message, A2AError> {
        tracing::info!(
            task_id,
            role = ?message.role,
            "Handling message"
        );

        Ok(Message {
            role: Role::Agent,
            parts: vec![],
            metadata: None,
            timestamp: Some(chrono::Utc::now()),
        })
    }
}

/// Simple task manager using in-memory storage
#[derive(Clone)]
struct SimpleTaskManager {
    storage: Arc<InMemoryTaskStorage>,
}

#[async_trait::async_trait]
impl AsyncTaskManager for SimpleTaskManager {
    async fn create_task(&self, task: Task) -> Result<Task, A2AError> {
        self.storage.create_task(task).await
    }

    async fn get_task(&self, task_id: &str) -> Result<Task, A2AError> {
        self.storage.get_task(task_id).await
    }

    async fn list_tasks(
        &self,
        params: &a2a_rs::ListTasksParams,
    ) -> Result<a2a_rs::ListTasksResult, A2AError> {
        self.storage.list_tasks(params).await
    }

    async fn cancel_task(&self, task_id: &str) -> Result<Task, A2AError> {
        let mut task = self.storage.get_task(task_id).await?;
        task.status = TaskState::Canceled;
        self.storage.update_task(task).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,a2a_rs=debug".into()),
        )
        .init();

    // Create agent card
    let agent_card = AgentCard {
        agent_id: "production-example-agent".to_string(),
        display_name: "Production Example Agent".to_string(),
        description: Some("Production-ready A2A agent with middleware".to_string()),
        version: Some("1.0.0".to_string()),
        capabilities: Some(AgentCapabilities {
            streaming: Some(false),
            push_notifications: Some(false),
            ..Default::default()
        }),
        skills: vec
![AgentSkill {
            id: "echo".to_string(),
            display_name: "Echo".to_string(),
            description: Some("Echoes back messages".to_string()),
            input_modes: Some(vec
!["text/plain".to_string()]),
            output_modes: Some(vec
!["text/plain".to_string()]),
            ..Default::default()
        }],
        extensions: None,
        provider: None,
    };

    let agent_info = SimpleAgentInfo::from_card(agent_card);

    // Create task storage
    let storage = Arc::new(InMemoryTaskStorage::new());

    // Create handlers
    let message_handler = EchoMessageHandler;
    let task_manager = SimpleTaskManager {
        storage: storage.clone(),
    };
    let notification_sender = NoopPushNotificationSender;

    // Create request processor
    let processor = DefaultRequestProcessor::new(
        message_handler,
        task_manager,
        notification_sender,
        agent_info.clone(),
    );

    // Create health checker
    let mut health_checker = HealthChecker::new().with_version("1.0.0".to_string());

    // Register components
    health_checker.register_component("database".to_string(), HealthStatus::Healthy).await;
    health_checker.register_component("cache".to_string(), HealthStatus::Healthy).await;

    // Create OpenAPI builder
    let openapi = OpenApiBuilder::new()
        .with_title("A2A Production Example Server".to_string())
        .with_version("1.0.0".to_string())
        .with_description("Production-ready A2A server with all middleware".to_string())
        .add_server("http://localhost:8080".to_string(), Some("Local development".to_string()))
        .include_health(true)
        .include_spec_endpoint(true);

    // Build server with production features
    let server = HttpServer::new(processor, agent_info, "0.0.0.0:8080".to_string())
        .with_cors(CorsConfig::permissive())
        .with_rate_limit(RateLimitConfig::default())
        .with_validation(ValidationConfig::default())
        .with_compression(CompressionConfig::fast())
        .with_health_checks(health_checker)
        .with_openapi(openapi)
        .with_request_timeout(Duration::from_secs(30))
        .with_shutdown_timeout(Duration::from_secs(10))
        .with_graceful_shutdown(true);

    tracing::info!("Starting production HTTP server on 0.0.0.0:8080");
    tracing::info!("Available endpoints:");
    tracing::info!("  POST /                        - JSON-RPC endpoint");
    tracing::info!("  GET  /.well-known/agent-card.json - Agent card");
    tracing::info!("  GET  /skills                     - List skills");
    tracing::info!("  GET  /health                     - Health check");
    tracing::info!("  GET  /ready                      - Readiness check");
    tracing::info!("  GET  /live                       - Liveness check");
    tracing::info!("  GET  /openapi.json               - OpenAPI spec");
    tracing::info!("");
    tracing::info!("Press Ctrl+C to gracefully shutdown");

    // Start server (blocks until shutdown)
    server.start().await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}
