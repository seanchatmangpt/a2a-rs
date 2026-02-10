//! Event bus port definition
//!
//! Defines the interface for publish/subscribe event distribution
//! using event ordering support (e.g., Google Cloud Pub/Sub ordered topics).

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::domain::EventBusError;

/// Configuration for topic creation with ordering support
#[derive(Debug, Clone)]
pub struct TopicConfig {
    /// Topic name (must be valid GCP topic name format)
    pub name: String,
    /// Enable message ordering by ordering key
    pub enable_ordering: bool,
    /// Optional labels for the topic
    pub labels: HashMap<String, String>,
}

impl TopicConfig {
    /// Create a new topic configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enable_ordering: false,
            labels: HashMap::new(),
        }
    }

    /// Enable message ordering for this topic
    pub fn with_ordering(mut self) -> Self {
        self.enable_ordering = true;
        self
    }

    /// Add a label to this topic
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// A published message with metadata
#[derive(Debug, Clone)]
pub struct PubMessage {
    /// Message ID assigned by the bus (set after publish)
    pub message_id: Option<String>,
    /// Message payload as JSON
    pub data: JsonValue,
    /// Optional ordering key for ordered topics
    pub ordering_key: Option<String>,
    /// Optional attributes (metadata)
    pub attributes: HashMap<String, String>,
    /// Timestamp when message was published
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl PubMessage {
    /// Create a new message with JSON data
    pub fn new(data: JsonValue) -> Self {
        Self {
            message_id: None,
            data,
            ordering_key: None,
            attributes: HashMap::new(),
            timestamp: None,
        }
    }

    /// Set the ordering key for ordered topics
    pub fn with_ordering_key(mut self, key: impl Into<String>) -> Self {
        self.ordering_key = Some(key.into());
        self
    }

    /// Add an attribute to the message
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// Subscription configuration
#[derive(Debug, Clone)]
pub struct SubscriptionConfig {
    /// Subscription name
    pub name: String,
    /// Topic to subscribe to
    pub topic: String,
    /// Acknowledgement deadline in seconds
    pub ack_deadline_secs: u32,
    /// Maximum number of messages to fetch at once
    pub max_messages: u32,
}

impl SubscriptionConfig {
    /// Create a new subscription configuration
    pub fn new(name: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            topic: topic.into(),
            ack_deadline_secs: 10,
            max_messages: 100,
        }
    }

    /// Set the acknowledgement deadline
    pub fn with_ack_deadline(mut self, secs: u32) -> Self {
        self.ack_deadline_secs = secs;
        self
    }

    /// Set the maximum messages to fetch
    pub fn with_max_messages(mut self, max: u32) -> Self {
        self.max_messages = max;
        self
    }
}

/// Received message with acknowledgement capability
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    /// Message ID from the broker
    pub message_id: String,
    /// Message payload
    pub data: JsonValue,
    /// Message attributes
    pub attributes: HashMap<String, String>,
    /// Ordering key used for delivery
    pub ordering_key: Option<String>,
    /// Timestamp when message was published
    pub publish_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Acknowledgement ID for manual ack
    pub ack_id: String,
}

/// Event bus port for publish/subscribe operations
///
/// Provides a unified interface to event brokers with support for:
/// - Topic creation and management
/// - Message publishing with optional ordering
/// - Subscription and message consumption
/// - Message acknowledgement semantics
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Create a new topic
    ///
    /// # Arguments
    /// * `config` - Topic configuration
    ///
    /// # Errors
    /// Returns `EventBusError::TopicCreationFailed` if creation fails
    async fn create_topic(&self, config: TopicConfig) -> Result<(), EventBusError>;

    /// Publish a message to a topic
    ///
    /// # Arguments
    /// * `topic` - Topic name
    /// * `message` - Message to publish
    ///
    /// # Errors
    /// Returns `EventBusError::PublishFailed` if publish fails
    /// Returns `EventBusError::TopicNotFound` if topic doesn't exist
    async fn publish(&self, topic: &str, message: PubMessage) -> Result<String, EventBusError>;

    /// Publish multiple messages to a topic
    ///
    /// # Arguments
    /// * `topic` - Topic name
    /// * `messages` - Messages to publish
    ///
    /// # Returns
    /// Vector of published message IDs
    ///
    /// # Errors
    /// Returns `EventBusError::PublishFailed` if any publish fails
    async fn publish_batch(
        &self,
        topic: &str,
        messages: Vec<PubMessage>,
    ) -> Result<Vec<String>, EventBusError>;

    /// Subscribe to a topic
    ///
    /// # Arguments
    /// * `config` - Subscription configuration
    ///
    /// # Errors
    /// Returns `EventBusError::SubscriptionFailed` if subscription fails
    async fn subscribe(&self, config: SubscriptionConfig) -> Result<(), EventBusError>;

    /// Receive messages from a subscription (non-blocking)
    ///
    /// # Arguments
    /// * `subscription` - Subscription name
    ///
    /// # Returns
    /// Vector of received messages (may be empty if no messages available)
    ///
    /// # Errors
    /// Returns `EventBusError::ReceiveError` if receive fails
    async fn receive(&self, subscription: &str) -> Result<Vec<ReceivedMessage>, EventBusError>;

    /// Acknowledge a message (mark as successfully processed)
    ///
    /// # Arguments
    /// * `subscription` - Subscription name
    /// * `ack_ids` - Acknowledgement IDs
    ///
    /// # Errors
    /// Returns `EventBusError::Internal` if ack fails
    async fn acknowledge(
        &self,
        subscription: &str,
        ack_ids: Vec<String>,
    ) -> Result<(), EventBusError>;

    /// Nack a message (mark for redelivery)
    ///
    /// # Arguments
    /// * `subscription` - Subscription name
    /// * `ack_ids` - Acknowledgement IDs to nack
    ///
    /// # Errors
    /// Returns `EventBusError::Internal` if nack fails
    async fn nack(&self, subscription: &str, ack_ids: Vec<String>) -> Result<(), EventBusError>;

    /// Check if a topic exists
    ///
    /// # Arguments
    /// * `topic` - Topic name
    async fn topic_exists(&self, topic: &str) -> Result<bool, EventBusError>;

    /// Check if a subscription exists
    ///
    /// * `subscription` - Subscription name
    async fn subscription_exists(&self, subscription: &str) -> Result<bool, EventBusError>;

    /// Delete a topic
    ///
    /// # Arguments
    /// * `topic` - Topic name
    ///
    /// # Errors
    /// Returns `EventBusError::TopicNotFound` if topic doesn't exist
    async fn delete_topic(&self, topic: &str) -> Result<(), EventBusError>;

    /// Delete a subscription
    ///
    /// # Arguments
    /// * `subscription` - Subscription name
    ///
    /// # Errors
    /// Returns `EventBusError::SubscriptionFailed` if subscription doesn't exist
    async fn delete_subscription(&self, subscription: &str) -> Result<(), EventBusError>;
}
