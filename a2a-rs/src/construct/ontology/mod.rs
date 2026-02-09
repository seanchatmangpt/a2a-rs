//! Ontology state model (O) for A2A-CONSTRUCT
//!
//! This module implements the ontology state model that represents the complete
//! protocol state of an A2A agent, including agents, tasks, messages, and
//! notification configurations. The state model provides efficient lookups via
//! indices and supports bounded state representation for persistence.
//!
//! # Architecture
//!
//! This is a domain layer module with zero external dependencies. It uses only
//! std library types and types from `domain::core`.
//!
//! # Determinism
//!
//! All collections use `BTreeMap` instead of `HashMap` to ensure deterministic
//! iteration order, which is critical for reproducible state serialization and
//! testing.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::domain::core::{AgentCard, Message, Task, TaskPushNotificationConfig};
use crate::domain::error::A2AError;

/// Maximum number of tasks stored in state (for bounded representation)
pub const DEFAULT_MAX_TASKS: usize = 10_000;

/// Maximum number of messages stored per task (for bounded representation)
pub const DEFAULT_MAX_MESSAGES_PER_TASK: usize = 1_000;

/// Maximum number of agents stored in state (for bounded representation)
pub const DEFAULT_MAX_AGENTS: usize = 1_000;

/// Ontology state model representing the complete protocol state.
///
/// This struct holds all protocol entities and provides indices for efficient
/// lookups. The state is bounded to prevent unbounded growth in production
/// environments.
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::ontology::OntologyState;
///
/// let state = OntologyState::new();
/// assert_eq!(state.task_count(), 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OntologyState {
    /// All tasks indexed by task ID
    tasks: BTreeMap<String, Task>,

    /// Message history for each task, indexed by task ID
    /// Each task maps to an ordered list of messages
    task_messages: BTreeMap<String, Vec<Message>>,

    /// All registered agents indexed by agent name
    agents: BTreeMap<String, AgentCard>,

    /// Push notification configurations indexed by task ID
    notification_configs: BTreeMap<String, TaskPushNotificationConfig>,

    /// Task context index: maps context ID to list of task IDs
    /// Enables efficient lookup of all tasks in a context
    context_to_tasks: BTreeMap<String, Vec<String>>,

    /// Configuration for bounded state
    bounds: StateBounds,
}

/// Configuration for bounded state representation.
///
/// These bounds prevent unbounded state growth by limiting the number of
/// entities stored. When limits are exceeded, implementations should apply
/// eviction policies (e.g., LRU, oldest first).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBounds {
    /// Maximum number of tasks to store
    pub max_tasks: usize,

    /// Maximum number of messages per task
    pub max_messages_per_task: usize,

    /// Maximum number of agents to store
    pub max_agents: usize,
}

impl Default for StateBounds {
    fn default() -> Self {
        Self {
            max_tasks: DEFAULT_MAX_TASKS,
            max_messages_per_task: DEFAULT_MAX_MESSAGES_PER_TASK,
            max_agents: DEFAULT_MAX_AGENTS,
        }
    }
}

impl OntologyState {
    /// Creates a new empty ontology state with default bounds.
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_rs::construct::ontology::OntologyState;
    ///
    /// let state = OntologyState::new();
    /// assert!(state.is_empty());
    /// ```
    pub fn new() -> Self {
        Self::with_bounds(StateBounds::default())
    }

    /// Creates a new empty ontology state with custom bounds.
    ///
    /// # Example
    ///
    /// ```rust
    /// use a2a_rs::construct::ontology::{OntologyState, StateBounds};
    ///
    /// let bounds = StateBounds {
    ///     max_tasks: 5000,
    ///     max_messages_per_task: 500,
    ///     max_agents: 100,
    /// };
    /// let state = OntologyState::with_bounds(bounds);
    /// ```
    pub fn with_bounds(bounds: StateBounds) -> Self {
        Self {
            tasks: BTreeMap::new(),
            task_messages: BTreeMap::new(),
            agents: BTreeMap::new(),
            notification_configs: BTreeMap::new(),
            context_to_tasks: BTreeMap::new(),
            bounds,
        }
    }

    /// Returns true if the state contains no entities.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.agents.is_empty() && self.notification_configs.is_empty()
    }

    /// Returns the current state bounds configuration.
    pub fn bounds(&self) -> &StateBounds {
        &self.bounds
    }

    // ==================== Task Operations ====================

    /// Adds or updates a task in the state.
    ///
    /// If the task already exists, it is updated. If adding a new task would
    /// exceed bounds, returns an error.
    ///
    /// # Errors
    ///
    /// Returns `A2AError::Internal` if adding the task would exceed max_tasks bound.
    pub fn put_task(&mut self, task: Task) -> Result<(), A2AError> {
        let task_id = task.id.clone();
        let context_id = task.context_id.clone();

        // Check bounds only for new tasks
        if !self.tasks.contains_key(&task_id) && self.tasks.len() >= self.bounds.max_tasks {
            return Err(A2AError::Internal(format!(
                "Task limit exceeded: cannot add task {}, max is {}",
                task_id, self.bounds.max_tasks
            )));
        }

        // Update context index
        self.context_to_tasks
            .entry(context_id)
            .or_insert_with(Vec::new)
            .push(task_id.clone());

        // Insert or update task
        self.tasks.insert(task_id, task);
        Ok(())
    }

    /// Retrieves a task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.get(task_id)
    }

    /// Retrieves a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.get_mut(task_id)
    }

    /// Returns all tasks for a given context ID.
    pub fn get_tasks_by_context(&self, context_id: &str) -> Vec<&Task> {
        self.context_to_tasks
            .get(context_id)
            .map(|task_ids| {
                task_ids
                    .iter()
                    .filter_map(|id| self.tasks.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns all tasks in the state.
    ///
    /// The tasks are returned in a deterministic order (sorted by task ID).
    pub fn get_all_tasks(&self) -> Vec<&Task> {
        self.tasks.values().collect()
    }

    /// Returns the total number of tasks in the state.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Removes a task from the state.
    ///
    /// Also removes associated messages and cleans up indices.
    pub fn remove_task(&mut self, task_id: &str) -> Option<Task> {
        // Remove task
        let task = self.tasks.remove(task_id)?;

        // Remove messages
        self.task_messages.remove(task_id);

        // Clean up context index
        if let Some(task_ids) = self.context_to_tasks.get_mut(&task.context_id) {
            task_ids.retain(|id| id != task_id);
            if task_ids.is_empty() {
                self.context_to_tasks.remove(&task.context_id);
            }
        }

        // Remove notification config
        self.notification_configs.remove(task_id);

        Some(task)
    }

    // ==================== Message Operations ====================

    /// Adds a message to a task's message history.
    ///
    /// If adding the message would exceed the per-task message limit, the oldest
    /// message is evicted (FIFO policy).
    ///
    /// # Errors
    ///
    /// Returns `A2AError::TaskNotFound` if the task does not exist.
    pub fn add_message(&mut self, task_id: &str, message: Message) -> Result<(), A2AError> {
        // Verify task exists
        if !self.tasks.contains_key(task_id) {
            return Err(A2AError::TaskNotFound(task_id.to_string()));
        }

        let messages = self
            .task_messages
            .entry(task_id.to_string())
            .or_insert_with(Vec::new);

        // Apply FIFO eviction if at limit
        if messages.len() >= self.bounds.max_messages_per_task {
            messages.remove(0);
        }

        messages.push(message);
        Ok(())
    }

    /// Retrieves all messages for a task.
    ///
    /// Messages are returned in chronological order (oldest first).
    pub fn get_messages(&self, task_id: &str) -> Option<&Vec<Message>> {
        self.task_messages.get(task_id)
    }

    /// Returns the number of messages for a specific task.
    pub fn message_count(&self, task_id: &str) -> usize {
        self.task_messages
            .get(task_id)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    // ==================== Agent Operations ====================

    /// Registers or updates an agent in the state.
    ///
    /// The agent is indexed by its name field. If an agent with the same name
    /// already exists, it will be updated.
    ///
    /// # Errors
    ///
    /// Returns `A2AError::Internal` if adding a new agent would exceed max_agents bound.
    pub fn put_agent(&mut self, agent: AgentCard) -> Result<(), A2AError> {
        let agent_name = agent.name.clone();

        // Check bounds only for new agents
        if !self.agents.contains_key(&agent_name) && self.agents.len() >= self.bounds.max_agents {
            return Err(A2AError::Internal(format!(
                "Agent limit exceeded: cannot add agent {}, max is {}",
                agent_name, self.bounds.max_agents
            )));
        }

        self.agents.insert(agent_name, agent);
        Ok(())
    }

    /// Retrieves an agent by name.
    pub fn get_agent(&self, agent_name: &str) -> Option<&AgentCard> {
        self.agents.get(agent_name)
    }

    /// Returns all registered agents.
    ///
    /// Agents are returned in deterministic order (sorted by agent name).
    pub fn get_all_agents(&self) -> Vec<&AgentCard> {
        self.agents.values().collect()
    }

    /// Returns the total number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Removes an agent from the state by name.
    pub fn remove_agent(&mut self, agent_name: &str) -> Option<AgentCard> {
        self.agents.remove(agent_name)
    }

    // ==================== Notification Operations ====================

    /// Adds or updates a push notification configuration for a task.
    ///
    /// # Errors
    ///
    /// Returns `A2AError::TaskNotFound` if the task does not exist.
    pub fn put_notification_config(
        &mut self,
        task_id: &str,
        config: TaskPushNotificationConfig,
    ) -> Result<(), A2AError> {
        // Verify task exists
        if !self.tasks.contains_key(task_id) {
            return Err(A2AError::TaskNotFound(task_id.to_string()));
        }

        self.notification_configs
            .insert(task_id.to_string(), config);
        Ok(())
    }

    /// Retrieves the push notification configuration for a task.
    pub fn get_notification_config(&self, task_id: &str) -> Option<&TaskPushNotificationConfig> {
        self.notification_configs.get(task_id)
    }

    /// Removes the push notification configuration for a task.
    pub fn remove_notification_config(
        &mut self,
        task_id: &str,
    ) -> Option<TaskPushNotificationConfig> {
        self.notification_configs.remove(task_id)
    }

    /// Returns all notification configurations.
    ///
    /// Configurations are returned in deterministic order (sorted by task ID).
    pub fn get_all_notification_configs(&self) -> Vec<(&String, &TaskPushNotificationConfig)> {
        self.notification_configs.iter().collect()
    }

    /// Returns the total number of notification configurations.
    pub fn notification_config_count(&self) -> usize {
        self.notification_configs.len()
    }

    // ==================== State Management ====================

    /// Clears all state, returning to an empty ontology.
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.task_messages.clear();
        self.agents.clear();
        self.notification_configs.clear();
        self.context_to_tasks.clear();
    }

    /// Returns statistics about the current state.
    pub fn stats(&self) -> StateStats {
        StateStats {
            task_count: self.tasks.len(),
            agent_count: self.agents.len(),
            notification_config_count: self.notification_configs.len(),
            context_count: self.context_to_tasks.len(),
            total_messages: self.task_messages.values().map(|v| v.len()).sum(),
        }
    }
}

impl Default for OntologyState {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the ontology state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateStats {
    pub task_count: usize,
    pub agent_count: usize,
    pub notification_config_count: usize,
    pub context_count: usize,
    pub total_messages: usize,
}

/// Storage interface for persisting ontology state.
///
/// This trait defines the interface for persisting and loading the ontology
/// state. Implementations can use databases, files, or other storage backends.
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct FileStorage {
///     path: PathBuf,
/// }
///
/// impl OntologyStorage for FileStorage {
///     fn save(&self, state: &OntologyState) -> Result<(), A2AError> {
///         let json = serde_json::to_string(state)?;
///         std::fs::write(&self.path, json)?;
///         Ok(())
///     }
///
///     fn load(&self) -> Result<OntologyState, A2AError> {
///         let json = std::fs::read_to_string(&self.path)?;
///         let state = serde_json::from_str(&json)?;
///         Ok(state)
///     }
/// }
/// ```
pub trait OntologyStorage {
    /// Persists the ontology state to storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be persisted.
    fn save(&self, state: &OntologyState) -> Result<(), A2AError>;

    /// Loads the ontology state from storage.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be loaded.
    fn load(&self) -> Result<OntologyState, A2AError>;

    /// Checks if stored state exists.
    fn exists(&self) -> bool;

    /// Deletes the stored state.
    ///
    /// # Errors
    ///
    /// Returns an error if the state cannot be deleted.
    fn delete(&self) -> Result<(), A2AError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::core::{TaskState, TaskStatus};

    #[test]
    fn test_new_state_is_empty() {
        let state = OntologyState::new();
        assert!(state.is_empty());
        assert_eq!(state.task_count(), 0);
        assert_eq!(state.agent_count(), 0);
    }

    #[test]
    fn test_put_and_get_task() {
        let mut state = OntologyState::new();
        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        state.put_task(task.clone()).unwrap();
        assert_eq!(state.task_count(), 1);

        let retrieved = state.get_task("task-1").unwrap();
        assert_eq!(retrieved.id, "task-1");
    }

    #[test]
    fn test_task_context_index() {
        let mut state = OntologyState::new();

        let task1 = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        let task2 = Task::builder()
            .id("task-2".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        state.put_task(task1).unwrap();
        state.put_task(task2).unwrap();

        let tasks = state.get_tasks_by_context("ctx-1");
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_add_message_enforces_bounds() {
        let bounds = StateBounds {
            max_tasks: 100,
            max_messages_per_task: 3,
            max_agents: 100,
        };
        let mut state = OntologyState::with_bounds(bounds);

        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        state.put_task(task).unwrap();

        // Add 4 messages (exceeds limit of 3)
        for i in 0..4 {
            let msg = Message::user_text(format!("Message {}", i), format!("msg-{}", i));
            state.add_message("task-1", msg).unwrap();
        }

        // Should only have 3 messages (oldest evicted)
        assert_eq!(state.message_count("task-1"), 3);
        let messages = state.get_messages("task-1").unwrap();
        assert_eq!(messages[0].message_id, "msg-1"); // msg-0 was evicted
    }

    #[test]
    fn test_remove_task_cleans_up_indices() {
        let mut state = OntologyState::new();

        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        state.put_task(task).unwrap();

        let msg = Message::user_text("Test".to_string(), "msg-1".to_string());
        state.add_message("task-1", msg).unwrap();

        let removed = state.remove_task("task-1");
        assert!(removed.is_some());
        assert_eq!(state.task_count(), 0);
        assert_eq!(state.message_count("task-1"), 0);
        assert!(state.get_tasks_by_context("ctx-1").is_empty());
    }

    #[test]
    fn test_deterministic_iteration_order() {
        let mut state = OntologyState::new();

        // Insert tasks in non-alphabetical order
        for id in ["task-c", "task-a", "task-b"] {
            let task = Task::builder()
                .id(id.to_string())
                .context_id("ctx-1".to_string())
                .status(TaskStatus::default())
                .build();
            state.put_task(task).unwrap();
        }

        // Should return in alphabetical order (BTreeMap guarantees this)
        let tasks = state.get_all_tasks();
        let ids: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["task-a", "task-b", "task-c"]);
    }
}
