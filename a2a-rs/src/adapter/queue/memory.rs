//! In-memory message queue implementation with backpressure support
//!
//! Provides a high-performance async message queue using tokio bounded channels
//! with three priority levels (high, normal, low) and comprehensive metrics tracking.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

#[cfg(feature = "tracing")]
use tracing::{debug, warn};

use crate::domain::{error::A2AError, Message};
use crate::port::message_queue::{MessageQueue, Priority, QueueMetrics};

/// Internal queue statistics for metrics calculation
#[derive(Debug, Clone)]
struct QueueStats {
    total_enqueued: u64,
    total_dequeued: u64,
    total_dropped: u64,
    last_throughput_calc: Instant,
    throughput_window_dequeued: u64,
}

impl Default for QueueStats {
    fn default() -> Self {
        Self {
            total_enqueued: 0,
            total_dequeued: 0,
            total_dropped: 0,
            last_throughput_calc: Instant::now(),
            throughput_window_dequeued: 0,
        }
    }
}

/// In-memory message queue with priority support and backpressure handling
///
/// Uses three bounded channels (one per priority level) to provide
/// backpressure when queues are full. Messages are dequeued in priority
/// order: high -> normal -> low.
///
/// # Example
/// ```no_run
/// use a2a_rs::adapter::queue::InMemoryMessageQueue;
/// use a2a_rs::port::message_queue::{MessageQueue, Priority};
/// use a2a_rs::Message;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let queue = InMemoryMessageQueue::new(100);
///
/// // Enqueue a message
/// let msg = Message::builder()
///     .role(a2a_rs::Role::User)
///     .message_id("msg-1".to_string())
///     .build();
/// queue.enqueue(msg, Priority::High).await?;
///
/// // Dequeue the message
/// if let Some(msg) = queue.dequeue().await? {
///     println!("Received message: {}", msg.message_id);
/// }
/// # Ok(())
/// # }
/// ```
pub struct InMemoryMessageQueue {
    high_tx: mpsc::Sender<Message>,
    high_rx: Arc<RwLock<mpsc::Receiver<Message>>>,
    normal_tx: mpsc::Sender<Message>,
    normal_rx: Arc<RwLock<mpsc::Receiver<Message>>>,
    low_tx: mpsc::Sender<Message>,
    low_rx: Arc<RwLock<mpsc::Receiver<Message>>>,
    stats: Arc<RwLock<QueueStats>>,
    capacity: usize,
}

impl InMemoryMessageQueue {
    /// Create a new in-memory message queue with the specified capacity per priority level
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of messages per priority queue (total capacity = 3 * capacity)
    pub fn new(capacity: usize) -> Self {
        let (high_tx, high_rx) = mpsc::channel(capacity);
        let (normal_tx, normal_rx) = mpsc::channel(capacity);
        let (low_tx, low_rx) = mpsc::channel(capacity);

        Self {
            high_tx,
            high_rx: Arc::new(RwLock::new(high_rx)),
            normal_tx,
            normal_rx: Arc::new(RwLock::new(normal_rx)),
            low_tx,
            low_rx: Arc::new(RwLock::new(low_rx)),
            stats: Arc::new(RwLock::new(QueueStats::default())),
            capacity,
        }
    }

    /// Calculate current throughput in messages per second
    fn calculate_throughput(stats: &mut QueueStats) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(stats.last_throughput_calc);
        let elapsed_secs = elapsed.as_secs_f64();

        if elapsed_secs > 0.0 {
            let throughput = stats.throughput_window_dequeued as f64 / elapsed_secs;

            // Reset window every second
            if elapsed_secs >= 1.0 {
                stats.throughput_window_dequeued = 0;
                stats.last_throughput_calc = now;
            }

            throughput
        } else {
            0.0
        }
    }

    /// Try to dequeue from a specific priority channel without blocking
    async fn try_dequeue_from(
        rx: &Arc<RwLock<mpsc::Receiver<Message>>>,
    ) -> Result<Option<Message>, A2AError> {
        let mut receiver = rx.write().await;
        Ok(receiver.try_recv().ok())
    }

    /// Dequeue from a specific priority channel with blocking
    async fn dequeue_from(
        rx: &Arc<RwLock<mpsc::Receiver<Message>>>,
    ) -> Result<Option<Message>, A2AError> {
        let mut receiver = rx.write().await;
        Ok(receiver.recv().await)
    }

    /// Dequeue from a specific priority channel with timeout
    async fn dequeue_from_timeout(
        rx: &Arc<RwLock<mpsc::Receiver<Message>>>,
        duration: Duration,
    ) -> Result<Option<Message>, A2AError> {
        let mut receiver = rx.write().await;
        match timeout(duration, receiver.recv()).await {
            Ok(Some(msg)) => Ok(Some(msg)),
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // Timeout
        }
    }

    /// Peek at a specific priority channel without removing
    async fn peek_from(
        rx: &Arc<RwLock<mpsc::Receiver<Message>>>,
    ) -> Result<Option<Message>, A2AError> {
        let receiver = rx.read().await;
        // Note: mpsc::Receiver doesn't have a native peek, so we simulate
        // by checking if there are messages available
        Ok(if !receiver.is_empty() {
            // We can't actually peek without consuming, so return None
            // A proper implementation would need a different data structure
            None
        } else {
            None
        })
    }
}

#[async_trait]
impl MessageQueue for InMemoryMessageQueue {
    async fn enqueue(&self, message: Message, priority: Priority) -> Result<(), A2AError> {
        let sender = match priority {
            Priority::High => &self.high_tx,
            Priority::Normal => &self.normal_tx,
            Priority::Low => &self.low_tx,
        };

        match sender.try_send(message.clone()) {
            Ok(_) => {
                let mut stats = self.stats.write().await;
                stats.total_enqueued += 1;

                #[cfg(feature = "tracing")]
                debug!(
                    priority = ?priority,
                    message_id = %message.message_id,
                    total_enqueued = stats.total_enqueued,
                    "Message enqueued"
                );

                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                let mut stats = self.stats.write().await;
                stats.total_dropped += 1;

                #[cfg(feature = "tracing")]
                warn!(
                    priority = ?priority,
                    message_id = %message.message_id,
                    total_dropped = stats.total_dropped,
                    "Queue full, message dropped (backpressure)"
                );

                Err(A2AError::Internal(format!(
                    "Queue full for priority {:?} (backpressure)",
                    priority
                )))
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(A2AError::Internal("Queue closed".to_string()))
            }
        }
    }

    async fn dequeue(&self) -> Result<Option<Message>, A2AError> {
        // Try high priority first (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.high_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;

            #[cfg(feature = "tracing")]
            debug!(
                priority = ?Priority::High,
                message_id = %msg.message_id,
                total_dequeued = stats.total_dequeued,
                "Message dequeued"
            );

            return Ok(Some(msg));
        }

        // Try normal priority (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.normal_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;

            #[cfg(feature = "tracing")]
            debug!(
                priority = ?Priority::Normal,
                message_id = %msg.message_id,
                total_dequeued = stats.total_dequeued,
                "Message dequeued"
            );

            return Ok(Some(msg));
        }

        // Try low priority (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.low_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;

            #[cfg(feature = "tracing")]
            debug!(
                priority = ?Priority::Low,
                message_id = %msg.message_id,
                total_dequeued = stats.total_dequeued,
                "Message dequeued"
            );

            return Ok(Some(msg));
        }

        // All queues empty, block on highest priority available
        // Use tokio::select! to wait on all channels simultaneously
        tokio::select! {
            msg = Self::dequeue_from(&self.high_rx) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
            msg = Self::dequeue_from(&self.normal_rx) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
            msg = Self::dequeue_from(&self.low_rx) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
        }

        Ok(None)
    }

    async fn dequeue_timeout(
        &self,
        duration: Duration,
    ) -> Result<Option<Message>, A2AError> {
        // Try high priority first (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.high_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;
            return Ok(Some(msg));
        }

        // Try normal priority (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.normal_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;
            return Ok(Some(msg));
        }

        // Try low priority (non-blocking)
        if let Some(msg) = Self::try_dequeue_from(&self.low_rx).await? {
            let mut stats = self.stats.write().await;
            stats.total_dequeued += 1;
            stats.throughput_window_dequeued += 1;
            return Ok(Some(msg));
        }

        // All queues empty, wait with timeout
        tokio::select! {
            msg = Self::dequeue_from_timeout(&self.high_rx, duration) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
            msg = Self::dequeue_from_timeout(&self.normal_rx, duration) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
            msg = Self::dequeue_from_timeout(&self.low_rx, duration) => {
                if let Some(m) = msg? {
                    let mut stats = self.stats.write().await;
                    stats.total_dequeued += 1;
                    stats.throughput_window_dequeued += 1;
                    return Ok(Some(m));
                }
            }
        }

        Ok(None)
    }

    async fn peek(&self) -> Result<Option<Message>, A2AError> {
        // Try to peek at each priority level in order
        if let Some(msg) = Self::peek_from(&self.high_rx).await? {
            return Ok(Some(msg));
        }

        if let Some(msg) = Self::peek_from(&self.normal_rx).await? {
            return Ok(Some(msg));
        }

        if let Some(msg) = Self::peek_from(&self.low_rx).await? {
            return Ok(Some(msg));
        }

        Ok(None)
    }

    async fn metrics(&self) -> Result<QueueMetrics, A2AError> {
        let high_depth = self.high_tx.max_capacity() - self.high_tx.capacity();
        let normal_depth = self.normal_tx.max_capacity() - self.normal_tx.capacity();
        let low_depth = self.low_tx.max_capacity() - self.low_tx.capacity();

        let mut stats = self.stats.write().await;
        let throughput = Self::calculate_throughput(&mut stats);

        Ok(QueueMetrics {
            high_priority_depth: high_depth,
            normal_priority_depth: normal_depth,
            low_priority_depth: low_depth,
            total_enqueued: stats.total_enqueued,
            total_dequeued: stats.total_dequeued,
            total_dropped: stats.total_dropped,
            throughput_mps: throughput,
        })
    }

    async fn capacity(&self) -> Result<usize, A2AError> {
        Ok(self.capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Role;

    fn create_test_message(id: &str) -> Message {
        Message::builder()
            .role(Role::User)
            .message_id(id.to_string())
            .build()
    }

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = InMemoryMessageQueue::new(10);
        let msg = create_test_message("test-1");

        queue.enqueue(msg.clone(), Priority::Normal).await.unwrap();
        let dequeued = queue.dequeue().await.unwrap().unwrap();

        assert_eq!(dequeued.message_id, msg.message_id);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = InMemoryMessageQueue::new(10);

        // Enqueue in reverse priority order
        let low_msg = create_test_message("low");
        let normal_msg = create_test_message("normal");
        let high_msg = create_test_message("high");

        queue.enqueue(low_msg, Priority::Low).await.unwrap();
        queue.enqueue(normal_msg, Priority::Normal).await.unwrap();
        queue.enqueue(high_msg, Priority::High).await.unwrap();

        // Should dequeue in priority order: high, normal, low
        assert_eq!(
            queue.dequeue().await.unwrap().unwrap().message_id,
            "high"
        );
        assert_eq!(
            queue.dequeue().await.unwrap().unwrap().message_id,
            "normal"
        );
        assert_eq!(
            queue.dequeue().await.unwrap().unwrap().message_id,
            "low"
        );
    }

    #[tokio::test]
    async fn test_backpressure() {
        let queue = InMemoryMessageQueue::new(2);

        // Fill the queue
        queue
            .enqueue(create_test_message("1"), Priority::Normal)
            .await
            .unwrap();
        queue
            .enqueue(create_test_message("2"), Priority::Normal)
            .await
            .unwrap();

        // Third message should trigger backpressure
        let result = queue
            .enqueue(create_test_message("3"), Priority::Normal)
            .await;
        assert!(result.is_err());

        // Verify dropped count
        let metrics = queue.metrics().await.unwrap();
        assert_eq!(metrics.total_dropped, 1);
    }

    #[tokio::test]
    async fn test_metrics() {
        let queue = InMemoryMessageQueue::new(10);

        queue
            .enqueue(create_test_message("1"), Priority::High)
            .await
            .unwrap();
        queue
            .enqueue(create_test_message("2"), Priority::Normal)
            .await
            .unwrap();

        let metrics = queue.metrics().await.unwrap();
        assert_eq!(metrics.total_enqueued, 2);
        assert_eq!(metrics.high_priority_depth, 1);
        assert_eq!(metrics.normal_priority_depth, 1);
        assert_eq!(metrics.total_depth(), 2);

        queue.dequeue().await.unwrap();

        let metrics = queue.metrics().await.unwrap();
        assert_eq!(metrics.total_dequeued, 1);
    }

    #[tokio::test]
    async fn test_dequeue_timeout() {
        let queue = InMemoryMessageQueue::new(10);

        // Empty queue should timeout
        let result = queue
            .dequeue_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        assert!(result.is_none());

        // With message should succeed immediately
        queue
            .enqueue(create_test_message("1"), Priority::Normal)
            .await
            .unwrap();
        let result = queue
            .dequeue_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_is_empty() {
        let queue = InMemoryMessageQueue::new(10);

        assert!(queue.is_empty().await.unwrap());

        queue
            .enqueue(create_test_message("1"), Priority::Normal)
            .await
            .unwrap();
        assert!(!queue.is_empty().await.unwrap());

        queue.dequeue().await.unwrap();
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_capacity() {
        let queue = InMemoryMessageQueue::new(42);
        assert_eq!(queue.capacity().await.unwrap(), 42);
    }
}
