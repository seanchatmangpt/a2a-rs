//! Production-ready WebSocket client with automatic reconnection
//!
//! This module provides a robust WebSocket client implementation with:
//! - Automatic reconnection with exponential backoff
//! - Heartbeat/ping mechanism to detect stale connections
//! - Message queue during disconnection
//! - Connection state machine
//! - Graceful network failure handling

use async_trait::async_trait;
use futures::{
    stream::{Stream, StreamExt},
    SinkExt,
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, RwLock},
    time::{interval, sleep, timeout, Instant},
};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use url::Url;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace, warn};

use crate::{
    adapter::error::WebSocketClientError,
    application::{
        json_rpc::{self, A2ARequest, SendTaskRequest, TaskResubscriptionRequest},
        JSONRPCResponse,
    },
    domain::{
        A2AError, Message, Task, TaskArtifactUpdateEvent, TaskIdParams, TaskPushNotificationConfig,
        TaskQueryParams, TaskSendParams, TaskStatusUpdateEvent,
    },
    services::client::{AsyncA2AClient, StreamItem},
};

type WebSocketTx = Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>;

/// Connection state for the WebSocket client
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Connected and operational
    Connected,
    /// Reconnecting after a failure
    Reconnecting,
    /// Connection permanently closed
    Closed,
}

/// Configuration for automatic reconnection
#[derive(Debug, Clone)]
pub struct ReconnectionConfig {
    /// Initial backoff delay in milliseconds
    pub initial_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds
    pub max_backoff_ms: u64,
    /// Backoff multiplier (e.g., 2.0 for exponential doubling)
    pub backoff_multiplier: f64,
    /// Maximum number of reconnection attempts (None = infinite)
    pub max_attempts: Option<u32>,
    /// Jitter factor (0.0 to 1.0) to add randomness to backoff
    pub jitter_factor: f64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            initial_backoff_ms: 1000,      // 1 second
            max_backoff_ms: 60000,         // 60 seconds
            backoff_multiplier: 2.0,       // exponential doubling
            max_attempts: None,            // infinite retries
            jitter_factor: 0.1,            // 10% jitter
        }
    }
}

/// Configuration for heartbeat/ping mechanism
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between ping messages in seconds
    pub ping_interval_secs: u64,
    /// Timeout waiting for pong response in seconds
    pub pong_timeout_secs: u64,
    /// Enable heartbeat mechanism
    pub enabled: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            ping_interval_secs: 30,
            pong_timeout_secs: 10,
            enabled: true,
        }
    }
}

/// Message queue entry
#[derive(Debug, Clone)]
struct QueuedMessage {
    message: WsMessage,
    enqueued_at: Instant,
}

/// Internal state for the WebSocket client
struct ClientState {
    /// Current connection state
    state: ConnectionState,
    /// Active WebSocket connection
    connection: Option<WebSocketTx>,
    /// Queue of messages to send when reconnected
    message_queue: VecDeque<QueuedMessage>,
    /// Maximum queue size (older messages dropped when exceeded)
    max_queue_size: usize,
    /// Current reconnection attempt count
    reconnection_attempts: u32,
    /// Current backoff delay in milliseconds
    current_backoff_ms: u64,
    /// Last time a pong was received
    last_pong_received: Option<Instant>,
    /// Last time a ping was sent
    last_ping_sent: Option<Instant>,
}

/// Production-ready WebSocket client with automatic reconnection
pub struct RobustWebSocketClient {
    /// Base WebSocket URL of the A2A API
    base_url: String,
    /// Authorization token, if any
    auth_token: Option<String>,
    /// Request timeout in seconds
    timeout: u64,
    /// Reconnection configuration
    reconnection_config: ReconnectionConfig,
    /// Heartbeat configuration
    heartbeat_config: HeartbeatConfig,
    /// Internal state (protected by RwLock for concurrent access)
    state: Arc<RwLock<ClientState>>,
}

impl RobustWebSocketClient {
    /// Create a new robust WebSocket client with the given base URL
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            auth_token: None,
            timeout: 30,
            reconnection_config: ReconnectionConfig::default(),
            heartbeat_config: HeartbeatConfig::default(),
            state: Arc::new(RwLock::new(ClientState {
                state: ConnectionState::Disconnected,
                connection: None,
                message_queue: VecDeque::new(),
                max_queue_size: 1000,
                reconnection_attempts: 0,
                current_backoff_ms: ReconnectionConfig::default().initial_backoff_ms,
                last_pong_received: None,
                last_ping_sent: None,
            })),
        }
    }

    /// Create a new WebSocket client with authentication
    pub fn with_auth(base_url: String, auth_token: String) -> Self {
        let mut client = Self::new(base_url);
        client.auth_token = Some(auth_token);
        client
    }

    /// Set the timeout for operations
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    /// Configure reconnection behavior
    pub fn with_reconnection_config(mut self, config: ReconnectionConfig) -> Self {
        self.reconnection_config = config;
        self
    }

    /// Configure heartbeat behavior
    pub fn with_heartbeat_config(mut self, config: HeartbeatConfig) -> Self {
        self.heartbeat_config = config;
        self
    }

    /// Set maximum message queue size
    pub async fn set_max_queue_size(&self, size: usize) {
        let mut state = self.state.write().await;
        state.max_queue_size = size;
    }

    /// Get current connection state
    pub async fn connection_state(&self) -> ConnectionState {
        let state = self.state.read().await;
        state.state
    }

    /// Get number of queued messages
    pub async fn queued_message_count(&self) -> usize {
        let state = self.state.read().await;
        state.message_queue.len()
    }

    /// Calculate backoff delay with jitter
    fn calculate_backoff(&self, current_backoff_ms: u64) -> u64 {
        let jitter_range = (current_backoff_ms as f64 * self.reconnection_config.jitter_factor) as u64;
        let jitter = if jitter_range > 0 {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hash, Hasher};

            let mut hasher = RandomState::new().build_hasher();
            Instant::now().hash(&mut hasher);
            hasher.finish() % jitter_range
        } else {
            0
        };

        current_backoff_ms.saturating_add(jitter)
    }

    /// Connect to the WebSocket server with reconnection logic
    async fn connect_with_retry(&self) -> Result<(), A2AError> {
        loop {
            {
                let state = self.state.read().await;
                if state.state == ConnectionState::Closed {
                    return Err(WebSocketClientError::Connection(
                        "Client is permanently closed".to_string(),
                    )
                    .into());
                }

                // Check if max attempts reached
                if let Some(max_attempts) = self.reconnection_config.max_attempts {
                    if state.reconnection_attempts >= max_attempts {
                        #[cfg(feature = "tracing")]
                        error!("Max reconnection attempts ({}) reached", max_attempts);
                        return Err(WebSocketClientError::Connection(
                            "Max reconnection attempts reached".to_string(),
                        )
                        .into());
                    }
                }
            }

            // Update state to connecting/reconnecting
            {
                let mut state = self.state.write().await;
                state.state = if state.reconnection_attempts == 0 {
                    ConnectionState::Connecting
                } else {
                    ConnectionState::Reconnecting
                };
            }

            // Attempt connection
            match self.attempt_connection().await {
                Ok(ws_stream) => {
                    let mut state = self.state.write().await;
                    state.connection = Some(Arc::new(Mutex::new(ws_stream)));
                    state.state = ConnectionState::Connected;
                    state.reconnection_attempts = 0;
                    state.current_backoff_ms = self.reconnection_config.initial_backoff_ms;
                    state.last_pong_received = Some(Instant::now());

                    #[cfg(feature = "tracing")]
                    info!("WebSocket connection established");

                    // Flush queued messages
                    self.flush_message_queue().await?;

                    return Ok(());
                }
                Err(e) => {
                    let mut state = self.state.write().await;
                    state.reconnection_attempts += 1;

                    let backoff = self.calculate_backoff(state.current_backoff_ms);

                    #[cfg(feature = "tracing")]
                    warn!(
                        "Connection attempt {} failed: {}. Retrying in {}ms",
                        state.reconnection_attempts, e, backoff
                    );

                    // Update backoff for next attempt
                    state.current_backoff_ms = (state.current_backoff_ms as f64
                        * self.reconnection_config.backoff_multiplier) as u64;
                    state.current_backoff_ms = state
                        .current_backoff_ms
                        .min(self.reconnection_config.max_backoff_ms);

                    drop(state);

                    // Sleep before retry
                    sleep(Duration::from_millis(backoff)).await;
                }
            }
        }
    }

    /// Attempt a single connection
    async fn attempt_connection(&self) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, A2AError> {
        let mut url = Url::parse(&self.base_url)
            .map_err(|e| WebSocketClientError::Connection(format!("Invalid URL: {}", e)))?;

        // Add auth token to URL if present
        if let Some(token) = &self.auth_token {
            url.query_pairs_mut().append_pair("token", token);
        }

        let (ws_stream, _) = connect_async(url).await.map_err(|e| {
            WebSocketClientError::Connection(format!("WebSocket connection error: {}", e))
        })?;

        Ok(ws_stream)
    }

    /// Flush queued messages after reconnection
    async fn flush_message_queue(&self) -> Result<(), A2AError> {
        let mut state = self.state.write().await;

        if state.message_queue.is_empty() {
            return Ok(());
        }

        let conn = state.connection.as_ref().ok_or_else(|| {
            WebSocketClientError::Connection("No connection available".to_string())
        })?;

        #[cfg(feature = "tracing")]
        info!("Flushing {} queued messages", state.message_queue.len());

        let mut conn_guard = conn.lock().await;

        while let Some(queued) = state.message_queue.pop_front() {
            #[cfg(feature = "tracing")]
            trace!("Sending queued message (age: {:?})", queued.enqueued_at.elapsed());

            if let Err(e) = conn_guard.send(queued.message).await {
                #[cfg(feature = "tracing")]
                error!("Failed to send queued message: {}", e);

                // Put message back in queue
                state.message_queue.push_front(queued);
                return Err(WebSocketClientError::Message(format!("Send error: {}", e)).into());
            }
        }

        Ok(())
    }

    /// Queue a message for sending
    async fn queue_message(&self, message: WsMessage) {
        let mut state = self.state.write().await;

        // Enforce max queue size
        while state.message_queue.len() >= state.max_queue_size {
            let dropped = state.message_queue.pop_front();
            #[cfg(feature = "tracing")]
            if let Some(msg) = dropped {
                warn!("Dropped queued message due to queue size limit (age: {:?})",
                      msg.enqueued_at.elapsed());
            }
        }

        state.message_queue.push_back(QueuedMessage {
            message,
            enqueued_at: Instant::now(),
        });

        #[cfg(feature = "tracing")]
        debug!("Message queued (queue size: {})", state.message_queue.len());
    }

    /// Send a message with automatic reconnection
    async fn send_ws_message_robust(&self, message: WsMessage) -> Result<WsMessage, A2AError> {
        // Ensure we're connected
        self.ensure_connected().await?;

        // Try to send the message
        match self.try_send_message(message.clone()).await {
            Ok(response) => Ok(response),
            Err(_) => {
                // Connection failed, queue message and reconnect
                #[cfg(feature = "tracing")]
                warn!("Send failed, queueing message and reconnecting");

                self.queue_message(message).await;
                self.handle_disconnection().await?;

                Err(WebSocketClientError::Connection(
                    "Message queued for retry after reconnection".to_string(),
                )
                .into())
            }
        }
    }

    /// Ensure the client is connected
    async fn ensure_connected(&self) -> Result<(), A2AError> {
        let state = self.state.read().await;

        if state.state == ConnectionState::Connected && state.connection.is_some() {
            Ok(())
        } else {
            drop(state);
            self.connect_with_retry().await
        }
    }

    /// Try to send a message (without retry logic)
    async fn try_send_message(&self, message: WsMessage) -> Result<WsMessage, A2AError> {
        let state = self.state.read().await;

        let conn = state.connection.as_ref().ok_or_else(|| {
            WebSocketClientError::Connection("No connection".to_string())
        })?;

        let conn_clone = conn.clone();
        drop(state);

        // Send the message
        {
            let mut guard = conn_clone.lock().await;
            guard.send(message).await.map_err(|e| {
                WebSocketClientError::Message(format!("Send error: {}", e))
            })?;
        }

        // Receive the response
        let response = {
            let mut guard = conn_clone.lock().await;

            let timeout_duration = Duration::from_secs(self.timeout);
            let result = timeout(timeout_duration, guard.next())
                .await
                .map_err(|_| WebSocketClientError::Timeout)?;

            match result {
                Some(Ok(msg)) => msg,
                Some(Err(e)) => {
                    return Err(WebSocketClientError::Message(format!("WebSocket error: {}", e)).into());
                }
                None => return Err(WebSocketClientError::Closed.into()),
            }
        };

        Ok(response)
    }

    /// Handle disconnection and trigger reconnection
    async fn handle_disconnection(&self) -> Result<(), A2AError> {
        {
            let mut state = self.state.write().await;
            state.connection = None;
            state.state = ConnectionState::Disconnected;
        }

        #[cfg(feature = "tracing")]
        warn!("Connection lost, initiating reconnection");

        self.connect_with_retry().await
    }

    /// Start heartbeat monitoring task
    pub async fn start_heartbeat(&self) -> Result<(), A2AError> {
        if !self.heartbeat_config.enabled {
            return Ok(());
        }

        let state_clone = Arc::clone(&self.state);
        let ping_interval = Duration::from_secs(self.heartbeat_config.ping_interval_secs);
        let pong_timeout = Duration::from_secs(self.heartbeat_config.pong_timeout_secs);

        tokio::spawn(async move {
            let mut ticker = interval(ping_interval);

            loop {
                ticker.tick().await;

                let state = state_clone.read().await;

                if state.state != ConnectionState::Connected {
                    drop(state);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let conn = match &state.connection {
                    Some(c) => c.clone(),
                    None => {
                        drop(state);
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                // Check if pong timed out
                if let Some(last_pong) = state.last_pong_received {
                    if last_pong.elapsed() > pong_timeout + ping_interval {
                        #[cfg(feature = "tracing")]
                        error!("Heartbeat timeout - no pong received");

                        drop(state);
                        let mut state_mut = state_clone.write().await;
                        state_mut.connection = None;
                        state_mut.state = ConnectionState::Disconnected;
                        continue;
                    }
                }

                drop(state);

                // Send ping
                let mut guard = conn.lock().await;
                if let Err(e) = guard.send(WsMessage::Ping(vec![])).await {
                    #[cfg(feature = "tracing")]
                    error!("Failed to send ping: {}", e);

                    drop(guard);
                    let mut state_mut = state_clone.write().await;
                    state_mut.connection = None;
                    state_mut.state = ConnectionState::Disconnected;
                } else {
                    drop(guard);
                    let mut state_mut = state_clone.write().await;
                    state_mut.last_ping_sent = Some(Instant::now());

                    #[cfg(feature = "tracing")]
                    trace!("Heartbeat ping sent");
                }
            }
        });

        Ok(())
    }

    /// Close the connection permanently
    pub async fn close(&self) -> Result<(), A2AError> {
        let mut state = self.state.write().await;

        if let Some(conn) = state.connection.take() {
            let mut guard = conn.lock().await;
            let _ = guard.close(None).await;
        }

        state.state = ConnectionState::Closed;
        state.message_queue.clear();

        #[cfg(feature = "tracing")]
        info!("WebSocket client closed");

        Ok(())
    }
}

#[async_trait]
impl AsyncA2AClient for RobustWebSocketClient {
    async fn send_raw_request<'a>(&self, request: &'a str) -> Result<String, A2AError> {
        let response = self
            .send_ws_message_robust(WsMessage::Text(request.to_string()))
            .await?;

        match response {
            WsMessage::Text(text) => Ok(text),
            _ => Err(A2AError::Internal(
                "Unexpected WebSocket message type".to_string(),
            )),
        }
    }

    async fn send_request<'a>(&self, request: &'a A2ARequest) -> Result<JSONRPCResponse, A2AError> {
        let json = json_rpc::serialize_request(request)?;
        let response_text = self.send_raw_request(&json).await?;
        let response: JSONRPCResponse = serde_json::from_str(&response_text)?;
        Ok(response)
    }

    async fn send_task_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        session_id: Option<&'a str>,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let params = TaskSendParams {
            id: task_id.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            message: message.clone(),
            push_notification: None,
            history_length,
            metadata: None,
        };

        let request = SendTaskRequest::new(params);
        let response = self.send_request(&A2ARequest::SendTask(request)).await?;

        match response.result {
            Some(value) => {
                let task: Task = serde_json::from_value(value)?;
                Ok(task)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length,
            metadata: None,
        };

        let request = json_rpc::GetTaskRequest::new(params);
        let response = self.send_request(&A2ARequest::GetTask(request)).await?;

        let Some(value) = response.result else {
            if let Some(error) = response.error {
                return Err(A2AError::JsonRpc {
                    code: error.code,
                    message: error.message,
                    data: error.data,
                });
            }
            return Err(A2AError::Internal("Empty response".to_string()));
        };

        let task: Task = serde_json::from_value(value)?;
        Ok(task)
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };

        let request = json_rpc::CancelTaskRequest::new(params);
        let response = self.send_request(&A2ARequest::CancelTask(request)).await?;

        let Some(value) = response.result else {
            if let Some(error) = response.error {
                return Err(A2AError::JsonRpc {
                    code: error.code,
                    message: error.message,
                    data: error.data,
                });
            }
            return Err(A2AError::Internal("Empty response".to_string()));
        };

        let task: Task = serde_json::from_value(value)?;
        Ok(task)
    }

    async fn set_task_push_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let request = json_rpc::SetTaskPushNotificationRequest::new(config.clone());
        let response = self
            .send_request(&A2ARequest::SetTaskPushNotification(request))
            .await?;

        let Some(value) = response.result else {
            if let Some(error) = response.error {
                return Err(A2AError::JsonRpc {
                    code: error.code,
                    message: error.message,
                    data: error.data,
                });
            }
            return Err(A2AError::Internal("Empty response".to_string()));
        };

        let config: TaskPushNotificationConfig = serde_json::from_value(value)?;
        Ok(config)
    }

    async fn get_task_push_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };

        let request = json_rpc::GetTaskPushNotificationRequest::new(params);
        let response = self
            .send_request(&A2ARequest::GetTaskPushNotification(request))
            .await?;

        match response.result {
            Some(value) => {
                let config: TaskPushNotificationConfig = serde_json::from_value(value)?;
                Ok(config)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn list_tasks<'a>(
        &self,
        params: &'a crate::domain::ListTasksParams,
    ) -> Result<crate::domain::ListTasksResult, A2AError> {
        let request = json_rpc::ListTasksRequest::new(Some(params.clone()));
        let response = self.send_request(&A2ARequest::ListTasks(request)).await?;

        match response.result {
            Some(value) => {
                let result: crate::domain::ListTasksResult = serde_json::from_value(value)?;
                Ok(result)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn list_push_notification_configs<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<Vec<crate::domain::TaskPushNotificationConfig>, A2AError> {
        use crate::domain::ListTaskPushNotificationConfigParams;

        let request = json_rpc::ListTaskPushNotificationConfigRequest::new(
            ListTaskPushNotificationConfigParams {
                id: task_id.to_string(),
                metadata: None,
            },
        );
        let response = self
            .send_request(&A2ARequest::ListTaskPushNotificationConfigs(request))
            .await?;

        match response.result {
            Some(value) => {
                let configs: Vec<crate::domain::TaskPushNotificationConfig> =
                    serde_json::from_value(value)?;
                Ok(configs)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn get_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<crate::domain::TaskPushNotificationConfig, A2AError> {
        use crate::domain::GetTaskPushNotificationConfigParams;

        let request = json_rpc::GetTaskPushNotificationConfigRequest::new(
            GetTaskPushNotificationConfigParams {
                id: task_id.to_string(),
                push_notification_config_id: Some(config_id.to_string()),
                metadata: None,
            },
        );
        let response = self
            .send_request(&A2ARequest::GetTaskPushNotificationConfig(request))
            .await?;

        match response.result {
            Some(value) => {
                let config: crate::domain::TaskPushNotificationConfig =
                    serde_json::from_value(value)?;
                Ok(config)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn delete_push_notification_config<'a>(
        &self,
        task_id: &'a str,
        config_id: &'a str,
    ) -> Result<(), A2AError> {
        use crate::domain::DeleteTaskPushNotificationConfigParams;

        let request = json_rpc::DeleteTaskPushNotificationConfigRequest::new(
            DeleteTaskPushNotificationConfigParams {
                id: task_id.to_string(),
                push_notification_config_id: config_id.to_string(),
                metadata: None,
            },
        );
        let response = self
            .send_request(&A2ARequest::DeleteTaskPushNotificationConfig(request))
            .await?;

        if let Some(error) = response.error {
            Err(A2AError::JsonRpc {
                code: error.code,
                message: error.message,
                data: error.data,
            })
        } else {
            Ok(())
        }
    }

    async fn subscribe_to_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamItem, A2AError>> + Send>>, A2AError> {
        // Ensure we're connected
        self.ensure_connected().await?;

        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length,
            metadata: None,
        };

        let request = TaskResubscriptionRequest::new(params);
        let json = json_rpc::serialize_request(&A2ARequest::TaskResubscription(request))?;

        // Get the connection
        let state = self.state.read().await;
        let connection = state
            .connection
            .as_ref()
            .ok_or_else(|| WebSocketClientError::Connection("No connection".to_string()))?
            .clone();
        drop(state);

        // Send the request
        {
            let mut guard = connection.lock().await;
            guard
                .send(WsMessage::Text(json))
                .await
                .map_err(|e| WebSocketClientError::Message(format!("Send error: {}", e)))?;
        }

        // Create a stream that will process incoming messages
        let stream = futures::stream::unfold(connection, move |conn| {
            Box::pin(async move {
                loop {
                    let message_result = {
                        let mut guard = conn.lock().await;
                        guard.next().await
                    };

                    let message = match message_result {
                        Some(Ok(msg)) => msg,
                        Some(Err(e)) => {
                            return Some((
                                Err(WebSocketClientError::Message(format!(
                                    "WebSocket error: {}",
                                    e
                                ))
                                .into()),
                                conn,
                            ));
                        }
                        None => {
                            return Some((Err(WebSocketClientError::Closed.into()), conn));
                        }
                    };

                    match message {
                        WsMessage::Text(text) => {
                            #[cfg(feature = "tracing")]
                            trace!("Received WebSocket message: {}", text);

                            let response: Value = match serde_json::from_str(&text) {
                                Ok(value) => value,
                                Err(e) => {
                                    #[cfg(feature = "tracing")]
                                    debug!("JSON parse error: {}", e);
                                    return Some((Err(A2AError::JsonParse(e)), conn));
                                }
                            };

                            if let Some(error) = response.get("error")
                                && error.is_object()
                            {
                                let response_clone = response.clone();
                                let error: JSONRPCResponse =
                                    match serde_json::from_value(response_clone) {
                                        Ok(resp) => resp,
                                        Err(e) => {
                                            return Some((Err(A2AError::JsonParse(e)), conn));
                                        }
                                    };

                                if let Some(err) = error.error {
                                    return Some((
                                        Err(A2AError::JsonRpc {
                                            code: err.code,
                                            message: err.message,
                                            data: err.data,
                                        }),
                                        conn,
                                    ));
                                }
                            }

                            if response.get("jsonrpc").is_some() && response.get("result").is_some()
                            {
                                let result = response.get("result").cloned().unwrap_or(Value::Null);

                                if result.is_null() {
                                    #[cfg(feature = "tracing")]
                                    debug!("Task doesn't exist yet, waiting for next message");
                                    continue;
                                }

                                if let Ok(task) = serde_json::from_value::<Task>(result.clone()) {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as Task");
                                    return Some((Ok(StreamItem::Task(task)), conn));
                                }

                                if let Ok(status_update) =
                                    serde_json::from_value::<TaskStatusUpdateEvent>(result.clone())
                                {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as StatusUpdate");
                                    return Some((
                                        Ok(StreamItem::StatusUpdate(status_update)),
                                        conn,
                                    ));
                                }

                                if let Ok(artifact_update) =
                                    serde_json::from_value::<TaskArtifactUpdateEvent>(result)
                                {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as ArtifactUpdate");
                                    return Some((
                                        Ok(StreamItem::ArtifactUpdate(artifact_update)),
                                        conn,
                                    ));
                                }
                            }

                            #[cfg(feature = "tracing")]
                            debug!("Failed to parse streaming response");
                            return Some((
                                Err(WebSocketClientError::Protocol(
                                    "Failed to parse streaming response".to_string(),
                                )
                                .into()),
                                conn,
                            ));
                        }
                        WsMessage::Pong(_) => {
                            #[cfg(feature = "tracing")]
                            trace!("Received pong");
                            continue;
                        }
                        _ => {
                            return Some((
                                Err(WebSocketClientError::Protocol(
                                    "Unexpected WebSocket message type".to_string(),
                                )
                                .into()),
                                conn,
                            ));
                        }
                    }
                }
            })
        });

        Ok(Box::pin(stream))
    }
}

impl Clone for RobustWebSocketClient {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            auth_token: self.auth_token.clone(),
            timeout: self.timeout,
            reconnection_config: self.reconnection_config.clone(),
            heartbeat_config: self.heartbeat_config.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_reconnection_config() {
        let config = ReconnectionConfig::default();
        assert_eq!(config.initial_backoff_ms, 1000);
        assert_eq!(config.max_backoff_ms, 60000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.max_attempts.is_none());
    }

    #[test]
    fn test_default_heartbeat_config() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.ping_interval_secs, 30);
        assert_eq!(config.pong_timeout_secs, 10);
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_connection_state() {
        let client = RobustWebSocketClient::new("ws://localhost:8080".to_string());
        assert_eq!(client.connection_state().await, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_queue_size() {
        let client = RobustWebSocketClient::new("ws://localhost:8080".to_string());
        assert_eq!(client.queued_message_count().await, 0);

        client.set_max_queue_size(500).await;
        let state = client.state.read().await;
        assert_eq!(state.max_queue_size, 500);
    }
}
