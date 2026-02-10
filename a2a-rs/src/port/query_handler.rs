//! Query handler port for CQRS read operations.
//!
//! This port defines the interface for executing queries against read-optimized views.
//! Queries are read-only operations that return data without modifying state.

use async_trait::async_trait;

use crate::domain::{
    error::A2AError, AgentStatsView, GetAgentStats, GetMessagesByAgent, GetTasksByStatus,
    MessageListView, TaskListView,
};

/// Trait for handling queries against read models.
///
/// This trait provides async methods for executing queries against
/// read-optimized projections and materialized views. Implementations
/// should focus on performance and denormalized data access.
///
/// # Example Implementation
/// ```rust,no_run
/// use async_trait::async_trait;
/// use a2a_rs::{QueryHandler, GetTasksByStatus, TaskListView, A2AError};
///
/// struct MyQueryHandler;
///
/// #[async_trait]
/// impl QueryHandler for MyQueryHandler {
///     async fn get_tasks_by_status(&self, query: GetTasksByStatus) -> Result<TaskListView, A2AError> {
///         // Implementation
///         todo!()
///     }
///
///     async fn get_messages_by_agent(&self, query: a2a_rs::GetMessagesByAgent) -> Result<a2a_rs::MessageListView, A2AError> {
///         todo!()
///     }
///
///     async fn get_agent_stats(&self, query: a2a_rs::GetAgentStats) -> Result<a2a_rs::AgentStatsView, A2AError> {
///         todo!()
///     }
/// }
/// ```
#[async_trait]
pub trait QueryHandler: Send + Sync {
    /// Execute a query to retrieve tasks by status.
    ///
    /// Returns a list of tasks matching the specified status, with optional
    /// pagination and context filtering.
    ///
    /// # Arguments
    /// * `query` - Query parameters including status, pagination, and filters
    ///
    /// # Returns
    /// A `TaskListView` containing the matching tasks and pagination info
    async fn get_tasks_by_status(&self, query: GetTasksByStatus)
        -> Result<TaskListView, A2AError>;

    /// Execute a query to retrieve messages by agent.
    ///
    /// Returns a list of messages sent to or from the specified agent,
    /// with optional time range filtering and pagination.
    ///
    /// # Arguments
    /// * `query` - Query parameters including agent ID, time range, and pagination
    ///
    /// # Returns
    /// A `MessageListView` containing the matching messages and pagination info
    async fn get_messages_by_agent(
        &self,
        query: GetMessagesByAgent,
    ) -> Result<MessageListView, A2AError>;

    /// Execute a query to retrieve agent statistics.
    ///
    /// Returns aggregated statistics about an agent's activity,
    /// including task counts by status and message counts.
    ///
    /// # Arguments
    /// * `query` - Query parameters including agent ID and optional time range
    ///
    /// # Returns
    /// An `AgentStatsView` containing the agent's statistics
    async fn get_agent_stats(&self, query: GetAgentStats) -> Result<AgentStatsView, A2AError>;
}
