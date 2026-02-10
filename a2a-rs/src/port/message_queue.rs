//! Message queue port definition
//!
//! Defines the interface for asynchronous message queuing with priority support
//! and backpressure handling.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::{Message, error::A2AError};

/// Priority level for queued messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Queue metrics for monitoring and observability
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueMetrics {
    /// Current number of messages in high priority queue
    pub high_priority_depth: usize,
    /// Current number of messages in normal priority queue
    pub normal_priority_depth: usize,
    /// Current number of messages in low priority queue
    pub low_priority_depth: usize,
    /// Total messages enqueued since start
    pub total_enqueued: u64,
    /// Total messages dequeued since start
    pub total_dequeued: u64,
    /// Total messages dropped due to backpressure
    pub total_dropped: u64,
    /// Current throughput in messages per second (calculated)
    pub throughput_mps: f64,
}

impl QueueMetrics {
    /// Get total queue depth across all priorities
    pub fn total_depth(&self) -> usize {
        self.high_priority_depth + self.normal_priority_depth + self.low_priority_depth
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.total_depth() == 0
    }
}

/// Message queue interface for asynchronous message handling with priority support
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// Enqueue a message with specified priority
    ///
    /// Returns an error if the queue is full (backpressure). Callers should
    /// handle backpressure by retrying with exponential backoff or dropping
    /// the message after a timeout.
    async fn enqueue(&self, message: Message, priority: Priority) -> Result<(), A2AError>;

    /// Dequeue the next message based on priority
    ///
    /// Returns the highest priority message available. Returns None if the
    /// queue is empty. This operation blocks until a message is available
    /// or a timeout occurs.
    async fn dequeue(&self) -> Result<Option<Message>, A2AError>;

    /// Dequeue with timeout
    ///
    /// Returns None if no message is available within the specified duration
    async fn dequeue_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Option<Message>, A2AError>;

    /// Peek at the next message without removing it
    ///
    /// Returns a reference to the highest priority message without dequeuing it
    async fn peek(&self) -> Result<Option<Message>, A2AError>;

    /// Get current queue metrics
    async fn metrics(&self) -> Result<QueueMetrics, A2AError>;

    /// Get the current queue depth
    async fn depth(&self) -> Result<usize, A2AError> {
        Ok(self.metrics().await?.total_depth())
    }

    /// Check if the queue is empty
    async fn is_empty(&self) -> Result<bool, A2AError> {
        Ok(self.metrics().await?.is_empty())
    }

    /// Get the configured capacity of the queue
    async fn capacity(&self) -> Result<usize, A2AError>;
}
