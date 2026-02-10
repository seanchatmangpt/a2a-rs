//! Batch processing port definitions

#[cfg(feature = "server")]
use async_trait::async_trait;

use crate::domain::{A2AError, Message, Task};

/// Result of processing a single message in a batch
#[derive(Debug, Clone)]
pub struct BatchItemResult {
    /// The message ID that was processed
    pub message_id: String,
    /// The task ID associated with this message
    pub task_id: String,
    /// The result of processing this message
    pub result: Result<Task, A2AError>,
}

/// Result of processing an entire batch
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Individual results for each message in the batch
    pub items: Vec<BatchItemResult>,
    /// Number of successful items
    pub success_count: usize,
    /// Number of failed items
    pub failure_count: usize,
}

impl BatchResult {
    /// Create a new batch result from individual item results
    pub fn from_items(items: Vec<BatchItemResult>) -> Self {
        let success_count = items.iter().filter(|i| i.result.is_ok()).count();
        let failure_count = items.len() - success_count;
        Self {
            items,
            success_count,
            failure_count,
        }
    }

    /// Check if all items succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failure_count == 0
    }

    /// Check if any items failed
    pub fn has_failures(&self) -> bool {
        self.failure_count > 0
    }

    /// Get successful items
    pub fn successes(&self) -> impl Iterator<Item = &BatchItemResult> {
        self.items.iter().filter(|i| i.result.is_ok())
    }

    /// Get failed items
    pub fn failures(&self) -> impl Iterator<Item = &BatchItemResult> {
        self.items.iter().filter(|i| i.result.is_err())
    }
}

/// Configuration for batch processing
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of messages in a batch
    pub max_batch_size: usize,
    /// Maximum time to wait before processing a partial batch (in milliseconds)
    pub max_batch_delay_ms: u64,
    /// Maximum number of concurrent batch processing tasks
    pub max_concurrent_batches: usize,
    /// Whether to commit the entire batch or individual items
    pub atomic_batches: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            max_batch_delay_ms: 1000,
            max_concurrent_batches: 4,
            atomic_batches: false,
        }
    }
}

#[cfg(feature = "server")]
#[async_trait]
/// A trait for batch processing of messages
pub trait BatchProcessor: Send + Sync {
    /// Process a batch of messages
    ///
    /// # Arguments
    /// * `messages` - Vector of (task_id, message, session_id) tuples to process
    ///
    /// # Returns
    /// A BatchResult containing individual results for each message
    async fn process_batch(
        &self,
        messages: Vec<(String, Message, Option<String>)>,
    ) -> Result<BatchResult, A2AError>;

    /// Handle commit for a successful batch
    ///
    /// This is called after all messages in a batch have been processed successfully
    /// when atomic_batches is enabled. Implementations can use this to commit
    /// database transactions or perform other finalization steps.
    async fn commit_batch(&self, _batch_id: &str) -> Result<(), A2AError> {
        // Default implementation does nothing
        Ok(())
    }

    /// Handle rollback for a failed batch
    ///
    /// This is called when a batch processing fails and atomic_batches is enabled.
    /// Implementations can use this to rollback database transactions or undo
    /// any partial changes.
    async fn rollback_batch(&self, _batch_id: &str) -> Result<(), A2AError> {
        // Default implementation does nothing
        Ok(())
    }

    /// Get the batch configuration
    fn get_config(&self) -> &BatchConfig;

    /// Validate a batch before processing
    ///
    /// This allows implementations to reject batches that don't meet
    /// specific criteria (e.g., size limits, rate limits, etc.)
    async fn validate_batch(
        &self,
        messages: &[(String, Message, Option<String>)],
    ) -> Result<(), A2AError> {
        // Default implementation - basic size validation
        let config = self.get_config();
        if messages.len() > config.max_batch_size {
            return Err(A2AError::InvalidParams(format!(
                "Batch size {} exceeds maximum of {}",
                messages.len(),
                config.max_batch_size
            )));
        }
        Ok(())
    }
}
