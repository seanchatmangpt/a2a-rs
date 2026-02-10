//! SQLite-specific task storage adapter
//!
//! This module provides a persistent task storage implementation using SQLite
//! via sqlx. It implements the AsyncTaskManager port trait with full CRUD
//! operations and filtering capabilities.

#[cfg(feature = "sqlite")]
use async_trait::async_trait;
#[cfg(feature = "sqlite")]
use serde_json;
#[cfg(feature = "sqlite")]
use sqlx::{Row, SqlitePool};

#[cfg(feature = "sqlite")]
use crate::domain::{
    A2AError, DeleteTaskPushNotificationConfigParams, GetTaskPushNotificationConfigParams,
    ListTaskPushNotificationConfigParams, ListTasksParams, ListTasksResult, Message, Task,
    TaskPushNotificationConfig, TaskState, TaskStatus,
};
#[cfg(feature = "sqlite")]
use crate::port::AsyncTaskManager;

#[cfg(feature = "sqlite")]
/// SQLite task storage adapter
///
/// Provides persistent task storage using SQLite with support for:
/// - Full CRUD operations on tasks
/// - Filtering by status, context_id (agent_id), and creation time
/// - Task history tracking
/// - Push notification configuration storage
pub struct SqliteTaskStorage {
    /// Database connection pool
    pool: SqlitePool,
}

#[cfg(feature = "sqlite")]
impl SqliteTaskStorage {
    /// Create a new SQLite task storage with the given database URL
    ///
    /// # Arguments
    /// * `database_url` - SQLite connection string (e.g., "sqlite:tasks.db" or "sqlite::memory:")
    ///
    /// # Example
    /// ```no_run
    /// # use a2a_rs::adapter::storage::SqliteTaskStorage;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let storage = SqliteTaskStorage::new("sqlite:tasks.db").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(database_url: &str) -> Result<Self, A2AError> {
        let pool = SqlitePool::connect(database_url).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to connect to database: {}", e))
        })?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    /// Create storage from an existing pool
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Run database migrations
    async fn run_migrations(pool: &SqlitePool) -> Result<(), A2AError> {
        // Create tasks table
        sqlx::query(include_str!("../../../migrations/001_initial_schema.sql"))
            .execute(pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Migration 001 failed: {}", e)))?;

        // Create v0.3.0 push notification configs
        sqlx::query(include_str!(
            "../../../migrations/002_v030_push_configs.sql"
        ))
        .execute(pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Migration 002 failed: {}", e)))?;

        Ok(())
    }

    /// Convert database row to Task
    fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<Task, A2AError> {
        let task_id: String = row
            .try_get("id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get task_id: {}", e)))?;
        let context_id: String = row
            .try_get("context_id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get context_id: {}", e)))?;
        let status_state: String = row
            .try_get("status_state")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get status_state: {}", e)))?;
        let status_message_json: Option<String> = row
            .try_get("status_message")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get status_message: {}", e)))?;
        let metadata_json: Option<String> = row
            .try_get("metadata")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get metadata: {}", e)))?;
        let artifacts_json: Option<String> = row
            .try_get("artifacts")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get artifacts: {}", e)))?;

        // Parse task state
        let state = Self::parse_task_state(&status_state);

        // Parse status message
        let status_message = if let Some(msg_str) = status_message_json {
            Some(serde_json::from_str(&msg_str).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to parse status message: {}", e))
            })?)
        } else {
            None
        };

        // Parse metadata
        let metadata =
            if let Some(meta_str) = metadata_json {
                Some(serde_json::from_str(&meta_str).map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to parse metadata: {}", e))
                })?)
            } else {
                None
            };

        // Parse artifacts
        let artifacts = if let Some(artifacts_str) = artifacts_json {
            Some(serde_json::from_str(&artifacts_str).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to parse artifacts: {}", e))
            })?)
        } else {
            None
        };

        let task_status = TaskStatus {
            state,
            message: status_message,
            timestamp: Some(chrono::Utc::now()),
        };

        let task = Task {
            id: task_id,
            context_id,
            status: task_status,
            history: None, // Will be set separately if needed
            metadata,
            artifacts,
            kind: "task".to_string(),
        };

        Ok(task)
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

    /// Load task history from database
    async fn load_task_history(
        &self,
        task_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Message>, A2AError> {
        let query_str = if let Some(limit) = limit {
            format!(
                "SELECT message FROM task_history WHERE task_id = ? ORDER BY timestamp ASC LIMIT {}",
                limit
            )
        } else {
            "SELECT message FROM task_history WHERE task_id = ? ORDER BY timestamp ASC".to_string()
        };

        let rows = sqlx::query(&query_str)
            .bind(task_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to load task history: {}", e)))?;

        let mut history = Vec::new();
        for row in rows {
            let message_json: Option<String> = row.try_get("message").map_err(|e| {
                A2AError::DatabaseError(format!("Failed to get message from history: {}", e))
            })?;

            if let Some(msg_str) = message_json {
                let message: Message = serde_json::from_str(&msg_str).map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to parse message from history: {}", e))
                })?;
                history.push(message);
            }
        }

        Ok(history)
    }

    /// Add entry to task history
    async fn add_to_history(
        &self,
        task_id: &str,
        state: &TaskState,
        message: Option<&Message>,
    ) -> Result<(), A2AError> {
        let state_str = Self::task_state_to_string(state);

        let message_json = if let Some(msg) = message {
            Some(serde_json::to_string(msg).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to serialize message: {}", e))
            })?)
        } else {
            None
        };

        sqlx::query("INSERT INTO task_history (task_id, status_state, message) VALUES (?, ?, ?)")
            .bind(task_id)
            .bind(state_str)
            .bind(message_json)
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to add task history: {}", e)))?;

        Ok(())
    }

    /// Query tasks with filtering
    ///
    /// # Arguments
    /// * `context_id` - Filter by context ID (agent ID)
    /// * `status` - Filter by task status
    /// * `created_after` - Filter by creation timestamp
    /// * `limit` - Maximum number of tasks to return
    pub async fn query_tasks(
        &self,
        context_id: Option<&str>,
        status: Option<TaskState>,
        created_after: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<Task>, A2AError> {
        let mut where_conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(ctx_id) = context_id {
            where_conditions.push("context_id = ?");
            params.push(ctx_id.to_string());
        }

        if let Some(state) = status {
            where_conditions.push("status_state = ?");
            params.push(Self::task_state_to_string(&state).to_string());
        }

        if let Some(created_after) = created_after {
            where_conditions.push("created_at > ?");
            params.push(created_after.format("%Y-%m-%d %H:%M:%S").to_string());
        }

        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        let limit_clause = if let Some(limit) = limit {
            format!(" LIMIT {}", limit)
        } else {
            String::new()
        };

        let query_str = format!(
            "SELECT * FROM tasks{} ORDER BY created_at DESC{}",
            where_clause, limit_clause
        );

        let mut query = sqlx::query(&query_str);
        for param in params {
            query = query.bind(param);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to query tasks: {}", e)))?;

        rows.iter()
            .map(|row| Self::row_to_task(row))
            .collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl AsyncTaskManager for SqliteTaskStorage {
    async fn create_task<'a>(
        &self,
        task_id: &'a str,
        context_id: &'a str,
    ) -> Result<Task, A2AError> {
        // Check if task already exists
        let existing = sqlx::query("SELECT id FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to check existing task: {}", e))
            })?;

        if existing.is_some() {
            return Err(A2AError::TaskNotFound(format!(
                "Task {} already exists",
                task_id
            )));
        }

        // Create new task
        let task = Task::new(task_id.to_string(), context_id.to_string());

        // Serialize metadata and artifacts
        let metadata_json = task
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());
        let artifacts_json = task
            .artifacts
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());
        let status_message_str = task
            .status
            .message
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        // Insert into database
        sqlx::query("INSERT INTO tasks (id, context_id, status_state, status_message, metadata, artifacts) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&task.id)
            .bind(&task.context_id)
            .bind("submitted")
            .bind(status_message_str)
            .bind(metadata_json)
            .bind(artifacts_json)
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create task: {}", e)))?;

        // Add initial history entry
        self.add_to_history(task_id, &TaskState::Submitted, None)
            .await?;

        Ok(task)
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        // Get task from database
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get task: {}", e)))?;

        let Some(row) = row else {
            return Err(A2AError::TaskNotFound(task_id.to_string()));
        };

        let mut task = Self::row_to_task(&row)?;

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
        let state_str = Self::task_state_to_string(&state);

        // Serialize status message if present
        let status_message_str = message
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        // Update task in database
        let result =
            sqlx::query("UPDATE tasks SET status_state = ?, status_message = ? WHERE id = ?")
                .bind(state_str)
                .bind(status_message_str)
                .bind(task_id)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to update task status: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(A2AError::TaskNotFound(task_id.to_string()));
        }

        // Add to history
        self.add_to_history(task_id, &state, message.as_ref())
            .await?;

        // Get and return updated task
        self.get_task(task_id, None).await
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        // Get current task
        let task = self.get_task(task_id, None).await?;

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
        let cancel_message_str = serde_json::to_string(&cancel_message).unwrap_or_default();
        sqlx::query("UPDATE tasks SET status_state = ?, status_message = ? WHERE id = ?")
            .bind("canceled")
            .bind(cancel_message_str)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to cancel task: {}", e)))?;

        // Add to history
        self.add_to_history(task_id, &TaskState::Canceled, Some(&cancel_message))
            .await?;

        // Get and return updated task
        self.get_task(task_id, None).await
    }

    async fn task_exists<'a>(&self, task_id: &'a str) -> Result<bool, A2AError> {
        let row = sqlx::query("SELECT id FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to check task existence: {}", e))
            })?;

        Ok(row.is_some())
    }

    async fn list_tasks<'a>(
        &self,
        context_id: Option<&'a str>,
        limit: Option<u32>,
    ) -> Result<Vec<Task>, A2AError> {
        self.query_tasks(context_id, None, None, limit).await
    }

    // ===== v0.3.0 Methods =====

    async fn list_tasks_v3<'a>(
        &self,
        params: &'a ListTasksParams,
    ) -> Result<ListTasksResult, A2AError> {
        // Build WHERE clause conditions
        let mut where_conditions = Vec::new();
        let mut bind_params: Vec<String> = Vec::new();

        // Filter by context_id
        if let Some(ref context_id) = params.context_id {
            where_conditions.push("context_id = ?");
            bind_params.push(context_id.clone());
        }

        // Filter by status
        if let Some(ref status) = params.status {
            where_conditions.push("status_state = ?");
            bind_params.push(Self::task_state_to_string(status).to_string());
        }

        // Filter by lastUpdatedAfter
        if let Some(last_updated_after) = params.last_updated_after {
            let timestamp = chrono::DateTime::from_timestamp_millis(last_updated_after)
                .unwrap_or(chrono::Utc::now());
            where_conditions.push("updated_at > ?");
            bind_params.push(timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
        }

        // Build WHERE clause
        let where_clause = if where_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_conditions.join(" AND "))
        };

        // Get total count
        let count_query = format!("SELECT COUNT(*) as count FROM tasks{}", where_clause);
        let mut count_q = sqlx::query(&count_query);
        for param in &bind_params {
            count_q = count_q.bind(param);
        }

        let count_row = count_q
            .fetch_one(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to count tasks: {}", e)))?;

        let total_size: i32 = count_row
            .try_get("count")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get count: {}", e)))?;

        // Handle pagination
        let page_size = params.page_size.unwrap_or(50).clamp(1, 100);
        let offset = if let Some(ref token) = params.page_token {
            token.parse::<i32>().unwrap_or(0)
        } else {
            0
        };

        // Build main query
        let main_query = format!(
            "SELECT * FROM tasks{} ORDER BY updated_at DESC LIMIT ? OFFSET ?",
            where_clause
        );

        let mut main_q = sqlx::query(&main_query);
        for param in &bind_params {
            main_q = main_q.bind(param);
        }
        main_q = main_q.bind(page_size).bind(offset);

        let rows = main_q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to list tasks: {}", e)))?;

        // Convert rows to tasks
        let mut tasks: Vec<Task> = rows
            .iter()
            .filter_map(|row| Self::row_to_task(row).ok())
            .collect();

        // Load history for each task if requested
        let history_length = params.history_length.unwrap_or(0);
        for task in &mut tasks {
            if history_length > 0 {
                let history = self
                    .load_task_history(&task.id, Some(history_length as u32))
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
        }

        // Generate next page token
        let has_more = offset + page_size < total_size;
        let next_page_token = if has_more {
            (offset + page_size).to_string()
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
            A2AError::TaskNotFound("push_notification_config_id is required".to_string())
        })?;

        let row = sqlx::query(
            "SELECT id, task_id, url, token, authentication FROM push_notification_configs WHERE task_id = ? AND id = ?"
        )
        .bind(&params.id)
        .bind(config_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Failed to get push config: {}", e)))?;

        if let Some(row) = row {
            let id: String = row
                .try_get("id")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get config id: {}", e)))?;
            let url: String = row
                .try_get("url")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get url: {}", e)))?;
            let token: Option<String> = row.try_get("token").ok();
            let auth_json: Option<String> = row.try_get("authentication").ok();

            let authentication = if let Some(auth_str) = auth_json {
                serde_json::from_str(&auth_str).ok()
            } else {
                None
            };

            Ok(TaskPushNotificationConfig {
                task_id: params.id.clone(),
                push_notification_config: crate::domain::PushNotificationConfig {
                    id: Some(id),
                    url,
                    token,
                    authentication,
                },
            })
        } else {
            Err(A2AError::TaskNotFound(format!(
                "Push notification config not found for task {} with id {}",
                params.id, config_id
            )))
        }
    }

    async fn list_push_notification_configs<'a>(
        &self,
        params: &'a ListTaskPushNotificationConfigParams,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError> {
        let rows = sqlx::query(
            "SELECT id, task_id, url, token, authentication FROM push_notification_configs WHERE task_id = ?"
        )
        .bind(&params.id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Failed to list push configs: {}", e)))?;

        let configs: Vec<TaskPushNotificationConfig> = rows
            .iter()
            .filter_map(|row| {
                let id: String = row.try_get("id").ok()?;
                let url: String = row.try_get("url").ok()?;
                let token: Option<String> = row.try_get("token").ok().flatten();
                let auth_json: Option<String> = row.try_get("authentication").ok().flatten();

                let authentication = if let Some(auth_str) = auth_json {
                    serde_json::from_str(&auth_str).ok()
                } else {
                    None
                };

                Some(TaskPushNotificationConfig {
                    task_id: params.id.clone(),
                    push_notification_config: crate::domain::PushNotificationConfig {
                        id: Some(id),
                        url,
                        token,
                        authentication,
                    },
                })
            })
            .collect();

        Ok(configs)
    }

    async fn delete_push_notification_config<'a>(
        &self,
        params: &'a DeleteTaskPushNotificationConfigParams,
    ) -> Result<(), A2AError> {
        sqlx::query("DELETE FROM push_notification_configs WHERE task_id = ? AND id = ?")
            .bind(&params.id)
            .bind(&params.push_notification_config_id)
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to delete push config: {}", e)))?;

        // Idempotent - don't error if already deleted (v0.3.0 spec behavior)
        Ok(())
    }
}

#[cfg(feature = "sqlite")]
impl Clone for SqliteTaskStorage {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
