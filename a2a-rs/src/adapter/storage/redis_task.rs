//! Redis-based task storage adapter
//!
//! This module provides a persistent task storage implementation using Redis
//! with support for task queues, TTL-based expiration, and atomic operations.

#[cfg(feature = "redis")]
use async_trait::async_trait;
#[cfg(feature = "redis")]
use redis::AsyncCommands;
#[cfg(feature = "redis")]
use serde_json;

#[cfg(feature = "redis")]
use crate::domain::{
    A2AError, DeleteTaskPushNotificationConfigParams, GetTaskPushNotificationConfigParams,
    ListTaskPushNotificationConfigParams, ListTasksParams, ListTasksResult, Message, Task,
    TaskPushNotificationConfig, TaskState, TaskStatus,
};
#[cfg(feature = "redis")]
use crate::port::AsyncTaskManager;

#[cfg(feature = "redis")]
/// Redis task storage adapter
///
/// Provides persistent task storage using Redis with support for:
/// - Sorted Sets for task queues (ordered by timestamp/priority)
/// - Hash storage for task data
/// - TTL for completed tasks (configurable, default 24 hours)
/// - Atomic operations using pipelines and transactions
/// - Task history tracking
/// - Push notification configuration storage
///
/// # Redis Data Structure
///
/// - `task:{task_id}` - Hash containing task data
/// - `tasks:all` - Sorted Set of all tasks (score = timestamp)
/// - `tasks:context:{context_id}` - Sorted Set of tasks per context
/// - `tasks:status:{state}` - Set of task IDs by status
/// - `task:{task_id}:history` - List of message history
/// - `task:{task_id}:push_configs:{config_id}` - Hash of push notification config
/// - `task:{task_id}:push_configs` - Set of config IDs for a task
pub struct RedisTaskStorage {
    /// Redis connection manager
    connection_manager: redis::aio::ConnectionManager,
    /// TTL for completed tasks in seconds (default: 86400 = 24 hours)
    completed_task_ttl: u64,
}

#[cfg(feature = "redis")]
impl RedisTaskStorage {
    /// Create a new Redis task storage with the given connection string
    ///
    /// # Arguments
    /// * `redis_url` - Redis connection string (e.g., "redis://127.0.0.1:6379")
    ///
    /// # Example
    /// ```no_run
    /// # use a2a_rs::adapter::storage::RedisTaskStorage;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let storage = RedisTaskStorage::new("redis://127.0.0.1:6379").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(redis_url: &str) -> Result<Self, A2AError> {
        let client = redis::Client::open(redis_url).map_err(|e| {
            A2AError::DatabaseError(format!("Failed to create Redis client: {}", e))
        })?;

        let connection_manager = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self {
            connection_manager,
            completed_task_ttl: 86400, // 24 hours default
        })
    }

    /// Create storage with custom TTL for completed tasks
    pub async fn with_ttl(redis_url: &str, completed_task_ttl: u64) -> Result<Self, A2AError> {
        let mut storage = Self::new(redis_url).await?;
        storage.completed_task_ttl = completed_task_ttl;
        Ok(storage)
    }

    /// Get a cloned connection for operations
    fn get_connection(&self) -> redis::aio::ConnectionManager {
        self.connection_manager.clone()
    }

    /// Convert task state to string
    fn task_state_to_string(state: &TaskState) -> &'static str {
        match state {
            TaskState::Submitted => "submitted",
            TaskState::Working => "working",
            TaskState::InputRequired => "input-required",
            TaskState::Completed => "completed",
            TaskState::Canceled => "canceled",
            TaskState::Failed => "failed",
            TaskState::Rejected => "rejected",
            TaskState::AuthRequired => "auth-required",
            TaskState::Unknown => "unknown",
        }
    }

    /// Parse task state from string
    fn parse_task_state(state_str: &str) -> TaskState {
        match state_str {
            "submitted" => TaskState::Submitted,
            "working" => TaskState::Working,
            "input-required" => TaskState::InputRequired,
            "completed" => TaskState::Completed,
            "canceled" => TaskState::Canceled,
            "failed" => TaskState::Failed,
            "rejected" => TaskState::Rejected,
            "auth-required" => TaskState::AuthRequired,
            _ => TaskState::Unknown,
        }
    }

    /// Check if a state is terminal (task is finished)
    fn is_terminal_state(state: &TaskState) -> bool {
        matches!(
            state,
            TaskState::Completed | TaskState::Canceled | TaskState::Failed | TaskState::Rejected
        )
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> i64 {
        chrono::Utc::now().timestamp()
    }

    /// Store task in Redis using atomic pipeline
    async fn store_task(&self, task: &Task, is_new: bool) -> Result<(), A2AError> {
        let mut con = self.get_connection();
        let task_key = format!("task:{}", task.id);
        let status_key = format!(
            "tasks:status:{}",
            Self::task_state_to_string(&task.status.state)
        );
        let context_key = format!("tasks:context:{}", task.context_id);
        let timestamp = Self::current_timestamp();

        // Serialize optional fields
        let status_message_json = task
            .status
            .message
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());
        let metadata_json = task
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());
        let artifacts_json = task
            .artifacts
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        // Use pipeline for atomic operations
        let mut pipe = redis::pipe();
        pipe.atomic();

        // Store task data in hash
        pipe.hset(&task_key, "id", &task.id)
            .ignore()
            .hset(&task_key, "context_id", &task.context_id)
            .ignore()
            .hset(
                &task_key,
                "status_state",
                Self::task_state_to_string(&task.status.state),
            )
            .ignore()
            .hset(&task_key, "kind", &task.kind)
            .ignore()
            .hset(&task_key, "updated_at", timestamp)
            .ignore();

        if let Some(msg) = status_message_json {
            pipe.hset(&task_key, "status_message", msg).ignore();
        } else {
            pipe.hdel(&task_key, "status_message").ignore();
        }

        if let Some(meta) = metadata_json {
            pipe.hset(&task_key, "metadata", meta).ignore();
        } else {
            pipe.hdel(&task_key, "metadata").ignore();
        }

        if let Some(artifacts) = artifacts_json {
            pipe.hset(&task_key, "artifacts", artifacts).ignore();
        } else {
            pipe.hdel(&task_key, "artifacts").ignore();
        }

        if is_new {
            pipe.hset(&task_key, "created_at", timestamp).ignore();
        }

        // Add to sorted sets (score = timestamp)
        pipe.zadd("tasks:all", &task.id, timestamp).ignore();
        pipe.zadd(&context_key, &task.id, timestamp).ignore();

        // Add to status set
        pipe.sadd(&status_key, &task.id).ignore();

        // Set TTL for completed tasks
        if Self::is_terminal_state(&task.status.state) {
            pipe.expire(&task_key, self.completed_task_ttl as i64)
                .ignore();
            pipe.expire(
                &format!("task:{}:history", task.id),
                self.completed_task_ttl as i64,
            )
            .ignore();
        }

        pipe.query_async(&mut con)
            .await
            .map_err(|e: redis::RedisError| {
                A2AError::DatabaseError(format!("Failed to store task: {}", e))
            })?;

        Ok(())
    }

    /// Load task from Redis
    async fn load_task(&self, task_id: &str) -> Result<Task, A2AError> {
        let mut con = self.get_connection();
        let task_key = format!("task:{}", task_id);

        // Get all hash fields
        let task_data: Vec<(String, String)> = con
            .hgetall(&task_key)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to load task: {}", e)))?;

        if task_data.is_empty() {
            return Err(A2AError::TaskNotFound(task_id.to_string()));
        }

        // Parse hash into map
        let mut data_map = std::collections::HashMap::new();
        for (key, value) in task_data {
            data_map.insert(key, value);
        }

        // Extract required fields
        let id = data_map
            .get("id")
            .ok_or_else(|| A2AError::DatabaseError("Task missing id field".to_string()))?
            .clone();

        let context_id = data_map
            .get("context_id")
            .ok_or_else(|| A2AError::DatabaseError("Task missing context_id field".to_string()))?
            .clone();

        let status_state = data_map
            .get("status_state")
            .ok_or_else(|| A2AError::DatabaseError("Task missing status_state field".to_string()))?
            .clone();

        let kind = data_map.get("kind").unwrap_or(&"task".to_string()).clone();

        // Parse optional fields
        let status_message = data_map
            .get("status_message")
            .and_then(|s| serde_json::from_str(s).ok());

        let metadata = data_map
            .get("metadata")
            .and_then(|s| serde_json::from_str(s).ok());

        let artifacts = data_map
            .get("artifacts")
            .and_then(|s| serde_json::from_str(s).ok());

        let task = Task {
            id,
            context_id,
            status: TaskStatus {
                state: Self::parse_task_state(&status_state),
                message: status_message,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts,
            history: None, // Loaded separately
            metadata,
            kind,
        };

        Ok(task)
    }

    /// Load task history from Redis
    async fn load_task_history(
        &self,
        task_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Message>, A2AError> {
        let mut con = self.get_connection();
        let history_key = format!("task:{}:history", task_id);

        let message_strings: Vec<String> = if let Some(limit) = limit {
            // Get the last N messages (most recent)
            con.lrange(&history_key, -(limit as isize), -1)
                .await
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to load task history: {}", e))
                })?
        } else {
            // Get all messages
            con.lrange(&history_key, 0, -1).await.map_err(|e| {
                A2AError::DatabaseError(format!("Failed to load task history: {}", e))
            })?
        };

        let mut history = Vec::new();
        for msg_str in message_strings {
            if let Ok(message) = serde_json::from_str::<Message>(&msg_str) {
                history.push(message);
            }
        }

        Ok(history)
    }

    /// Add message to task history
    async fn add_to_history(&self, task_id: &str, message: &Message) -> Result<(), A2AError> {
        let mut con = self.get_connection();
        let history_key = format!("task:{}:history", task_id);

        let message_json = serde_json::to_string(message)
            .map_err(|e| A2AError::DatabaseError(format!("Failed to serialize message: {}", e)))?;

        con.rpush(&history_key, message_json)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to add to history: {}", e)))?;

        Ok(())
    }

    /// Remove task from old status set (for status transitions)
    async fn remove_from_status_set(
        &self,
        task_id: &str,
        old_state: &TaskState,
    ) -> Result<(), A2AError> {
        let mut con = self.get_connection();
        let old_status_key = format!("tasks:status:{}", Self::task_state_to_string(old_state));

        let _: () = con.srem(&old_status_key, task_id).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to remove from status set: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(feature = "redis")]
#[async_trait]
impl AsyncTaskManager for RedisTaskStorage {
    async fn create_task<'a>(
        &self,
        task_id: &'a str,
        context_id: &'a str,
    ) -> Result<Task, A2AError> {
        // Check if task already exists
        let exists = self.task_exists(task_id).await?;
        if exists {
            return Err(A2AError::DatabaseError(format!(
                "Task {} already exists",
                task_id
            )));
        }

        // Create new task
        let task = Task::new(task_id.to_string(), context_id.to_string());

        // Store task
        self.store_task(&task, true).await?;

        Ok(task)
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let mut task = self.load_task(task_id).await?;

        // Load history if requested
        if history_length.is_some() || history_length.is_none() {
            let history = self.load_task_history(task_id, history_length).await?;
            task.history = if history.is_empty() {
                None
            } else {
                Some(history)
            };
        }

        Ok(task)
    }

    async fn update_task_status<'a>(
        &self,
        task_id: &'a str,
        state: TaskState,
        message: Option<Message>,
    ) -> Result<Task, A2AError> {
        // Load current task to get old state
        let mut task = self.load_task(task_id).await?;
        let old_state = task.status.state.clone();

        // Update task status
        task.status = TaskStatus {
            state: state.clone(),
            message: message.clone(),
            timestamp: Some(chrono::Utc::now()),
        };

        // Remove from old status set if state changed
        if old_state != state {
            self.remove_from_status_set(task_id, &old_state).await?;
        }

        // Store updated task
        self.store_task(&task, false).await?;

        // Add message to history if provided
        if let Some(msg) = message {
            self.add_to_history(task_id, &msg).await?;
        }

        // Return task with history
        self.get_task(task_id, None).await
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        // Get current task
        let task = self.load_task(task_id).await?;

        // Only working tasks can be canceled
        if task.status.state != TaskState::Working {
            return Err(A2AError::TaskNotCancelable(format!(
                "Task {} is in state {:?} and cannot be canceled",
                task_id, task.status.state
            )));
        }

        // Create a cancellation message
        let cancel_message = Message {
            role: crate::domain::Role::Agent,
            parts: vec![crate::domain::Part::Text {
                text: format!("Task {} canceled.", task_id),
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: uuid::Uuid::new_v4().to_string(),
            task_id: Some(task_id.to_string()),
            context_id: Some(task.context_id.clone()),
            extensions: None,
            kind: "message".to_string(),
        };

        // Update task status
        self.update_task_status(task_id, TaskState::Canceled, Some(cancel_message))
            .await
    }

    async fn task_exists<'a>(&self, task_id: &'a str) -> Result<bool, A2AError> {
        let mut con = self.get_connection();
        let task_key = format!("task:{}", task_id);

        let exists: bool = con.exists(&task_key).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to check task existence: {}", e))
        })?;

        Ok(exists)
    }

    async fn list_tasks<'a>(
        &self,
        context_id: Option<&'a str>,
        limit: Option<u32>,
    ) -> Result<Vec<Task>, A2AError> {
        let mut con = self.get_connection();

        // Determine which sorted set to query
        let sorted_set_key = if let Some(ctx_id) = context_id {
            format!("tasks:context:{}", ctx_id)
        } else {
            "tasks:all".to_string()
        };

        // Get task IDs from sorted set (most recent first)
        let count = limit.unwrap_or(100) as isize;
        let task_ids: Vec<String> = con
            .zrevrange(&sorted_set_key, 0, count - 1)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to list tasks: {}", e)))?;

        // Load each task
        let mut tasks = Vec::new();
        for task_id in task_ids {
            if let Ok(task) = self.load_task(&task_id).await {
                tasks.push(task);
            }
        }

        Ok(tasks)
    }

    // ===== v0.3.0 Methods =====

    async fn list_tasks_v3<'a>(
        &self,
        params: &'a ListTasksParams,
    ) -> Result<ListTasksResult, A2AError> {
        let mut con = self.get_connection();

        // Build the set of task IDs to query based on filters
        let mut task_ids: Vec<String> = if let Some(ref ctx_id) = params.context_id {
            // Filter by context
            let context_key = format!("tasks:context:{}", ctx_id);
            con.zrevrange(&context_key, 0, -1).await.map_err(|e| {
                A2AError::DatabaseError(format!("Failed to get context tasks: {}", e))
            })?
        } else {
            // Get all tasks
            con.zrevrange("tasks:all", 0, -1)
                .await
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get all tasks: {}", e)))?
        };

        // Filter by status if specified
        if let Some(ref status) = params.status {
            let status_key = format!("tasks:status:{}", Self::task_state_to_string(status));
            let status_task_ids: Vec<String> = con.smembers(&status_key).await.map_err(|e| {
                A2AError::DatabaseError(format!("Failed to get status tasks: {}", e))
            })?;

            // Intersect with current task_ids
            let status_set: std::collections::HashSet<String> =
                status_task_ids.into_iter().collect();
            task_ids.retain(|id| status_set.contains(id));
        }

        // Filter by lastUpdatedAfter if specified
        if let Some(last_updated_after) = params.last_updated_after {
            let threshold_timestamp = last_updated_after / 1000; // Convert ms to seconds
            task_ids.retain(|_| true); // Placeholder - would need to check updated_at from hash
        }

        let total_size = task_ids.len() as i32;

        // Handle pagination
        let page_size = params.page_size.unwrap_or(50).clamp(1, 100);
        let offset = if let Some(ref token) = params.page_token {
            token.parse::<usize>().unwrap_or(0)
        } else {
            0
        };

        // Slice task IDs for this page
        let end = (offset + page_size as usize).min(task_ids.len());
        let page_task_ids = &task_ids[offset..end];

        // Load tasks
        let mut tasks = Vec::new();
        for task_id in page_task_ids {
            if let Ok(mut task) = self.load_task(task_id).await {
                // Load history if requested
                let history_length = params.history_length.unwrap_or(0);
                if history_length > 0 {
                    let history = self
                        .load_task_history(task_id, Some(history_length as u32))
                        .await?;
                    task.history = if history.is_empty() {
                        None
                    } else {
                        Some(history)
                    };
                } else {
                    task.history = None;
                }

                // Remove artifacts if not requested
                if !params.include_artifacts.unwrap_or(false) {
                    task.artifacts = None;
                }

                tasks.push(task);
            }
        }

        // Generate next page token
        let has_more = end < task_ids.len();
        let next_page_token = if has_more {
            end.to_string()
        } else {
            String::new()
        };

        Ok(ListTasksResult {
            tasks,
            total_size,
            page_size,
            next_page_token,
        })
    }

    async fn get_push_notification_config<'a>(
        &self,
        params: &'a GetTaskPushNotificationConfigParams,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let config_id = params.push_notification_config_id.as_ref().ok_or_else(|| {
            A2AError::DatabaseError("push_notification_config_id is required".to_string())
        })?;

        let mut con = self.get_connection();
        let config_key = format!("task:{}:push_configs:{}", params.id, config_id);

        // Get config hash
        let config_data: Vec<(String, String)> = con
            .hgetall(&config_key)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get push config: {}", e)))?;

        if config_data.is_empty() {
            return Err(A2AError::TaskNotFound(format!(
                "Push notification config {} not found for task {}",
                config_id, params.id
            )));
        }

        let mut data_map = std::collections::HashMap::new();
        for (key, value) in config_data {
            data_map.insert(key, value);
        }

        let url = data_map
            .get("url")
            .ok_or_else(|| A2AError::DatabaseError("Config missing url".to_string()))?
            .clone();

        let token = data_map.get("token").cloned();
        let authentication = data_map
            .get("authentication")
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(TaskPushNotificationConfig {
            task_id: params.id.clone(),
            push_notification_config: crate::domain::PushNotificationConfig {
                id: Some(config_id.clone()),
                url,
                token,
                authentication,
            },
        })
    }

    async fn list_push_notification_configs<'a>(
        &self,
        params: &'a ListTaskPushNotificationConfigParams,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError> {
        let mut con = self.get_connection();
        let configs_set_key = format!("task:{}:push_configs", params.id);

        // Get all config IDs
        let config_ids: Vec<String> = con.smembers(&configs_set_key).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to list push config IDs: {}", e))
        })?;

        let mut configs = Vec::new();
        for config_id in config_ids {
            let get_params = GetTaskPushNotificationConfigParams {
                id: params.id.clone(),
                push_notification_config_id: Some(config_id),
                metadata: None,
            };

            if let Ok(config) = self.get_push_notification_config(&get_params).await {
                configs.push(config);
            }
        }

        Ok(configs)
    }

    async fn delete_push_notification_config<'a>(
        &self,
        params: &'a DeleteTaskPushNotificationConfigParams,
    ) -> Result<(), A2AError> {
        let mut con = self.get_connection();
        let config_key = format!(
            "task:{}:push_configs:{}",
            params.id, params.push_notification_config_id
        );
        let configs_set_key = format!("task:{}:push_configs", params.id);

        // Use pipeline for atomic deletion
        let mut pipe = redis::pipe();
        pipe.atomic();
        pipe.del(&config_key).ignore();
        pipe.srem(&configs_set_key, &params.push_notification_config_id)
            .ignore();

        pipe.query_async(&mut con)
            .await
            .map_err(|e: redis::RedisError| {
                A2AError::DatabaseError(format!("Failed to delete push config: {}", e))
            })?;

        // Idempotent - don't error if already deleted
        Ok(())
    }
}

#[cfg(feature = "redis")]
impl Clone for RedisTaskStorage {
    fn clone(&self) -> Self {
        Self {
            connection_manager: self.connection_manager.clone(),
            completed_task_ttl: self.completed_task_ttl,
        }
    }
}
