//! Error types for osiris-edge operations

use thiserror::Error;

/// Errors that can occur during WIP gate operations
#[derive(Debug, Clone, Error)]
pub enum WipError {
    /// WIP limit reached - work is rejected
    #[error("WIP limit reached: {current}/{limit} slots occupied")]
    WipLimitReached { current: usize, limit: usize },

    /// Gate is closed and not accepting new work
    #[error("Gate is closed")]
    GateClosed,

    /// Work execution failed
    #[error("Work execution failed: {0}")]
    ExecutionFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// General edge gateway error type
#[derive(Debug, Error)]
pub enum EdgeError {
    /// Authentication error
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Authorization error
    #[error("Authorization failed: {0}")]
    Authorization(String),

    /// Token validation error
    #[error("Token validation failed: {0}")]
    TokenValidation(String),

    /// Invalid token format
    #[error("Invalid token format: {0}")]
    InvalidToken(String),

    /// Token expired
    #[error("Token expired")]
    TokenExpired,

    /// Missing required claim
    #[error("Missing required claim: {0}")]
    MissingClaim(String),

    /// Invalid issuer
    #[error("Invalid issuer: expected {expected}, got {actual}")]
    InvalidIssuer { expected: String, actual: String },

    /// Invalid audience
    #[error("Invalid audience: expected {expected}, got {actual}")]
    InvalidAudience { expected: String, actual: String },

    /// HTTP client error
    #[error("HTTP request failed: {0}")]
    HttpClient(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// WIP error
    #[error("WIP error: {0}")]
    Wip(#[from] WipError),

    /// Event bus error
    #[error("Event bus error: {0}")]
    EventBus(#[from] EventBusError),
}

/// Errors that can occur during event bus operations
#[derive(Debug, Clone, Error)]
pub enum EventBusError {
    /// Topic not found
    #[error("Topic not found: {0}")]
    TopicNotFound(String),

    /// Failed to create topic
    #[error("Failed to create topic: {0}")]
    TopicCreationFailed(String),

    /// Failed to publish message
    #[error("Failed to publish message: {0}")]
    PublishFailed(String),

    /// Failed to subscribe to topic
    #[error("Failed to subscribe to topic: {0}")]
    SubscriptionFailed(String),

    /// Failed to receive message
    #[error("Failed to receive message: {0}")]
    ReceiveError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Invalid topic name
    #[error("Invalid topic name: {0}")]
    InvalidTopicName(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}
