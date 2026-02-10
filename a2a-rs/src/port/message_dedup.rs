//! Port for message deduplication
//!
//! Defines the contract for detecting and filtering duplicate messages.

use async_trait::async_trait;

use crate::domain::{A2AError, Message};

/// Result of a deduplication check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupResult {
    /// Message is unique (not seen before)
    Unique,
    /// Message is a duplicate
    Duplicate,
}

/// Port for message deduplication
///
/// Implementations should provide fast detection of duplicate messages
/// within a time window. This is useful for preventing duplicate processing
/// in distributed systems or when dealing with unreliable networks.
#[async_trait]
pub trait MessageDeduplicator: Send + Sync {
    /// Check if a message is a duplicate and record it if not
    ///
    /// # Arguments
    /// * `message` - The message to check
    ///
    /// # Returns
    /// * `Ok(DedupResult::Unique)` - Message has not been seen before (now recorded)
    /// * `Ok(DedupResult::Duplicate)` - Message is a duplicate
    /// * `Err(_)` - Error during deduplication check
    async fn check_and_record(&self, message: &Message) -> Result<DedupResult, A2AError>;

    /// Clear all deduplication state (useful for testing)
    async fn clear(&self) -> Result<(), A2AError>;

    /// Get statistics about the deduplication state
    async fn stats(&self) -> Result<DedupStats, A2AError>;
}

/// Statistics about deduplication state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupStats {
    /// Approximate number of messages tracked
    pub tracked_count: usize,
    /// Number of duplicate detections since last clear
    pub duplicate_count: u64,
    /// Number of unique messages seen since last clear
    pub unique_count: u64,
}
