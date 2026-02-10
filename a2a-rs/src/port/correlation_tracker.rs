//! Correlation ID tracking port for request/response pairs
//!
//! This port defines the interface for tracking correlation between requests
//! and responses, enabling request timeout management and orphaned request detection.

use async_trait::async_trait;
use std::time::Duration;
use uuid::Uuid;

/// Metadata associated with a tracked request
#[derive(Debug, Clone)]
pub struct CorrelationMetadata {
    /// Unique correlation ID for the request
    pub correlation_id: Uuid,
    /// Timestamp when the request was created
    pub created_at: std::time::Instant,
    /// Optional timeout duration for the request
    pub timeout: Option<Duration>,
    /// Optional context data associated with the request
    pub context: Option<serde_json::Value>,
}

/// Statistics about correlation tracking
#[derive(Debug, Clone)]
pub struct CorrelationStats {
    /// Total number of currently pending requests
    pub pending_count: usize,
    /// Number of requests that timed out
    pub timed_out_count: usize,
    /// Number of successfully matched responses
    pub matched_count: usize,
    /// Number of orphaned requests (no response received)
    pub orphaned_count: usize,
}

/// Port interface for correlation ID tracking
///
/// Implementations provide thread-safe tracking of request/response pairs
/// using correlation IDs, with support for timeouts and metrics collection.
#[async_trait]
pub trait CorrelationTracker: Send + Sync {
    /// Generate a new correlation ID (UUID v7)
    ///
    /// Returns a time-ordered UUID suitable for correlation tracking.
    fn generate_correlation_id(&self) -> Uuid;

    /// Track a new pending request with optional timeout
    ///
    /// # Arguments
    /// * `correlation_id` - Unique identifier for this request
    /// * `timeout` - Optional timeout duration
    /// * `context` - Optional context data to associate with this request
    ///
    /// # Returns
    /// `Ok(())` if tracking started successfully, or an error message
    async fn track_request(
        &self,
        correlation_id: Uuid,
        timeout: Option<Duration>,
        context: Option<serde_json::Value>,
    ) -> Result<(), String>;

    /// Mark a request as having received a response
    ///
    /// # Arguments
    /// * `correlation_id` - The correlation ID of the request
    ///
    /// # Returns
    /// `Ok(metadata)` with the original request metadata if found,
    /// or an error if the correlation ID was not found
    async fn match_response(&self, correlation_id: Uuid) -> Result<CorrelationMetadata, String>;

    /// Check if a request is still pending
    ///
    /// # Arguments
    /// * `correlation_id` - The correlation ID to check
    ///
    /// # Returns
    /// `true` if the request is pending, `false` otherwise
    async fn is_pending(&self, correlation_id: Uuid) -> bool;

    /// Get metadata for a pending request
    ///
    /// # Arguments
    /// * `correlation_id` - The correlation ID to look up
    ///
    /// # Returns
    /// `Some(metadata)` if found, `None` otherwise
    async fn get_metadata(&self, correlation_id: Uuid) -> Option<CorrelationMetadata>;

    /// Remove all timed-out requests and return their correlation IDs
    ///
    /// # Returns
    /// Vector of correlation IDs for requests that have timed out
    async fn cleanup_timed_out(&self) -> Vec<Uuid>;

    /// Get all currently pending correlation IDs
    ///
    /// # Returns
    /// Vector of all pending correlation IDs
    async fn pending_ids(&self) -> Vec<Uuid>;

    /// Get statistics about correlation tracking
    ///
    /// # Returns
    /// Current correlation tracking statistics
    async fn get_stats(&self) -> CorrelationStats;

    /// Clear all tracked requests (useful for testing/reset)
    async fn clear_all(&self);
}

/// Async type alias for correlation tracker
pub type AsyncCorrelationTracker = dyn CorrelationTracker;
