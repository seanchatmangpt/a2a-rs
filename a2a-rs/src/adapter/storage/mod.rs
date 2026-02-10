//! Storage adapter implementations

#[cfg(feature = "server")]
pub mod task_storage;

#[cfg(feature = "sqlx-storage")]
pub mod sqlx_storage;

#[cfg(feature = "sqlite")]
pub mod sqlite_task;

#[cfg(feature = "redis")]
pub mod redis_task;

#[cfg(feature = "sqlx-storage")]
pub mod database_config;

#[cfg(feature = "server")]
pub use task_storage::InMemoryTaskStorage;

#[cfg(feature = "sqlx-storage")]
pub use sqlx_storage::SqlxTaskStorage;

#[cfg(feature = "sqlite")]
pub use sqlite_task::SqliteTaskStorage;

#[cfg(feature = "redis")]
pub use redis_task::RedisTaskStorage;

#[cfg(feature = "sqlx-storage")]
pub use database_config::DatabaseConfig;
