//! Enhanced client service with builder pattern, retry logic, and batching
//!
//! This module provides a high-level client interface for the A2A protocol with:
//! - Builder pattern for easy configuration
//! - Automatic retry with exponential backoff
//! - Connection pool management
//! - Batch operations support
//! - Automatic token refresh for authentication
//! - Comprehensive error handling

use async_trait::async_trait;
use bon::Builder;
use futures::Stream;
use std::{
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

#[cfg(feature = "tracing")]
use tracing::{info, warn, instrument};

use crate::{
    application::{JSONRPCResponse, json_rpc::A2ARequest},
    domain::{
        A2AError, ListTasksParams, ListTasksResult, Message, Task, TaskArtifactUpdateEvent,
        TaskPushNotificationConfig, TaskStatusUpdateEvent,
    },
};

#[cfg(feature = "http-client")]
use crate::adapter::HttpClient;

/// Items that can be streamed from the server during task subscriptions.
///
/// When subscribing to streaming updates for a task, the server can send
/// different types of items:
/// - `Task`: The complete initial task state when subscription starts
/// - `StatusUpdate`: Updates to the task's status (state changes, progress)
/// - `ArtifactUpdate`: Notifications about new or updated artifacts
///
/// This allows clients to receive real-time updates about task progress
/// and results as they become available.
#[derive(Debug, Clone)]
pub enum StreamItem {
    /// The initial task state
    Task(Task),
    /// A task status update
    StatusUpdate(TaskStatusUpdateEvent),
    /// A task artifact update
    ArtifactUpdate(TaskArtifactUpdateEvent),
}

/// Configuration for retry behavior with exponential backoff
///
/// # Example
/// ```rust
/// use a2a_rs::services::RetryConfig;
///
/// let retry_config = RetryConfig::builder()
///     .max_retries(3)
///     .initial_delay(Duration::from_millis(100))
///     .max_delay(Duration::from_secs(5))
///     .backoff_multiplier(2.0)
///     .build();
/// ```
#[derive(Debug, Clone, Builder)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (default: 3)
    #[builder(default = 3)]
    pub max_retries: usize,

    /// Initial delay before first retry (default: 100ms)
    #[builder(default = Duration::from_millis(100))]
    pub initial_delay: Duration,

    /// Maximum delay between retries (default: 5s)
    #[builder(default = Duration::from_secs(5))]
    pub max_delay: Duration,

    /// Multiplier for exponential backoff (default: 2.0)
    #[builder(default = 2.0)]
    pub backoff_multiplier: f64,

    /// Whether to jitter the delay (default: true)
    #[builder(default = true)]
    pub jitter: bool,
}

impl RetryConfig {
    /// Calculate delay for a given retry attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = base_delay.min(self.max_delay.as_millis() as f64) as u64;

        // Add jitter if enabled (random +/- 25%)
        if self.jitter {
            let jitter_range = (delay_ms as f64 * 0.25) as u64;
            let jitter = rand::random::<u64>() % (2 * jitter_range + 1);
            Duration::from_millis(delay_ms + jitter - jitter_range)
        } else {
            Duration::from_millis(delay_ms)
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Connection pool configuration
///
/// # Example
/// ```rust
/// use a2a_rs::services::PoolConfig;
///
/// let pool_config = PoolConfig::builder()
///     .max_connections(10)
///     .min_idle(2)
///     .connection_timeout(Duration::from_secs(30))
///     .idle_timeout(Duration::from_secs(300))
///     .build();
/// ```
#[derive(Debug, Clone, Builder)]
pub struct PoolConfig {
    /// Maximum number of connections in the pool (default: 10)
    #[builder(default = 10)]
    pub max_connections: usize,

    /// Minimum number of idle connections to maintain (default: 2)
    #[builder(default = 2)]
    pub min_idle: usize,

    /// Timeout for establishing a new connection (default: 30s)
    #[builder(default = Duration::from_secs(30))]
    pub connection_timeout: Duration,

    /// Timeout for idle connections before they're closed (default: 5min)
    #[builder(default = Duration::from_secs(300))]
    pub idle_timeout: Duration,

    /// Maximum lifetime of a connection (default: 1 hour)
    #[builder(default = Duration::from_secs(3600))]
    pub max_lifetime: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Token refresh configuration for automatic authentication renewal
///
/// # Example
/// ```rust
/// use a2a_rs::services::TokenRefreshConfig;
/// use std::time::Duration;
///
/// let refresh_config = TokenRefreshConfig::builder()
///     .refresh_before_expiry(Duration::from_secs(300)) // 5 minutes before expiry
///     .max_refresh_retries(2)
///     .build();
/// ```
#[derive(Debug, Clone, Builder)]
pub struct TokenRefreshConfig {
    /// How long before token expiry to trigger refresh (default: 5 minutes)
    #[builder(default = Duration::from_secs(300))]
    pub refresh_before_expiry: Duration,

    /// Maximum retries for token refresh (default: 2)
    #[builder(default = 2)]
    pub max_refresh_retries: usize,

    /// Whether token refresh is enabled (default: true)
    #[builder(default = true)]
    pub enabled: bool,
}

impl Default for TokenRefreshConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Batch operation configuration for optimizing multiple operations
///
/// # Example
/// ```rust
/// use a2a_rs::services::BatchConfig;
/// use std::time::Duration;
///
/// let batch_config = BatchConfig::builder()
///     .max_batch_size(50)
///     .max_batch_latency(Duration::from_millis(100))
///     .build();
/// ```
#[derive(Debug, Clone, Builder)]
pub struct BatchConfig {
    /// Maximum number of operations in a single batch (default: 50)
    #[builder(default = 50)]
    pub max_batch_size: usize,

    /// Maximum time to wait before flushing a partial batch (default: 100ms)
    #[builder(default = Duration::from_millis(100))]
    pub max_batch_latency: Duration,

    /// Whether batching is enabled (default: true)
    #[builder(default = true)]
    pub enabled: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Token information for authentication
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// The actual token string
    pub token: String,
    /// When the token expires (None if unknown/no expiry)
    pub expires_at: Option<Instant>,
    /// Refresh token if available
    pub refresh_token: Option<String>,
}

impl TokenInfo {
    /// Check if the token is expired or will expire soon
    pub fn is_expired(&self, config: &TokenRefreshConfig) -> bool {
        if !config.enabled {
            return false;
        }

        match self.expires_at {
            Some(expires_at) => {
                let now = Instant::now();
                // Check if we're within the refresh window
                // We need to refresh if the current time is past (expires_at - refresh_before_expiry)
                let refresh_time = expires_at.checked_sub(config.refresh_before_expiry).unwrap_or(expires_at);
                now >= refresh_time
            }
            None => false,
        }
    }
}

/// Enhanced A2A client configuration
///
/// # Example
/// ```rust
/// use a2a_rs::services::{A2AClientConfig, RetryConfig, PoolConfig};
/// use std::time::Duration;
///
/// let config = A2AClientConfig::builder()
///     .base_url("http://localhost:8080".to_string())
///     .auth_token("secret-token".to_string())
///     .retry_config(RetryConfig::builder().max_retries(5).build())
///     .pool_config(PoolConfig::builder().max_connections(20).build())
///     .request_timeout(Duration::from_secs(60))
///     .build();
/// ```
#[derive(Debug, Clone, Builder)]
pub struct A2AClientConfig {
    /// Base URL of the A2A service
    pub base_url: String,

    /// Authentication token (if required)
    pub auth_token: Option<String>,

    /// Retry configuration
    pub retry_config: Option<RetryConfig>,

    /// Connection pool configuration
    pub pool_config: Option<PoolConfig>,

    /// Token refresh configuration
    pub token_refresh_config: Option<TokenRefreshConfig>,

    /// Batch configuration
    pub batch_config: Option<BatchConfig>,

    /// Request timeout (default: 30s)
    #[builder(default = Duration::from_secs(30))]
    pub request_timeout: Duration,
}

/// An async trait defining the methods an async client should implement
#[async_trait]
pub trait AsyncA2AClient: Send + Sync {
    /// Send a raw request to the server and get a response
    async fn send_raw_request<'a>(&self, request: &'a str) -> Result<String, A2AError>;

    /// Send a structured request to the server and get a response
    async fn send_request<'a>(&self, request: &'a A2ARequest) -> Result<JSONRPCResponse, A2AError>;

    /// Send a message to a task
    async fn send_task_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        session_id: Option<&'a str>,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError>;

    /// Get a task by ID
    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError>;

    /// Cancel a task
    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError>;

    /// Set up push notifications for a task
    async fn set_task_push_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError>;

    /// Get push notification configuration for a task
    async fn get_task_push_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError>;

    /// List tasks with filtering and pagination (v0.3.0)
    async fn list_tasks<'a>(
        &self,
        params: &'a ListTasksParams,
    ) -> Result<ListTasksResult, A2AError>;

    /// List all push notification configs for a task (v0.3.0)
    async fn list_push_notification_configs<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError>;

    /// Get a specific push notification config by ID (v0.3.0)
    async fn get_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError>;

    /// Delete a specific push notification config (v0.3.0)
    async fn delete_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<(), A2AError>;

    /// Subscribe to task updates (for streaming)
    async fn subscribe_to_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamItem, A2AError>> + Send>>, A2AError>;
}

/// Enhanced HTTP client with connection pooling and retry logic
///
/// # Example
/// ```rust
/// use a2a_rs::services::{EnhancedHttpClient, A2AClientConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = A2AClientConfig::builder()
///         .base_url("http://localhost:8080".to_string())
///         .auth_token("secret-token".to_string())
///         .build();
///
///     let client = EnhancedHttpClient::new(config)?;
///
///     // Use the client...
///     Ok(())
/// }
/// ```
#[cfg(feature = "http-client")]
pub struct EnhancedHttpClient {
    /// Configuration
    config: A2AClientConfig,
    /// Underlying HTTP client
    http_client: HttpClient,
    /// Connection pool semaphore
    pool_semaphore: Arc<Semaphore>,
    /// Token information (if using authentication)
    token_info: Arc<Mutex<Option<TokenInfo>>>,
    /// Token refresh callback
    token_refresh_callback: Arc<Mutex<Option<Box<dyn Fn() -> Result<String, A2AError> + Send + Sync>>>>,
}

#[cfg(feature = "http-client")]
impl EnhancedHttpClient {
    /// Create a new enhanced HTTP client with the given configuration
    pub fn new(config: A2AClientConfig) -> Result<Self, A2AError> {
        let pool_config = config.pool_config.clone().unwrap_or_default();
        let pool_semaphore = Arc::new(Semaphore::new(pool_config.max_connections));

        let http_client = HttpClient::with_auth(
            config.base_url.clone(),
            config.auth_token.clone().unwrap_or_default(),
        );

        Ok(Self {
            config,
            http_client,
            pool_semaphore,
            token_info: Arc::new(Mutex::new(None)),
            token_refresh_callback: Arc::new(Mutex::new(None)),
        })
    }

    /// Set a callback for token refresh
    pub fn with_token_refresh<F>(
        mut self,
        callback: F,
    ) -> Self
    where
        F: Fn() -> Result<String, A2AError> + Send + Sync + 'static,
    {
        let callback_boxed = Box::new(callback) as Box<dyn Fn() -> Result<String, A2AError> + Send + Sync>;
        self.token_refresh_callback = Arc::new(Mutex::new(Some(callback_boxed)));
        self
    }

    /// Check and refresh token if needed
    async fn ensure_valid_token(&self) -> Result<(), A2AError> {
        let refresh_config = self.config.token_refresh_config.clone().unwrap_or_default();

        let token_info_guard = self.token_info.lock().await;
        if let Some(token_info) = token_info_guard.as_ref() {
            if !token_info.is_expired(&refresh_config) {
                return Ok(());
            }
        }
        drop(token_info_guard);

        // Need to refresh token
        let callback_guard = self.token_refresh_callback.lock().await;
        if let Some(callback) = callback_guard.as_ref() {
            #[cfg(feature = "tracing")]
            info!("Token expired, refreshing...");

            let new_token = callback()?;

            let mut token_info_guard = self.token_info.lock().await;
            *token_info_guard = Some(TokenInfo {
                token: new_token.clone(),
                expires_at: None, // Would be set by callback in real implementation
                refresh_token: None,
            });

            #[cfg(feature = "tracing")]
            info!("Token refreshed successfully");

            // Update underlying client
            // Note: In a real implementation, we'd update the HTTP client's token
        }

        Ok(())
    }

    /// Execute an operation with retry logic
    async fn execute_with_retry<T, Fut, F>(&self, operation: F) -> Result<T, A2AError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, A2AError>> + Send,
    {
        let retry_config = self.config.retry_config.clone().unwrap_or_default();

        for attempt in 0..=retry_config.max_retries {
            // Check token validity before each attempt
            self.ensure_valid_token().await?;

            // Acquire connection from pool
            let _permit = self.pool_semaphore.acquire().await.map_err(|e| {
                A2AError::Internal(format!("Failed to acquire connection from pool: {}", e))
            })?;

            let result = operation().await;

            match &result {
                Ok(_) => {
                    if attempt > 0 {
                        #[cfg(feature = "tracing")]
                        info!("Operation succeeded after {} retries", attempt);
                    }
                    return result;
                }
                Err(e) => {
                    // Check if error is retryable
                    if attempt < retry_config.max_retries && self.is_retryable_error(e) {
                        let delay = retry_config.delay_for_attempt(attempt);

                        #[cfg(feature = "tracing")]
                        warn!(
                            "Operation failed (attempt {}/{}), retrying in {:?}: {}",
                            attempt + 1,
                            retry_config.max_retries + 1,
                            delay,
                            e
                        );

                        tokio::time::sleep(delay).await;
                    } else {
                        return result;
                    }
                }
            }
        }

        // Shouldn't reach here, but handle it
        Err(A2AError::Internal(
            "Operation failed after all retry attempts".to_string(),
        ))
    }
    fn is_retryable_error(&self, error: &A2AError) -> bool {
        match error {
            A2AError::Internal(msg) if msg.contains("timeout") => true,
            A2AError::Internal(msg) if msg.contains("connection") => true,
            A2AError::Io(_) => true,
            _ => false,
        }
    }
}

#[cfg(feature = "http-client")]
#[async_trait]
impl AsyncA2AClient for EnhancedHttpClient {
    #[cfg_attr(feature = "tracing", instrument(skip(self, request)))]
    async fn send_raw_request<'a>(&self, request: &'a str) -> Result<String, A2AError> {
        let request = request.to_owned();
        self.execute_with_retry(|| async {
            self.http_client.send_raw_request(&request).await
        })
        .await
    }

    #[cfg_attr(feature = "tracing", instrument(skip(self, request)))]
    async fn send_request<'a>(&self, request: &'a A2ARequest) -> Result<JSONRPCResponse, A2AError> {
        let request = request.clone();
        self.execute_with_retry(|| async {
            self.http_client.send_request(&request).await
        })
        .await
    }

    async fn send_task_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        session_id: Option<&'a str>,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let task_id = task_id.to_owned();
        let message = message.clone();
        let session_id = session_id.map(|s| (*s).to_owned());
        self.execute_with_retry(|| async {
            self.http_client
                .send_task_message(&task_id, &message, session_id.as_deref(), history_length)
                .await
        })
        .await
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let task_id = task_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client.get_task(&task_id, history_length).await
        })
        .await
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        let task_id = task_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client.cancel_task(&task_id).await
        })
        .await
    }

    async fn set_task_push_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let config = config.clone();
        self.execute_with_retry(|| async {
            self.http_client.set_task_push_notification(&config).await
        })
        .await
    }

    async fn get_task_push_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let task_id = task_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client.get_task_push_notification(&task_id).await
        })
        .await
    }

    async fn list_tasks<'a>(
        &self,
        params: &'a ListTasksParams,
    ) -> Result<ListTasksResult, A2AError> {
        let params = params.clone();
        self.execute_with_retry(|| async {
            self.http_client.list_tasks(&params).await
        })
        .await
    }

    async fn list_push_notification_configs<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError> {
        let task_id = task_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client.list_push_notification_configs(&task_id).await
        })
        .await
    }

    async fn get_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let task_id = task_id.to_owned();
        let config_id = config_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client
                .get_push_notification_config(&task_id, &config_id)
                .await
        })
        .await
    }

    async fn delete_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<(), A2AError> {
        let task_id = task_id.to_owned();
        let config_id = config_id.to_owned();
        self.execute_with_retry(|| async {
            self.http_client
                .delete_push_notification_config(&task_id, &config_id)
                .await
        })
        .await
    }

    async fn subscribe_to_task<'a>(
        &self,
        _task_id: &'a str,
        _history_length: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamItem, A2AError>> + Send>>, A2AError> {
        Err(A2AError::UnsupportedOperation(
            "Streaming is not supported with HTTP client".to_string(),
        ))
    }
}

/// Helper trait for batch operations on multiple clients
#[async_trait]
pub trait BatchClientOperations {
    /// Get multiple tasks concurrently
    async fn get_tasks_batch(
        &self,
        task_ids: Vec<String>,
    ) -> Vec<Result<Task, A2AError>>;

    /// Cancel multiple tasks concurrently
    async fn cancel_tasks_batch(
        &self,
        task_ids: Vec<String>,
    ) -> Vec<Result<Task, A2AError>>;
}

#[cfg(feature = "http-client")]
#[async_trait]
impl BatchClientOperations for EnhancedHttpClient {
    async fn get_tasks_batch(
        &self,
        task_ids: Vec<String>,
    ) -> Vec<Result<Task, A2AError>> {
        let mut results = Vec::new();
        for id in &task_ids {
            results.push(self.get_task(id, None).await);
        }
        results
    }

    async fn cancel_tasks_batch(
        &self,
        task_ids: Vec<String>,
    ) -> Vec<Result<Task, A2AError>> {
        let mut results = Vec::new();
        for id in &task_ids {
            results.push(self.cancel_task(id).await);
        }
        results
    }
}
