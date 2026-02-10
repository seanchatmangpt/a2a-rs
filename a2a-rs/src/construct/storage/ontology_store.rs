//! SQLx-based persistent storage for CONSTRUCT ontology state.
//!
//! This module provides database-backed persistence for the OntologyState,
//! supporting both PostgreSQL and SQLite with checkpoint/rollback capabilities.

#[cfg(feature = "sqlx-storage")]
use async_trait::async_trait;

#[cfg(feature = "sqlx-storage")]
use crate::construct::ontology::{OntologyState, OntologyStorage};
#[cfg(feature = "sqlx-storage")]
use crate::domain::core::{AgentCard, Message, Task, TaskPushNotificationConfig};
#[cfg(feature = "sqlx-storage")]
use crate::domain::error::A2AError;

#[cfg(feature = "sqlx-storage")]
use sqlx::Row;
#[cfg(all(feature = "sqlx-storage", feature = "postgres"))]
use sqlx::postgres::PgPool;
#[cfg(all(feature = "sqlx-storage", feature = "sqlite"))]
use sqlx::sqlite::SqlitePool;

/// SQLx-based ontology storage with checkpoint/rollback support.
///
/// Provides persistent storage for CONSTRUCT ontology state with:
/// - Atomic save operations
/// - Checkpoint creation for rollback
/// - Support for PostgreSQL and SQLite
/// - Proper transaction handling
#[cfg(feature = "sqlx-storage")]
pub struct SqlxOntologyStore {
    #[cfg(feature = "postgres")]
    pool: PgPool,
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    pool: SqlitePool,
}

#[cfg(feature = "sqlx-storage")]
impl SqlxOntologyStore {
    /// Creates a new SQLx ontology store and runs migrations.
    ///
    /// # Arguments
    ///
    /// * `database_url` - Database connection URL (postgres:// or sqlite://)
    ///
    /// # Errors
    ///
    /// Returns `A2AError::DatabaseError` if connection or migrations fail.
    #[cfg(feature = "postgres")]
    pub async fn new(database_url: &str) -> Result<Self, A2AError> {
        let pool = PgPool::connect(database_url).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to connect to database: {}", e))
        })?;

        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    /// Creates a new SQLx ontology store and runs migrations (SQLite).
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    pub async fn new(database_url: &str) -> Result<Self, A2AError> {
        let pool = SqlitePool::connect(database_url).await.map_err(|e| {
            A2AError::DatabaseError(format!("Failed to connect to database: {}", e))
        })?;

        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    /// Runs database migrations to create ontology tables.
    async fn run_migrations(&self) -> Result<(), A2AError> {
        // PostgreSQL migrations
        #[cfg(feature = "postgres")]
        {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_tasks (
                    id TEXT PRIMARY KEY,
                    context_id TEXT NOT NULL,
                    status_state TEXT NOT NULL,
                    status_message JSONB,
                    status_timestamp TIMESTAMPTZ,
                    artifacts JSONB,
                    history JSONB,
                    metadata JSONB,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_tasks_context ON ontology_tasks(context_id);
                CREATE INDEX IF NOT EXISTS idx_tasks_state ON ontology_tasks(status_state);
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create tasks table: {}", e)))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_messages (
                    id SERIAL PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    message JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    FOREIGN KEY (task_id) REFERENCES ontology_tasks(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_messages_task ON ontology_messages(task_id);
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create messages table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_agents (
                    name TEXT PRIMARY KEY,
                    agent_card JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create agents table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_notifications (
                    task_id TEXT PRIMARY KEY,
                    config JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    FOREIGN KEY (task_id) REFERENCES ontology_tasks(id) ON DELETE CASCADE
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create notifications table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_checkpoints (
                    id SERIAL PRIMARY KEY,
                    checkpoint_name TEXT UNIQUE NOT NULL,
                    state_snapshot JSONB NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create checkpoints table: {}", e))
            })?;
        }

        // SQLite migrations
        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_tasks (
                    id TEXT PRIMARY KEY,
                    context_id TEXT NOT NULL,
                    status_state TEXT NOT NULL,
                    status_message TEXT,
                    status_timestamp TEXT,
                    artifacts TEXT,
                    history TEXT,
                    metadata TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX IF NOT EXISTS idx_tasks_context ON ontology_tasks(context_id);
                CREATE INDEX IF NOT EXISTS idx_tasks_state ON ontology_tasks(status_state);
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create tasks table: {}", e)))?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    task_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    message TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    FOREIGN KEY (task_id) REFERENCES ontology_tasks(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_messages_task ON ontology_messages(task_id);
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create messages table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_agents (
                    name TEXT PRIMARY KEY,
                    agent_card TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create agents table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_notifications (
                    task_id TEXT PRIMARY KEY,
                    config TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                    FOREIGN KEY (task_id) REFERENCES ontology_tasks(id) ON DELETE CASCADE
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create notifications table: {}", e))
            })?;

            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS ontology_checkpoints (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    checkpoint_name TEXT UNIQUE NOT NULL,
                    state_snapshot TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
                "#,
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to create checkpoints table: {}", e))
            })?;
        }

        Ok(())
    }

    /// Saves the complete ontology state to the database.
    ///
    /// This is an atomic operation - all state is saved in a transaction.
    async fn save_state_impl(&self, state: &OntologyState) -> Result<(), A2AError> {
        // Start transaction
        #[cfg(feature = "postgres")]
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                A2AError::DatabaseError(format!("Failed to begin transaction: {}", e))
            })?;

        #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
        let mut tx =
            self.pool.begin().await.map_err(|e| {
                A2AError::DatabaseError(format!("Failed to begin transaction: {}", e))
            })?;

        // Clear existing data
        sqlx::query("DELETE FROM ontology_tasks")
            .execute(&mut *tx)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to clear tasks: {}", e)))?;

        sqlx::query("DELETE FROM ontology_agents")
            .execute(&mut *tx)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to clear agents: {}", e)))?;

        sqlx::query("DELETE FROM ontology_notifications")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to clear notifications: {}", e))
            })?;

        // Save all tasks
        for task in state.get_all_tasks() {
            let status_message_json = task
                .status
                .message
                .as_ref()
                .map(|m| serde_json::to_string(m))
                .transpose()
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to serialize status message: {}", e))
                })?;

            let artifacts_json = task
                .artifacts
                .as_ref()
                .map(|a| serde_json::to_string(a))
                .transpose()
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to serialize artifacts: {}", e))
                })?;

            let history_json = task
                .history
                .as_ref()
                .map(|h| serde_json::to_string(h))
                .transpose()
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to serialize history: {}", e))
                })?;

            let metadata_json = task
                .metadata
                .as_ref()
                .map(|m| serde_json::to_string(m))
                .transpose()
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to serialize metadata: {}", e))
                })?;

            let state_str = format!("{:?}", task.status.state).to_lowercase();
            let timestamp_str = task
                .status
                .timestamp
                .map(|t| t.to_rfc3339())
                .unwrap_or_default();

            sqlx::query(
                "INSERT INTO ontology_tasks (id, context_id, status_state, status_message, status_timestamp, artifacts, history, metadata) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&task.id)
            .bind(&task.context_id)
            .bind(state_str)
            .bind(status_message_json)
            .bind(timestamp_str)
            .bind(artifacts_json)
            .bind(history_json)
            .bind(metadata_json)
            .execute(&mut *tx)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to insert task: {}", e)))?;

            // Save messages for this task
            if let Some(messages) = state.get_messages(&task.id) {
                for message in messages {
                    let message_json = serde_json::to_string(message).map_err(|e| {
                        A2AError::DatabaseError(format!("Failed to serialize message: {}", e))
                    })?;

                    sqlx::query(
                        "INSERT INTO ontology_messages (task_id, message_id, message) VALUES (?, ?, ?)",
                    )
                    .bind(&task.id)
                    .bind(&message.message_id)
                    .bind(message_json)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        A2AError::DatabaseError(format!("Failed to insert message: {}", e))
                    })?;
                }
            }
        }

        // Save all agents
        for agent in state.get_all_agents() {
            let agent_json = serde_json::to_string(agent).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to serialize agent: {}", e))
            })?;

            sqlx::query("INSERT INTO ontology_agents (name, agent_card) VALUES (?, ?)")
                .bind(&agent.name)
                .bind(agent_json)
                .execute(&mut *tx)
                .await
                .map_err(|e| A2AError::DatabaseError(format!("Failed to insert agent: {}", e)))?;
        }

        // Save all notification configs
        for (task_id, config) in state.get_all_notification_configs() {
            let config_json = serde_json::to_string(config).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to serialize notification config: {}", e))
            })?;

            sqlx::query("INSERT INTO ontology_notifications (task_id, config) VALUES (?, ?)")
                .bind(task_id)
                .bind(config_json)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to insert notification: {}", e))
                })?;
        }

        // Commit transaction
        tx.commit()
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Loads the complete ontology state from the database.
    async fn load_state_impl(&self) -> Result<OntologyState, A2AError> {
        let mut state = OntologyState::new();

        // Load all tasks
        let task_rows = sqlx::query("SELECT * FROM ontology_tasks ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to load tasks: {}", e)))?;

        for row in task_rows {
            let task = self.row_to_task(&row)?;
            state.put_task(task)?;
        }

        // Load all messages
        let message_rows =
            sqlx::query("SELECT task_id, message FROM ontology_messages ORDER BY task_id, id")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| A2AError::DatabaseError(format!("Failed to load messages: {}", e)))?;

        for row in message_rows {
            let task_id: String = row
                .try_get("task_id")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get task_id: {}", e)))?;

            let message_json: String = row
                .try_get("message")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get message: {}", e)))?;

            let message: Message = serde_json::from_str(&message_json).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to deserialize message: {}", e))
            })?;

            state.add_message(&task_id, message)?;
        }

        // Load all agents
        let agent_rows = sqlx::query("SELECT agent_card FROM ontology_agents ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to load agents: {}", e)))?;

        for row in agent_rows {
            let agent_json: String = row
                .try_get("agent_card")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get agent_card: {}", e)))?;

            let agent: AgentCard = serde_json::from_str(&agent_json).map_err(|e| {
                A2AError::DatabaseError(format!("Failed to deserialize agent: {}", e))
            })?;

            state.put_agent(agent)?;
        }

        // Load all notification configs
        let notification_rows = sqlx::query("SELECT task_id, config FROM ontology_notifications")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to load notifications: {}", e)))?;

        for row in notification_rows {
            let task_id: String = row
                .try_get("task_id")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get task_id: {}", e)))?;

            let config_json: String = row
                .try_get("config")
                .map_err(|e| A2AError::DatabaseError(format!("Failed to get config: {}", e)))?;

            let config: TaskPushNotificationConfig =
                serde_json::from_str(&config_json).map_err(|e| {
                    A2AError::DatabaseError(format!("Failed to deserialize config: {}", e))
                })?;

            state.put_notification_config(&task_id, config)?;
        }

        Ok(state)
    }

    /// Converts a database row to a Task (PostgreSQL).
    #[cfg(feature = "postgres")]
    fn row_to_task(&self, row: &sqlx::postgres::PgRow) -> Result<Task, A2AError> {
        use sqlx::Row;

        let id: String = row
            .try_get("id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get id: {}", e)))?;

        let context_id: String = row
            .try_get("context_id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get context_id: {}", e)))?;

        let status_state: String = row
            .try_get("status_state")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get status_state: {}", e)))?;

        let status_message_json: Option<String> = row.try_get("status_message").ok();
        let artifacts_json: Option<String> = row.try_get("artifacts").ok();
        let history_json: Option<String> = row.try_get("history").ok();
        let metadata_json: Option<String> = row.try_get("metadata").ok();
        let timestamp_str: Option<String> = row.try_get("status_timestamp").ok();

        // Parse state
        let state = crate::domain::core::TaskState::from_str(&status_state);

        // Parse optional fields
        let status_message = status_message_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to parse status message: {}", e))
            })?;

        let artifacts = artifacts_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse artifacts: {}", e)))?;

        let history = history_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse history: {}", e)))?;

        let metadata = metadata_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse metadata: {}", e)))?;

        let timestamp = timestamp_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(Task {
            id,
            context_id,
            status: crate::domain::core::TaskStatus {
                state,
                message: status_message,
                timestamp,
            },
            artifacts,
            history,
            metadata,
            kind: "task".to_string(),
        })
    }

    /// Converts a database row to a Task (SQLite).
    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    fn row_to_task(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Task, A2AError> {
        use sqlx::Row;

        let id: String = row
            .try_get("id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get id: {}", e)))?;

        let context_id: String = row
            .try_get("context_id")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get context_id: {}", e)))?;

        let status_state: String = row
            .try_get("status_state")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get status_state: {}", e)))?;

        let status_message_json: Option<String> = row.try_get("status_message").ok();
        let artifacts_json: Option<String> = row.try_get("artifacts").ok();
        let history_json: Option<String> = row.try_get("history").ok();
        let metadata_json: Option<String> = row.try_get("metadata").ok();
        let timestamp_str: Option<String> = row.try_get("status_timestamp").ok();

        // Parse state
        let state = crate::domain::core::TaskState::from_str(&status_state);

        // Parse optional fields
        let status_message = status_message_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| {
                A2AError::DatabaseError(format!("Failed to parse status message: {}", e))
            })?;

        let artifacts = artifacts_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse artifacts: {}", e)))?;

        let history = history_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse history: {}", e)))?;

        let metadata = metadata_json
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to parse metadata: {}", e)))?;

        let timestamp = timestamp_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(Task {
            id,
            context_id,
            status: crate::domain::core::TaskStatus {
                state,
                message: status_message,
                timestamp,
            },
            artifacts,
            history,
            metadata,
            kind: "task".to_string(),
        })
    }

    /// Creates a checkpoint of the current database state.
    ///
    /// Checkpoints allow rollback to a previous state.
    ///
    /// # Arguments
    ///
    /// * `checkpoint_name` - Unique name for the checkpoint
    pub async fn checkpoint(&self, checkpoint_name: &str) -> Result<(), A2AError> {
        // Load current state
        let state = self.load_state_impl().await?;

        // Serialize to JSON
        let snapshot_json = serde_json::to_string(&state).map_err(|e| {
            A2AError::DatabaseError(format!("Failed to serialize state snapshot: {}", e))
        })?;

        // Store checkpoint
        sqlx::query(
            "INSERT INTO ontology_checkpoints (checkpoint_name, state_snapshot) VALUES (?, ?) ON CONFLICT (checkpoint_name) DO UPDATE SET state_snapshot = EXCLUDED.state_snapshot",
        )
        .bind(checkpoint_name)
        .bind(snapshot_json)
        .execute(&self.pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Failed to create checkpoint: {}", e)))?;

        Ok(())
    }

    /// Rolls back the database to a previous checkpoint.
    ///
    /// # Arguments
    ///
    /// * `checkpoint_name` - Name of the checkpoint to restore
    pub async fn rollback(&self, checkpoint_name: &str) -> Result<(), A2AError> {
        // Load checkpoint
        let row = sqlx::query(
            "SELECT state_snapshot FROM ontology_checkpoints WHERE checkpoint_name = ?",
        )
        .bind(checkpoint_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Failed to load checkpoint: {}", e)))?;

        let snapshot_json = row
            .ok_or_else(|| {
                A2AError::DatabaseError(format!("Checkpoint '{}' not found", checkpoint_name))
            })?
            .try_get::<String, _>("state_snapshot")
            .map_err(|e| A2AError::DatabaseError(format!("Failed to get snapshot data: {}", e)))?;

        // Deserialize state
        let state: OntologyState = serde_json::from_str(&snapshot_json).map_err(|e| {
            A2AError::DatabaseError(format!("Failed to deserialize checkpoint: {}", e))
        })?;

        // Restore state
        self.save_state_impl(&state).await?;

        Ok(())
    }

    /// Deletes a checkpoint.
    ///
    /// # Arguments
    ///
    /// * `checkpoint_name` - Name of the checkpoint to delete
    pub async fn delete_checkpoint(&self, checkpoint_name: &str) -> Result<(), A2AError> {
        sqlx::query("DELETE FROM ontology_checkpoints WHERE checkpoint_name = ?")
            .bind(checkpoint_name)
            .execute(&self.pool)
            .await
            .map_err(|e| A2AError::DatabaseError(format!("Failed to delete checkpoint: {}", e)))?;

        Ok(())
    }

    /// Lists all available checkpoints.
    pub async fn list_checkpoints(&self) -> Result<Vec<String>, A2AError> {
        let rows = sqlx::query(
            "SELECT checkpoint_name FROM ontology_checkpoints ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| A2AError::DatabaseError(format!("Failed to list checkpoints: {}", e)))?;

        let checkpoints = rows
            .iter()
            .filter_map(|row| row.try_get("checkpoint_name").ok())
            .collect();

        Ok(checkpoints)
    }
}

// Implement OntologyStorage trait
#[cfg(feature = "sqlx-storage")]
impl OntologyStorage for SqlxOntologyStore {
    fn save(&self, state: &OntologyState) -> Result<(), A2AError> {
        // Block on async operation
        tokio::runtime::Runtime::new()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create async runtime: {}", e)))?
            .block_on(self.save_state_impl(state))
    }

    fn load(&self) -> Result<OntologyState, A2AError> {
        // Block on async operation
        tokio::runtime::Runtime::new()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create async runtime: {}", e)))?
            .block_on(self.load_state_impl())
    }

    fn exists(&self) -> bool {
        // Check if any tasks exist in the database
        tokio::runtime::Runtime::new()
            .ok()
            .and_then(|rt| {
                rt.block_on(async {
                    sqlx::query("SELECT COUNT(*) as count FROM ontology_tasks")
                        .fetch_one(&self.pool)
                        .await
                        .ok()
                        .and_then(|row| row.try_get::<i64, _>("count").ok())
                        .map(|count| count > 0)
                })
            })
            .unwrap_or(false)
    }

    fn delete(&self) -> Result<(), A2AError> {
        tokio::runtime::Runtime::new()
            .map_err(|e| A2AError::DatabaseError(format!("Failed to create async runtime: {}", e)))?
            .block_on(async {
                sqlx::query("DELETE FROM ontology_tasks")
                    .execute(&self.pool)
                    .await
                    .map_err(|e| {
                        A2AError::DatabaseError(format!("Failed to delete tasks: {}", e))
                    })?;

                sqlx::query("DELETE FROM ontology_agents")
                    .execute(&self.pool)
                    .await
                    .map_err(|e| {
                        A2AError::DatabaseError(format!("Failed to delete agents: {}", e))
                    })?;

                sqlx::query("DELETE FROM ontology_notifications")
                    .execute(&self.pool)
                    .await
                    .map_err(|e| {
                        A2AError::DatabaseError(format!("Failed to delete notifications: {}", e))
                    })?;

                Ok(())
            })
    }
}

// Extension trait for async operations
#[cfg(feature = "sqlx-storage")]
#[async_trait]
pub trait AsyncOntologyStorage: Send + Sync {
    async fn save_async(&self, state: &OntologyState) -> Result<(), A2AError>;
    async fn load_async(&self) -> Result<OntologyState, A2AError>;
    async fn checkpoint_async(&self, checkpoint_name: &str) -> Result<(), A2AError>;
    async fn rollback_async(&self, checkpoint_name: &str) -> Result<(), A2AError>;
}

#[cfg(feature = "sqlx-storage")]
#[async_trait]
impl AsyncOntologyStorage for SqlxOntologyStore {
    async fn save_async(&self, state: &OntologyState) -> Result<(), A2AError> {
        self.save_state_impl(state).await
    }

    async fn load_async(&self) -> Result<OntologyState, A2AError> {
        self.load_state_impl().await
    }

    async fn checkpoint_async(&self, checkpoint_name: &str) -> Result<(), A2AError> {
        self.checkpoint(checkpoint_name).await
    }

    async fn rollback_async(&self, checkpoint_name: &str) -> Result<(), A2AError> {
        self.rollback(checkpoint_name).await
    }
}

// Helper to parse TaskState from string
#[cfg(feature = "sqlx-storage")]
impl crate::domain::core::TaskState {
    fn from_str(s: &str) -> Self {
        match s {
            "submitted" => Self::Submitted,
            "working" => Self::Working,
            "inputrequired" | "input-required" => Self::InputRequired,
            "completed" => Self::Completed,
            "canceled" => Self::Canceled,
            "failed" => Self::Failed,
            "rejected" => Self::Rejected,
            "authrequired" | "auth-required" => Self::AuthRequired,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
#[cfg(feature = "sqlx-storage")]
mod tests {
    use super::*;
    use crate::domain::core::{Task, TaskState, TaskStatus};

    #[tokio::test]
    #[cfg(feature = "sqlite")]
    async fn test_save_and_load_state() {
        let store = SqlxOntologyStore::new("sqlite::memory:")
            .await
            .expect("Failed to create store");

        let mut state = OntologyState::new();
        let task = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();

        state.put_task(task).expect("Failed to put task");

        store
            .save_state_impl(&state)
            .await
            .expect("Failed to save state");

        let loaded_state = store.load_state_impl().await.expect("Failed to load state");

        assert_eq!(loaded_state.task_count(), 1);
        assert!(loaded_state.get_task("task-1").is_some());
    }

    #[tokio::test]
    #[cfg(feature = "sqlite")]
    async fn test_checkpoint_and_rollback() {
        let store = SqlxOntologyStore::new("sqlite::memory:")
            .await
            .expect("Failed to create store");

        // Create initial state
        let mut state = OntologyState::new();
        let task1 = Task::builder()
            .id("task-1".to_string())
            .context_id("ctx-1".to_string())
            .status(TaskStatus::default())
            .build();
        state.put_task(task1).expect("Failed to put task");
        store
            .save_state_impl(&state)
            .await
            .expect("Failed to save state");

        // Create checkpoint
        store
            .checkpoint("checkpoint-1")
            .await
            .expect("Failed to create checkpoint");

        // Modify state
        let task2 = Task::builder()
            .id("task-2".to_string())
            .context_id("ctx-2".to_string())
            .status(TaskStatus::default())
            .build();
        state.put_task(task2).expect("Failed to put task");
        store
            .save_state_impl(&state)
            .await
            .expect("Failed to save modified state");

        // Verify modified state
        let modified_state = store.load_state_impl().await.expect("Failed to load state");
        assert_eq!(modified_state.task_count(), 2);

        // Rollback to checkpoint
        store
            .rollback("checkpoint-1")
            .await
            .expect("Failed to rollback");

        // Verify rollback
        let restored_state = store.load_state_impl().await.expect("Failed to load state");
        assert_eq!(restored_state.task_count(), 1);
        assert!(restored_state.get_task("task-1").is_some());
        assert!(restored_state.get_task("task-2").is_none());
    }
}
