//! Message storage port definitions
//!
//! Defines the interface for persistent message storage with support for
//! filtering, full-text search, and pagination.

#[cfg(feature = "server")]
use async_trait::async_trait;

use crate::domain::{A2AError, Message, Role};

/// Parameters for querying messages
#[derive(Debug, Clone)]
pub struct MessageQuery {
    /// Filter by sender ID (agent or user ID)
    pub sender_id: Option<String>,
    /// Filter by message type (text, file, data)
    pub message_type: Option<String>,
    /// Filter by role (user or agent)
    pub role: Option<Role>,
    /// Filter by task ID
    pub task_id: Option<String>,
    /// Filter by context ID
    pub context_id: Option<String>,
    /// Full-text search query
    pub search_query: Option<String>,
    /// Timestamp filter - messages after this time (Unix milliseconds)
    pub after_timestamp: Option<i64>,
    /// Timestamp filter - messages before this time (Unix milliseconds)
    pub before_timestamp: Option<i64>,
    /// Maximum number of results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl Default for MessageQuery {
    fn default() -> Self {
        Self {
            sender_id: None,
            message_type: None,
            role: None,
            task_id: None,
            context_id: None,
            search_query: None,
            after_timestamp: None,
            before_timestamp: None,
            limit: Some(50),
            offset: None,
        }
    }
}

/// Result of a paginated message query
#[derive(Debug, Clone)]
pub struct MessageQueryResult {
    /// Messages matching the query
    pub messages: Vec<Message>,
    /// Total number of messages matching the query (ignoring limit/offset)
    pub total_count: i64,
    /// Whether there are more results available
    pub has_more: bool,
}

#[cfg(feature = "server")]
#[async_trait]
/// A trait for persistent message storage
///
/// Provides operations for storing, retrieving, and searching messages
/// with support for filtering and pagination.
pub trait MessageStore: Send + Sync {
    /// Store a new message
    ///
    /// # Arguments
    /// * `message` - The message to store
    ///
    /// # Returns
    /// The stored message (may include database-generated fields)
    async fn store_message(&self, message: &Message) -> Result<Message, A2AError>;

    /// Retrieve a message by ID
    ///
    /// # Arguments
    /// * `message_id` - The unique identifier of the message
    ///
    /// # Returns
    /// The message if found, or an error if not found
    async fn get_message(&self, message_id: &str) -> Result<Message, A2AError>;

    /// Query messages with optional filtering and pagination
    ///
    /// # Arguments
    /// * `query` - Query parameters for filtering and pagination
    ///
    /// # Returns
    /// A result containing matching messages and pagination metadata
    async fn query_messages(&self, query: &MessageQuery) -> Result<MessageQueryResult, A2AError>;

    /// Delete a message by ID
    ///
    /// # Arguments
    /// * `message_id` - The unique identifier of the message to delete
    ///
    /// # Returns
    /// Ok(()) if successful, or an error if the message was not found
    async fn delete_message(&self, message_id: &str) -> Result<(), A2AError>;

    /// Delete all messages for a specific task
    ///
    /// # Arguments
    /// * `task_id` - The task ID whose messages should be deleted
    ///
    /// # Returns
    /// The number of messages deleted
    async fn delete_messages_by_task(&self, task_id: &str) -> Result<u64, A2AError>;

    /// Count messages matching a query
    ///
    /// # Arguments
    /// * `query` - Query parameters for filtering
    ///
    /// # Returns
    /// The number of messages matching the query
    async fn count_messages(&self, query: &MessageQuery) -> Result<i64, A2AError>;
}
