//! Port trait for consuming entitlement events from Google Cloud Pub/Sub.

use crate::domain::{EntitlementEvent, PubSubMessage};
use async_trait::async_trait;
use thiserror::Error;

/// Errors that can occur when consuming events
#[derive(Debug, Error)]
pub enum EventConsumerError {
    /// Failed to connect to Pub/Sub
    #[error("Failed to connect to Pub/Sub: {0}")]
    ConnectionError(String),

    /// Failed to decode message
    #[error("Failed to decode message: {0}")]
    DecodeError(String),

    /// Failed to parse event payload
    #[error("Failed to parse event payload: {0}")]
    ParseError(String),

    /// Failed to acknowledge message
    #[error("Failed to acknowledge message: {0}")]
    AckError(String),

    /// Subscription error
    #[error("Subscription error: {0}")]
    SubscriptionError(String),

    /// Other error
    #[error("Event consumer error: {0}")]
    Other(String),
}

/// Result type for event consumer operations
pub type EventConsumerResult<T> = Result<T, EventConsumerError>;

/// Port trait for consuming entitlement events from Partner Pub/Sub topic.
///
/// Implementations should:
/// - Connect to the configured Pub/Sub subscription
/// - Pull messages from the subscription
/// - Decode base64-encoded message data
/// - Parse JSON payloads into EntitlementEvent
/// - Acknowledge successfully processed messages
#[async_trait]
pub trait EventConsumer: Send + Sync {
    /// Start consuming events and invoke the handler for each event.
    ///
    /// This method should run indefinitely, pulling messages from the subscription
    /// and invoking the handler callback for each event. The consumer should
    /// acknowledge messages after the handler returns Ok.
    ///
    /// # Arguments
    ///
    /// * `handler` - Async callback to process each event
    ///
    /// # Returns
    ///
    /// Returns an error if the subscription fails or cannot be established.
    /// Individual message processing errors should be logged but not stop consumption.
    async fn consume<F, Fut>(&self, handler: F) -> EventConsumerResult<()>
    where
        F: Fn(EntitlementEvent) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>
            + Send;

    /// Pull a single batch of messages from the subscription.
    ///
    /// This is useful for testing or controlled processing scenarios.
    ///
    /// # Arguments
    ///
    /// * `max_messages` - Maximum number of messages to pull
    ///
    /// # Returns
    ///
    /// A vector of raw Pub/Sub messages
    async fn pull_messages(&self, max_messages: i32) -> EventConsumerResult<Vec<PubSubMessage>>;

    /// Parse a Pub/Sub message into an EntitlementEvent.
    ///
    /// # Arguments
    ///
    /// * `message` - The Pub/Sub message to parse
    ///
    /// # Returns
    ///
    /// The parsed EntitlementEvent
    fn parse_event(&self, message: &PubSubMessage) -> EventConsumerResult<EntitlementEvent>;

    /// Acknowledge a processed message.
    ///
    /// # Arguments
    ///
    /// * `message_id` - The ID of the message to acknowledge
    async fn acknowledge(&self, message_id: &str) -> EventConsumerResult<()>;
}
