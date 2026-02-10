//! WebSocket transport adapter with bidirectional streaming and reconnection.
//!
//! Implements the `Transport` port using tokio-tungstenite with:
//! - Bidirectional TypedPacket streaming
//! - Ping/pong heartbeat mechanism
//! - Exponential backoff reconnection logic
//! - Graceful connection lifecycle management

#[cfg(feature = "ws")]
use crate::domain::TypedPacket;
#[cfg(feature = "ws")]
use crate::port::transport::{TransportConfig, TransportError, TransportStatus};
#[cfg(feature = "ws")]
use async_trait::async_trait;
#[cfg(feature = "ws")]
use futures::sink::SinkExt;
#[cfg(feature = "ws")]
use futures::stream::StreamExt;
#[cfg(feature = "ws")]
use std::time::Duration;
#[cfg(feature = "ws")]
use tokio::net::TcpStream;
#[cfg(feature = "ws")]
use tokio::sync::mpsc;
#[cfg(feature = "ws")]
use tokio::time::{Instant, sleep};
#[cfg(feature = "ws")]
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
#[cfg(feature = "ws")]
use tracing::{debug, error, warn};

#[cfg(feature = "ws")]
/// WebSocket transport adapter
pub struct WebSocketTransport {
    /// Configuration
    config: TransportConfig,

    /// WebSocket connection
    ws: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,

    /// Current status
    status: TransportStatus,

    /// Reconnection attempt counter
    reconnect_attempts: u32,

    /// Last ping sent time
    last_ping: Option<Instant>,

    /// Shutdown signal receiver
    shutdown_rx: Option<mpsc::UnboundedReceiver<()>>,

    /// Shutdown signal sender
    shutdown_tx: mpsc::UnboundedSender<()>,
}

#[cfg(feature = "ws")]
impl WebSocketTransport {
    /// Create a new WebSocket transport
    #[must_use]
    pub fn new(config: TransportConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();
        Self {
            config,
            ws: None,
            status: TransportStatus::Disconnected,
            reconnect_attempts: 0,
            last_ping: None,
            shutdown_rx: Some(shutdown_rx),
            shutdown_tx,
        }
    }

    /// Create with default config
    #[must_use]
    pub fn with_default_config(url: impl Into<String>) -> Self {
        Self::new(TransportConfig::new(url))
    }

    /// Check if we should send a ping
    fn should_ping(&self) -> bool {
        match self.last_ping {
            None => true,
            Some(last) => last.elapsed() >= self.config.ping_interval,
        }
    }

    /// Calculate backoff delay for reconnection
    fn calculate_backoff(&self) -> Duration {
        let delay_ms = (self.config.initial_reconnect_delay.as_millis() as f64
            * self
                .config
                .reconnect_backoff
                .powi(self.reconnect_attempts as i32)) as u64;

        let max_ms = self.config.max_reconnect_delay.as_millis() as u64;
        Duration::from_millis(delay_ms.min(max_ms))
    }

    /// Check if max retries exceeded
    fn max_retries_exceeded(&self) -> bool {
        match self.config.max_reconnect_attempts {
            None => false,
            Some(max) => self.reconnect_attempts >= max,
        }
    }

    /// Reset reconnection counter
    fn reset_reconnection_counter(&mut self) {
        self.reconnect_attempts = 0;
    }

    /// Increment reconnection counter
    fn increment_reconnection_counter(&mut self) {
        self.reconnect_attempts = self.reconnect_attempts.saturating_add(1);
    }

    /// Handle incoming message
    fn handle_message(&self, msg: Message) -> Result<Option<TypedPacket>, TransportError> {
        match msg {
            Message::Text(text) => match serde_json::from_str::<TypedPacket>(&text) {
                Ok(packet) => Ok(Some(packet)),
                Err(e) => Err(TransportError::SerializationError(e.to_string())),
            },
            Message::Binary(bytes) => match serde_json::from_slice::<TypedPacket>(&bytes) {
                Ok(packet) => Ok(Some(packet)),
                Err(e) => Err(TransportError::SerializationError(e.to_string())),
            },
            Message::Ping(_data) => {
                debug!("Received ping, responding with pong");
                Ok(None) // Will be handled in receive loop
            }
            Message::Pong(_) => {
                debug!("Received pong");
                Ok(None)
            }
            Message::Close(frame) => {
                debug!("Received close: {:?}", frame.map(|f| f.reason.to_string()));
                Ok(None)
            }
            Message::Frame(_) => {
                warn!("Received raw frame");
                Ok(None)
            }
        }
    }

    /// Send internal message
    async fn send_message(&mut self, msg: Message) -> Result<(), TransportError> {
        if let Some(ws) = &mut self.ws {
            ws.send(msg).await.map_err(|e| {
                error!("Send failed: {}", e);
                TransportError::SendFailed(e.to_string())
            })?;
            Ok(())
        } else {
            Err(TransportError::NotConnected)
        }
    }

    /// Perform handshake with server (after connection)
    async fn perform_handshake(&mut self) -> Result<(), TransportError> {
        // Send initial handshake message (optional - can be customized)
        debug!("WebSocket connected, handshake complete");
        self.reset_reconnection_counter();
        self.status = TransportStatus::Connected;
        self.last_ping = Some(Instant::now());
        Ok(())
    }

    /// Poll for ping/pong maintenance
    async fn poll_heartbeat(&mut self) -> Result<(), TransportError> {
        if self.status != TransportStatus::Connected {
            return Ok(());
        }

        if self.should_ping() {
            self.send_message(Message::Ping(vec![])).await?;
            self.last_ping = Some(Instant::now());
            debug!("Sent ping frame");
        }

        Ok(())
    }
}

#[cfg(feature = "ws")]
#[async_trait]
impl crate::port::Transport for WebSocketTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        if self.status == TransportStatus::Connected {
            return Err(TransportError::AlreadyConnected);
        }

        debug!("Connecting to {}", self.config.url);
        self.status = TransportStatus::Connecting;

        match connect_async(&self.config.url).await {
            Ok((ws, _response)) => {
                self.ws = Some(ws);
                self.perform_handshake().await?;
                debug!("Connected successfully to {}", self.config.url);
                Ok(())
            }
            Err(e) => {
                error!("Connection failed: {}", e);
                self.status = TransportStatus::Disconnected;
                Err(TransportError::ConnectionFailed(e.to_string()))
            }
        }
    }

    async fn send(&mut self, packet: TypedPacket) -> Result<(), TransportError> {
        if self.status != TransportStatus::Connected {
            return Err(TransportError::NotConnected);
        }

        let json = serde_json::to_string(&packet)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;

        self.send_message(Message::Text(json)).await?;
        debug!("Sent packet: {}", packet.id);

        Ok(())
    }

    async fn receive(&mut self) -> Result<Option<TypedPacket>, TransportError> {
        if self.status != TransportStatus::Connected {
            return Err(TransportError::NotConnected);
        }

        // Poll heartbeat
        self.poll_heartbeat().await?;

        // Try to receive next message
        if let Some(ws) = &mut self.ws {
            match tokio::time::timeout(self.config.pong_timeout, ws.next()).await {
                Ok(Some(Ok(msg))) => {
                    match self.handle_message(msg)? {
                        Some(packet) => {
                            debug!("Received packet: {}", packet.id);
                            Ok(Some(packet))
                        }
                        None => {
                            // Non-packet message (ping/pong/close), try again
                            Ok(None)
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("WebSocket error: {}", e);
                    self.status = TransportStatus::Degraded;
                    Err(TransportError::ReceiveFailed(e.to_string()))
                }
                Ok(None) => {
                    debug!("WebSocket closed by server");
                    self.status = TransportStatus::Disconnected;
                    self.ws = None;
                    Ok(None)
                }
                Err(_) => {
                    // Timeout - close connection
                    error!("Receive timeout");
                    self.status = TransportStatus::Degraded;
                    Err(TransportError::Timeout(
                        "Pong not received within timeout".to_string(),
                    ))
                }
            }
        } else {
            Err(TransportError::NotConnected)
        }
    }

    fn status(&self) -> TransportStatus {
        self.status
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
            debug!("WebSocket disconnected");
        }

        self.status = TransportStatus::Disconnected;
        self.last_ping = None;

        Ok(())
    }

    async fn reconnect(&mut self) -> Result<(), TransportError> {
        if self.max_retries_exceeded() {
            error!("Max reconnection retries exceeded");
            self.status = TransportStatus::Failed;
            return Err(TransportError::MaxRetriesExhausted);
        }

        self.increment_reconnection_counter();
        let backoff = self.calculate_backoff();

        warn!(
            "Reconnecting (attempt {}) after {:?}",
            self.reconnect_attempts, backoff
        );
        self.status = TransportStatus::Degraded;

        sleep(backoff).await;

        match self.connect().await {
            Ok(()) => {
                debug!("Reconnection successful");
                self.reset_reconnection_counter();
                Ok(())
            }
            Err(e) => {
                error!("Reconnection failed: {}", e);
                Err(e)
            }
        }
    }

    async fn send_batch(&mut self, packets: Vec<TypedPacket>) -> Result<(), TransportError> {
        if self.status != TransportStatus::Connected {
            return Err(TransportError::NotConnected);
        }

        for packet in packets {
            self.send(packet).await?;
        }

        Ok(())
    }

    async fn receive_batch(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<TypedPacket>, TransportError> {
        let start = Instant::now();
        let mut packets = Vec::new();

        while start.elapsed() < timeout {
            let remaining = timeout - start.elapsed();
            match tokio::time::timeout(remaining, self.receive()).await {
                Ok(Ok(Some(packet))) => packets.push(packet),
                Ok(Ok(None)) => {
                    // Non-packet message, continue
                    continue;
                }
                Ok(Err(TransportError::Timeout(_))) => {
                    // Individual timeout, continue
                    continue;
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => break, // Overall timeout
            }
        }

        Ok(packets)
    }
}

#[cfg(feature = "ws")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_builder() {
        let config = TransportConfig::new("ws://localhost:8080/ws")
            .with_ping_interval(Duration::from_secs(15))
            .with_pong_timeout(Duration::from_secs(5))
            .with_buffer_size(512);

        assert_eq!(config.url, "ws://localhost:8080/ws");
        assert_eq!(config.ping_interval, Duration::from_secs(15));
        assert_eq!(config.pong_timeout, Duration::from_secs(5));
        assert_eq!(config.buffer_size, 512);
    }

    #[test]
    fn test_transport_config_defaults() {
        let config = TransportConfig::default();

        assert_eq!(config.url, "ws://localhost:8080/ws");
        assert_eq!(config.ping_interval, Duration::from_secs(30));
        assert_eq!(config.pong_timeout, Duration::from_secs(10));
        assert_eq!(config.max_reconnect_attempts, Some(10));
    }

    #[test]
    fn test_websocket_transport_creation() {
        let config = TransportConfig::new("ws://localhost:8080/ws");
        let transport = WebSocketTransport::new(config);

        assert_eq!(transport.status, TransportStatus::Disconnected);
        assert_eq!(transport.reconnect_attempts, 0);
        assert!(!transport.is_connected());
    }

    #[test]
    fn test_backoff_calculation() {
        let config = TransportConfig::new("ws://localhost:8080/ws").with_reconnect_config(
            Duration::from_millis(100),
            Duration::from_secs(30),
            2.0,
        );

        let mut transport = WebSocketTransport::new(config);

        // First backoff: 100ms
        let backoff = transport.calculate_backoff();
        assert_eq!(backoff, Duration::from_millis(100));

        // Second backoff: 200ms
        transport.increment_reconnection_counter();
        let backoff = transport.calculate_backoff();
        assert_eq!(backoff, Duration::from_millis(200));

        // Third backoff: 400ms
        transport.increment_reconnection_counter();
        let backoff = transport.calculate_backoff();
        assert_eq!(backoff, Duration::from_millis(400));

        // Fourth backoff: 800ms
        transport.increment_reconnection_counter();
        let backoff = transport.calculate_backoff();
        assert_eq!(backoff, Duration::from_millis(800));

        // Should cap at max (30s)
        transport.reconnect_attempts = 100;
        let backoff = transport.calculate_backoff();
        assert_eq!(backoff, Duration::from_secs(30));
    }

    #[test]
    fn test_max_retries_exceeded() {
        let config =
            TransportConfig::new("ws://localhost:8080/ws").with_max_reconnect_attempts(Some(3));

        let mut transport = WebSocketTransport::new(config);

        assert!(!transport.max_retries_exceeded());

        transport.reconnect_attempts = 2;
        assert!(!transport.max_retries_exceeded());

        transport.reconnect_attempts = 3;
        assert!(transport.max_retries_exceeded());
    }

    #[test]
    fn test_ping_timing() {
        let config = TransportConfig::new("ws://localhost:8080/ws")
            .with_ping_interval(Duration::from_secs(5));

        let transport = WebSocketTransport::new(config);

        // No ping sent yet
        assert!(transport.should_ping());

        // This would need async testing for actual timing
    }

    #[test]
    fn test_reconnection_counter_reset() {
        let config = TransportConfig::new("ws://localhost:8080/ws");
        let mut transport = WebSocketTransport::new(config);

        transport.reconnect_attempts = 5;
        assert_eq!(transport.reconnect_attempts, 5);

        transport.reset_reconnection_counter();
        assert_eq!(transport.reconnect_attempts, 0);
    }

    #[test]
    fn test_status_transitions() {
        let config = TransportConfig::new("ws://localhost:8080/ws");
        let transport = WebSocketTransport::new(config);

        assert_eq!(transport.status(), TransportStatus::Disconnected);
        assert!(!transport.is_connected());
    }
}
