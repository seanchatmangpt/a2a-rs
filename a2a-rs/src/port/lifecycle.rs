//! Lifecycle management port
//!
//! Defines contracts for application lifecycle operations including
//! startup, graceful shutdown, and health checks.

use async_trait::async_trait;
use std::time::Duration;

use crate::domain::A2AError;

/// Lifecycle manager for graceful startup and shutdown
#[async_trait]
pub trait LifecycleManager: Send + Sync {
    /// Start the service/component
    async fn start(&self) -> Result<(), A2AError>;

    /// Initiate graceful shutdown
    ///
    /// Returns when shutdown signal is sent to all components.
    /// Does not wait for completion.
    async fn shutdown(&self) -> Result<(), A2AError>;

    /// Wait for all components to finish shutting down
    ///
    /// Returns when all in-flight operations complete or timeout expires.
    async fn wait_for_shutdown(&self, timeout: Duration) -> Result<(), A2AError>;

    /// Check if the service is currently running
    async fn is_running(&self) -> bool;

    /// Check if shutdown has been initiated
    async fn is_shutting_down(&self) -> bool;
}

/// Shutdown coordinator for managing drain period and component ordering
#[async_trait]
pub trait ShutdownCoordinator: Send + Sync {
    /// Register a shutdown hook to be called during graceful shutdown
    ///
    /// Hooks are executed in reverse registration order (LIFO).
    /// This ensures listeners stop before workers, workers before storage, etc.
    async fn register_shutdown_hook(
        &self,
        name: String,
        hook: Box<dyn ShutdownHook>,
    ) -> Result<(), A2AError>;

    /// Signal all registered hooks to begin shutdown
    ///
    /// Executes hooks in reverse registration order with proper ordering:
    /// 1. Stop accepting new requests (listeners)
    /// 2. Drain in-flight requests (workers)
    /// 3. Close connections (storage, caches)
    async fn trigger_shutdown(&self, timeout: Duration) -> Result<(), A2AError>;

    /// Get number of registered shutdown hooks
    fn hook_count(&self) -> usize;
}

/// Individual shutdown hook for a component
#[async_trait]
pub trait ShutdownHook: Send + Sync {
    /// Execute the shutdown procedure for this component
    ///
    /// Should be idempotent - safe to call multiple times.
    async fn shutdown(&self) -> Result<(), A2AError>;

    /// Get a descriptive name for this component
    fn name(&self) -> &str;
}

/// Signal listener for OS signals (SIGTERM, SIGINT)
#[async_trait]
pub trait SignalListener: Send + Sync {
    /// Wait for shutdown signal (SIGTERM or SIGINT)
    ///
    /// Blocks until a signal is received.
    async fn wait_for_signal(&self) -> Result<(), A2AError>;

    /// Check if a signal has been received without blocking
    async fn signal_received(&self) -> bool;
}
