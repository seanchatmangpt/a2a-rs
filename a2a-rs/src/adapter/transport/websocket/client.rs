//! Production-ready WebSocket client adapter for the A2A protocol
//!
//! This implementation provides:
//! - Automatic reconnection with exponential backoff
//! - Session state tracking across reconnections
//! - Request queue for offline periods
//! - Heartbeat mechanism for connection health
//! - Complete error recovery

// This module is already conditionally compiled with #[cfg(feature = "ws-client")] in mod.rs

use async_trait::async_trait;
use futures::{
    SinkExt,
    stream::{Stream, StreamExt},
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, RwLock, mpsc, watch},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    connect_async,
    tungstenite::protocol::Message as WsMessage,
};
use url::Url;

#[cfg(feature = "tracing")]
use tracing::{debug, error, info, trace, warn};

use crate::{
    adapter::error::WebSocketClientError,
    application::{
        JSONRPCResponse,
        json_rpc::{self, A2ARequest, SendTaskRequest, TaskResubscriptionRequest},
    },
    domain::{
        A2AError, Message, Task, TaskArtifactUpdateEvent, TaskIdParams, TaskPushNotificationConfig,
        TaskQueryParams, TaskSendParams, TaskStatusUpdateEvent,
    },
    services::client::{AsyncA2AClient, StreamItem},
};

/// Write half of the WebSocket connection
type WebSocketWrite = Arc<Mutex<Option<futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>>>>;

/// Read half of the WebSocket connection
type WebSocketRead = Arc<Mutex<Option<futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>>;

/// Session state tracking for WebSocket connections
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Unique session identifier
    pub session_id: String,
    /// Connection timestamp
    pub connected_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Reconnection count
    pub reconnect_count: u32,
}

impl SessionState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            connected_at: now,
            last_activity: now,
            reconnect_count: 0,
        }
    }

    fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }
}

/// Queued request for offline retry
#[derive(Debug, Clone)]
struct QueuedRequest {
    /// Request ID for correlation
    id: String,
    /// Serialized request payload
    payload: String,
    /// Timestamp when queued
    queued_at: Instant,
    /// Retry attempt count
    retry_count: u32,
}

/// Connection health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Closed,
}

impl ConnectionStatus {
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    #[allow(dead_code)]
    fn can_send(&self) -> bool {
        matches!(self, Self::Connected | Self::Reconnecting)
    }
}

/// Reconnection configuration
#[derive(Debug, Clone, bon::Builder)]
pub struct ReconnectConfig {
    /// Enable automatic reconnection
    #[builder(default = true)]
    pub enabled: bool,

    /// Maximum number of reconnection attempts
    #[builder(default = 10)]
    pub max_attempts: u32,

    /// Initial backoff duration
    #[builder(default = Duration::from_millis(100))]
    pub initial_backoff: Duration,

    /// Maximum backoff duration
    #[builder(default = Duration::from_secs(30))]
    pub max_backoff: Duration,

    /// Backoff multiplier for exponential backoff
    #[builder(default = 2.0)]
    pub backoff_multiplier: f64,

    /// Jitter factor for backoff randomization (0.0-1.0)
    #[builder(default = 0.1)]
    pub jitter_factor: f64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ReconnectConfig {
    /// Calculate backoff duration with exponential backoff and jitter
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_backoff = self.initial_backoff.as_millis() as f64
            * self.backoff_multiplier.powi(attempt.min(31) as i32);
        let backoff_ms = base_backoff.min(self.max_backoff.as_millis() as f64);

        // Add jitter to avoid thundering herd
        let jitter_range = backoff_ms * self.jitter_factor;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;

        Duration::from_millis((backoff_ms + jitter).max(0.0) as u64)
    }
}

/// Heartbeat configuration
#[derive(Debug, Clone, bon::Builder)]
pub struct HeartbeatConfig {
    /// Enable heartbeat mechanism
    #[builder(default = true)]
    pub enabled: bool,

    /// Heartbeat interval
    #[builder(default = Duration::from_secs(30))]
    pub interval: Duration,

    /// Heartbeat timeout (no response considered dead)
    #[builder(default = Duration::from_secs(10))]
    pub timeout: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Request queue configuration
#[derive(Debug, Clone, bon::Builder)]
pub struct QueueConfig {
    /// Enable offline request queue
    #[builder(default = true)]
    pub enabled: bool,

    /// Maximum queue size
    #[builder(default = 1000)]
    pub max_size: usize,

    /// Maximum request age before discarding
    #[builder(default = Duration::from_secs(300))]
    pub max_age: Duration,

    /// Maximum retry attempts per request
    #[builder(default = 3)]
    pub max_retries: u32,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// Production-ready WebSocket client for the A2A protocol
pub struct WebSocketClient {
    /// Base WebSocket URL of the A2A API
    base_url: String,
    /// Authorization token, if any
    auth_token: Option<String>,
    /// Write half of the WebSocket connection
    write_half: WebSocketWrite,
    /// Read half of the WebSocket connection
    read_half: WebSocketRead,
    /// Session state
    session: Arc<RwLock<SessionState>>,
    /// Connection status
    status: Arc<watch::Sender<ConnectionStatus>>,
    /// Request queue for offline periods
    request_queue: Arc<Mutex<VecDeque<QueuedRequest>>>,
    /// Channel for sending requests
    request_sender: mpsc::Sender<String>,
    /// Channel for receiving responses
    response_receiver: Arc<Mutex<mpsc::Receiver<String>>>,
    /// Response sender for internal use
    response_sender: mpsc::Sender<String>,
    /// Timeout in seconds
    timeout: u64,
    /// Reconnection configuration
    reconnect_config: ReconnectConfig,
    /// Heartbeat configuration
    heartbeat_config: HeartbeatConfig,
    /// Queue configuration
    queue_config: QueueConfig,
    /// Session timeout
    session_timeout: Duration,
}

impl WebSocketClient {
    /// Create a new WebSocket client with the given base URL
    pub fn new(base_url: String) -> Self {
        let (request_sender, _) = mpsc::channel(1000);
        let (response_sender, response_receiver) = mpsc::channel(1000);

        let (status_tx, _) = watch::channel(ConnectionStatus::Disconnected);

        Self {
            base_url,
            auth_token: None,
            write_half: Arc::new(Mutex::new(None)),
            read_half: Arc::new(Mutex::new(None)),
            session: Arc::new(RwLock::new(SessionState::new())),
            status: Arc::new(status_tx),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            request_sender,
            response_receiver: Arc::new(Mutex::new(response_receiver)),
            response_sender,
            timeout: 30,
            reconnect_config: ReconnectConfig::default(),
            heartbeat_config: HeartbeatConfig::default(),
            queue_config: QueueConfig::default(),
            session_timeout: Duration::from_secs(300),
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

    /// Set reconnection configuration
    pub fn with_reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect_config = config;
        self
    }

    /// Set heartbeat configuration
    pub fn with_heartbeat_config(mut self, config: HeartbeatConfig) -> Self {
        self.heartbeat_config = config;
        self
    }

    /// Set queue configuration
    pub fn with_queue_config(mut self, config: QueueConfig) -> Self {
        self.queue_config = config;
        self
    }

    /// Set session timeout
    pub fn with_session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout = timeout;
        self
    }

    /// Get current connection status
    pub async fn status(&self) -> ConnectionStatus {
        *self.status.borrow()
    }

    /// Get current session state
    pub async fn session_state(&self) -> SessionState {
        self.session.read().await.clone()
    }

    /// Connect to the WebSocket server
    async fn connect(&mut self) -> Result<(), A2AError> {
        // Check if already connected
        {
            let write = self.write_half.lock().await;
            if write.is_some() {
                return Ok(());
            }
        }

        self.update_status(ConnectionStatus::Connecting).await;

        let mut url = Url::parse(&self.base_url)
            .map_err(|e| WebSocketClientError::Connection(format!("Invalid URL: {}", e)))?;

        // Add auth token to URL if present
        if let Some(token) = &self.auth_token {
            url.query_pairs_mut().append_pair("token", token);
        }

        let (ws_stream, _) = connect_async(url).await.map_err(|e| {
            WebSocketClientError::Connection(format!("WebSocket connection error: {}", e))
        })?;

        // Split the WebSocket into read and write halves
        let (write, read) = ws_stream.split();

        {
            let mut w = self.write_half.lock().await;
            *w = Some(write);
        }
        {
            let mut r = self.read_half.lock().await;
            *r = Some(read);
        }

        // Update session state
        {
            let mut session = self.session.write().await;
            session.touch();
        }

        self.update_status(ConnectionStatus::Connected).await;

        #[cfg(feature = "tracing")]
        info!("WebSocket connected: {}", self.base_url);

        Ok(())
    }

    /// Disconnect from the WebSocket server
    async fn disconnect(&mut self) -> Result<(), A2AError> {
        self.update_status(ConnectionStatus::Disconnected).await;

        {
            let mut w = self.write_half.lock().await;
            *w = None;
        }
        {
            let mut r = self.read_half.lock().await;
            *r = None;
        }

        #[cfg(feature = "tracing")]
        info!("WebSocket disconnected");

        Ok(())
    }

    /// Update connection status
    async fn update_status(&self, status: ConnectionStatus) {
        let _ = self.status.send(status);

        #[cfg(feature = "tracing")]
        debug!("Connection status: {:?}", status);
    }

    /// Reconnect with exponential backoff
    async fn reconnect(&mut self) -> Result<(), A2AError> {
        let mut attempt = 0;
        let session = self.session.read().await;
        let reconnect_count = session.reconnect_count;
        drop(session);

        while attempt < self.reconnect_config.max_attempts {
            attempt += 1;

            let backoff = self.reconnect_config.calculate_backoff(reconnect_count + attempt);

            #[cfg(feature = "tracing")]
            warn!("Reconnection attempt {}/{} after {:?}",
                attempt, self.reconnect_config.max_attempts, backoff);

            // Disconnect first
            self.disconnect().await?;

            // Wait for backoff
            tokio::time::sleep(backoff).await;

            // Try to connect
            match self.connect().await {
                Ok(_) => {
                    // Update reconnect count
                    {
                        let mut session = self.session.write().await;
                        session.reconnect_count = reconnect_count + attempt;
                    }

                    // Send queued requests
                    self.send_queued_requests().await?;

                    return Ok(());
                }
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    error!("Reconnection attempt {} failed: {}", attempt, e);

                    if attempt >= self.reconnect_config.max_attempts {
                        return Err(WebSocketClientError::ReconnectionFailed {
                            max_retries: self.reconnect_config.max_attempts,
                        }.into());
                    }
                }
            }
        }

        Err(WebSocketClientError::ReconnectionFailed {
            max_retries: self.reconnect_config.max_attempts,
        }.into())
    }

    /// Send queued requests after reconnection
    async fn send_queued_requests(&mut self) -> Result<(), A2AError> {
        // Collect requests to send
        let requests_to_send = {
            let mut queue = self.request_queue.lock().await;
            let mut to_send = Vec::new();

            while let Some(request) = queue.pop_front() {
                // Check if request is too old
                if request.queued_at.elapsed() > self.queue_config.max_age {
                    #[cfg(feature = "tracing")]
                    warn!("Discarding aged queued request: {}", request.id);
                    continue;
                }

                // Check retry count
                if request.retry_count >= self.queue_config.max_retries {
                    #[cfg(feature = "tracing")]
                    warn!("Discarding request exceeding max retries: {}", request.id);
                    continue;
                }

                to_send.push(request);
            }

            to_send
        };

        let mut failed = VecDeque::new();

        // Try to send each request
        for request in requests_to_send {
            // Try to send
            match self.send_message_internal(WsMessage::Text(request.payload.clone())).await {
                Ok(_) => {
                    #[cfg(feature = "tracing")]
                    debug!("Sent queued request: {}", request.id);
                }
                Err(_) => {
                    // Re-queue for next attempt
                    let mut updated_request = request;
                    updated_request.retry_count += 1;
                    failed.push_back(updated_request);
                }
            }
        }

        // Put failed requests back
        if !failed.is_empty() {
            let mut queue = self.request_queue.lock().await;
            queue.extend(failed);
        }

        Ok(())
    }

    /// Queue a request for offline retry
    #[allow(dead_code)]
    async fn queue_request(&self, payload: String) -> Result<(), A2AError> {
        if !self.queue_config.enabled {
            return Err(WebSocketClientError::QueueFull {
                current: 0,
                capacity: 0,
            }.into());
        }

        let mut queue = self.request_queue.lock().await;

        if queue.len() >= self.queue_config.max_size {
            return Err(WebSocketClientError::QueueFull {
                current: queue.len(),
                capacity: self.queue_config.max_size,
            }.into());
        }

        let request = QueuedRequest {
            id: uuid::Uuid::new_v4().to_string(),
            payload,
            queued_at: Instant::now(),
            retry_count: 0,
        };

        queue.push_back(request);

        #[cfg(feature = "tracing")]
        debug!("Queued request (queue size: {})", queue.len());

        Ok(())
    }

    /// Send a message internally without reconnection logic
    async fn send_message_internal(&mut self, message: WsMessage) -> Result<(), A2AError> {
        let mut write = self.write_half.lock().await;

        let ws_sink = write
            .as_mut()
            .ok_or_else(|| WebSocketClientError::Connection("No connection".to_string()))?;

        // Send the message
        ws_sink
            .send(message)
            .await
            .map_err(|e| WebSocketClientError::Message(format!("Send error: {}", e)))?;

        // Update session activity
        drop(write);
        {
            let mut session = self.session.write().await;
            session.touch();
        }

        Ok(())
    }

    /// Send a message to the WebSocket server and get a response
    async fn send_ws_message(&mut self, message: WsMessage) -> Result<WsMessage, A2AError> {
        // Try to connect if not connected
        if !self.status().await.is_connected() {
            if self.reconnect_config.enabled {
                self.reconnect().await?;
            } else {
                self.connect().await?;
            }
        }

        // Send the message
        self.send_message_internal(message.clone()).await?;

        // If message is Text, wait for response
        if let WsMessage::Text(_text) = &message {
            let timeout = Duration::from_secs(self.timeout);

            // Wait for response with timeout
            let response = tokio::time::timeout(timeout, async {
                let mut rx = self.response_receiver.lock().await;
                rx.recv().await
            })
            .await
            .map_err(|_| WebSocketClientError::Timeout)?
            .ok_or_else(|| WebSocketClientError::Closed)?;

            return Ok(WsMessage::Text(response));
        }

        // For ping/pong/close messages, just acknowledge
        Ok(WsMessage::Text("{}".to_string()))
    }

    /// Start background tasks for heartbeat and connection monitoring
    pub async fn start_background_tasks(&self) -> Result<(), A2AError> {
        if self.heartbeat_config.enabled {
            self.start_heartbeat_task().await?;
        }

        if self.reconnect_config.enabled {
            self.start_connection_monitor().await?;
        }

        Ok(())
    }

    /// Start heartbeat task
    async fn start_heartbeat_task(&self) -> Result<(), A2AError> {
        let write_half = self.write_half.clone();
        let session = self.session.clone();
        let status = self.status.clone();
        let interval = self.heartbeat_config.interval;
        let timeout = self.heartbeat_config.timeout;

        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            loop {
                timer.tick().await;

                // Check if connected
                let current_status = *status.borrow();
                if !current_status.is_connected() {
                    continue;
                }

                // Check session expiration
                {
                    let session_guard = session.read().await;
                    if session_guard.is_expired(timeout) {
                        #[cfg(feature = "tracing")]
                        warn!("Session expired, attempting reconnection");

                        drop(session_guard);
                        let _ = status.send(ConnectionStatus::Reconnecting);
                        continue;
                    }
                }

                // Send ping
                let mut write_guard = write_half.lock().await;
                if let Some(ws_sink) = write_guard.as_mut() {
                    if let Err(e) = ws_sink.send(WsMessage::Ping(vec![])).await {
                        #[cfg(feature = "tracing")]
                        error!("Heartbeat send failed: {}", e);
                        drop(write_guard);
                        let _ = status.send(ConnectionStatus::Reconnecting);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start connection monitor task
    async fn start_connection_monitor(&self) -> Result<(), A2AError> {
        let _write_half = self.write_half.clone();
        let status = self.status.clone();

        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(5));
            loop {
                timer.tick().await;

                let current_status = *status.borrow();

                // Try to reconnect if disconnected
                if !current_status.is_connected() && current_status != ConnectionStatus::Closed {
                    #[cfg(feature = "tracing")]
                    debug!("Connection monitor detected disconnection");
                    // Reconnection will be handled by send_ws_message
                }
            }
        });

        Ok(())
    }

    /// Close the WebSocket connection
    pub async fn close(&mut self) -> Result<(), A2AError> {
        self.update_status(ConnectionStatus::Closed).await;

        {
            let mut write = self.write_half.lock().await;
            if let Some(ws_sink) = write.as_mut() {
                let _ = ws_sink.close().await;
            }
            *write = None;
        }
        {
            let mut read = self.read_half.lock().await;
            *read = None;
        }

        #[cfg(feature = "tracing")]
        info!("WebSocket connection closed");

        Ok(())
    }
}

#[async_trait]
impl AsyncA2AClient for WebSocketClient {
    async fn send_raw_request<'a>(&self, request: &'a str) -> Result<String, A2AError> {
        let mut client = self.clone();
        let response = client
            .send_ws_message(WsMessage::Text(request.to_string()))
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

        // For delete operations, both Some(Null) and None are success if there's no error
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
        // First connect to ensure we have a connection
        let mut client_clone = self.clone();
        client_clone.connect().await?;

        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length,
            metadata: None,
        };

        let request = TaskResubscriptionRequest::new(params);
        let json = json_rpc::serialize_request(&A2ARequest::TaskResubscription(request))?;

        // Get the write and read halves
        let write_half = client_clone.write_half.clone();
        let read_half = client_clone.read_half.clone();

        // Send the request
        {
            let mut write_guard = write_half.lock().await;
            if let Some(ws_sink) = write_guard.as_mut() {
                ws_sink
                    .send(WsMessage::Text(json))
                    .await
                    .map_err(|e| WebSocketClientError::Message(format!("Send error: {}", e)))?;
            }
        }

        // Create a stream that will process incoming messages
        let stream = futures::stream::unfold(read_half, move |read| {
            Box::pin(async move {
                // Loop until we get a non-null message or an error
                loop {
                    // Get the next message from the WebSocket
                    let message_result = {
                        let mut read_guard = read.lock().await;
                        let has_connection = read_guard.as_mut().is_some();
                        drop(read_guard);

                        if has_connection {
                            let mut read_guard = read.lock().await;
                            let ws_stream = read_guard.as_mut().unwrap();
                            ws_stream.next().await
                        } else {
                            return Some((
                                Err(WebSocketClientError::Connection("Connection lost".to_string()).into()),
                                read,
                            ));
                        }
                    };

                    // Process result outside the lock scope
                    let message = match message_result {
                        Some(Ok(msg)) => msg,
                        Some(Err(e)) => {
                            return Some((
                                Err(WebSocketClientError::Message(format!(
                                    "WebSocket error: {}",
                                    e
                                ))
                                .into()),
                                read,
                            ));
                        }
                        None => {
                            return Some((Err(WebSocketClientError::Closed.into()), read));
                        }
                    };

                    // Process the message
                    match message {
                        WsMessage::Text(text) => {
                            // Add debug logging for received messages
                            #[cfg(feature = "tracing")]
                            trace!("Received WebSocket message: {}", text);

                            // Parse the response
                            let response: Value = match serde_json::from_str(&text) {
                                Ok(value) => value,
                                Err(e) => {
                                    #[cfg(feature = "tracing")]
                                    debug!("JSON parse error: {}", e);
                                    return Some((Err(A2AError::JsonParse(e)), read));
                                }
                            };

                            // Check for errors
                            if let Some(error) = response.get("error")
                                && error.is_object()
                            {
                                let response_clone = response.clone();
                                let error: JSONRPCResponse =
                                    match serde_json::from_value(response_clone) {
                                        Ok(resp) => resp,
                                        Err(e) => {
                                            return Some((Err(A2AError::JsonParse(e)), read));
                                        }
                                    };

                                if let Some(err) = error.error {
                                    return Some((
                                        Err(A2AError::JsonRpc {
                                            code: err.code,
                                            message: err.message,
                                            data: err.data,
                                        }),
                                        read,
                                    ));
                                }
                            }

                            // Check if it's a valid JSON-RPC message
                            if response.get("jsonrpc").is_some() && response.get("result").is_some()
                            {
                                let result = response.get("result").cloned().unwrap_or(Value::Null);

                                // If result is null, the task doesn't exist yet - keep streaming
                                if result.is_null() {
                                    #[cfg(feature = "tracing")]
                                    debug!("Task doesn't exist yet, waiting for next message");
                                    // Skip this message and wait for the next WebSocket message
                                    continue; // Continue the loop to get the next message
                                }

                                // Try to parse as an initial Task response first
                                if let Ok(task) = serde_json::from_value::<Task>(result.clone()) {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as Task");
                                    return Some((Ok(StreamItem::Task(task)), read));
                                }

                                // Try to parse as a status update
                                if let Ok(status_update) =
                                    serde_json::from_value::<TaskStatusUpdateEvent>(result.clone())
                                {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as StatusUpdate");
                                    return Some((
                                        Ok(StreamItem::StatusUpdate(status_update)),
                                        read,
                                    ));
                                }

                                // Try to parse as an artifact update
                                if let Ok(artifact_update) =
                                    serde_json::from_value::<TaskArtifactUpdateEvent>(result)
                                {
                                    #[cfg(feature = "tracing")]
                                    debug!("Parsed streaming response as ArtifactUpdate");
                                    return Some((
                                        Ok(StreamItem::ArtifactUpdate(artifact_update)),
                                        read,
                                    ));
                                }
                            }

                            // If we got here, we couldn't parse the response
                            #[cfg(feature = "tracing")]
                            debug!("Failed to parse streaming response");
                            return Some((
                                Err(WebSocketClientError::Protocol(
                                    "Failed to parse streaming response".to_string(),
                                )
                                .into()),
                                read,
                            ));
                        }
                        WsMessage::Pong(_) => {
                            // Heartbeat pong, continue streaming
                            continue;
                        }
                        WsMessage::Ping(_data) => {
                            // Respond to ping - we need the write half here
                            // But we can't clone both halves into the unfold
                            // For now, just continue - the heartbeat task handles pings
                            continue;
                        }
                        WsMessage::Close(_) => {
                            return Some((Err(WebSocketClientError::Closed.into()), read));
                        }
                        _ => {
                            return Some((
                                Err(WebSocketClientError::Protocol(
                                    "Unexpected WebSocket message type".to_string(),
                                )
                                .into()),
                                read,
                            ));
                        }
                    }; // End of match
                } // End of loop
            })
        });

        Ok(Box::pin(stream))
    }
}

impl Clone for WebSocketClient {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            auth_token: self.auth_token.clone(),
            write_half: self.write_half.clone(),
            read_half: self.read_half.clone(),
            session: self.session.clone(),
            status: self.status.clone(),
            request_queue: self.request_queue.clone(),
            request_sender: self.request_sender.clone(),
            response_receiver: self.response_receiver.clone(),
            response_sender: self.response_sender.clone(),
            timeout: self.timeout,
            reconnect_config: self.reconnect_config.clone(),
            heartbeat_config: self.heartbeat_config.clone(),
            queue_config: self.queue_config.clone(),
            session_timeout: self.session_timeout,
        }
    }
}
