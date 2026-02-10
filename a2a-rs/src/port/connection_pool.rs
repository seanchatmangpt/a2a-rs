//! Connection pool port definitions
//!
//! Defines the contract for managing pooled connections in an async environment.

use async_trait::async_trait;
use std::fmt::Debug;
use std::time::Duration;

/// Trait for connections that can be pooled
///
/// Implementations must provide health checking and recycling logic
/// to ensure connections remain valid throughout their lifecycle.
#[async_trait]
pub trait PoolableConnection: Send + Sync + Debug {
    /// Error type for connection operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Check if the connection is still healthy
    ///
    /// Should be lightweight and fast. Called before returning
    /// a connection from the pool.
    async fn is_healthy(&self) -> bool;

    /// Recycle the connection for reuse
    ///
    /// Called before returning a connection to the pool.
    /// Should reset any connection state (e.g., clear buffers, reset timeouts).
    async fn recycle(&mut self) -> Result<(), Self::Error>;
}

/// Statistics about connection pool usage
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of connections in the pool (idle + active)
    pub total_connections: usize,
    /// Number of idle connections available
    pub idle_connections: usize,
    /// Number of connections currently in use
    pub active_connections: usize,
    /// Number of times a connection was successfully acquired
    pub acquisitions: u64,
    /// Number of times acquisition timed out
    pub timeouts: u64,
}

/// Configuration for connection pool behavior
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections to maintain
    pub min_connections: usize,
    /// Maximum number of connections allowed
    pub max_connections: usize,
    /// Timeout for acquiring a connection from the pool
    pub acquire_timeout: Duration,
    /// Maximum idle time before a connection is closed
    pub idle_timeout: Option<Duration>,
    /// Interval for running health checks on idle connections
    pub health_check_interval: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
            health_check_interval: Some(Duration::from_secs(30)),
        }
    }
}

/// Port for managing a pool of connections
///
/// Provides async acquire/release semantics with configurable
/// pooling behavior.
#[async_trait]
pub trait ConnectionPool<T: PoolableConnection>: Send + Sync {
    /// Error type for pool operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Acquire a connection from the pool
    ///
    /// May create a new connection if the pool is not at capacity.
    /// Blocks until a connection is available or the acquire timeout expires.
    async fn acquire(&self) -> Result<PooledConnection<T>, Self::Error>;

    /// Get current pool statistics
    fn stats(&self) -> PoolStats;

    /// Get the pool configuration
    fn config(&self) -> &PoolConfig;

    /// Resize the pool to a new target size
    ///
    /// May close idle connections if shrinking, or create new connections if growing.
    async fn resize(&self, new_max: usize) -> Result<(), Self::Error>;

    /// Close all connections and shut down the pool
    async fn shutdown(self) -> Result<(), Self::Error>;
}

/// A connection acquired from the pool
///
/// When dropped, the connection is automatically returned to the pool
/// (if still healthy) or discarded.
pub struct PooledConnection<T: PoolableConnection> {
    connection: Option<T>,
    return_fn: Option<Box<dyn FnOnce(T) + Send + 'static>>,
}

impl<T: PoolableConnection> PooledConnection<T> {
    /// Create a new pooled connection wrapper
    pub fn new(connection: T, return_fn: impl FnOnce(T) + Send + 'static) -> Self {
        Self {
            connection: Some(connection),
            return_fn: Some(Box::new(return_fn)),
        }
    }

    /// Get a reference to the underlying connection
    pub fn as_ref(&self) -> &T {
        self.connection.as_ref().expect("connection already taken")
    }

    /// Get a mutable reference to the underlying connection
    pub fn as_mut(&mut self) -> &mut T {
        self.connection.as_mut().expect("connection already taken")
    }

    /// Consume the wrapper and take ownership of the connection
    ///
    /// The connection will NOT be returned to the pool.
    pub fn into_inner(mut self) -> T {
        self.return_fn.take(); // Disable return on drop
        self.connection.take().expect("connection already taken")
    }
}

impl<T: PoolableConnection> std::ops::Deref for PooledConnection<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T: PoolableConnection> std::ops::DerefMut for PooledConnection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T: PoolableConnection> Drop for PooledConnection<T> {
    fn drop(&mut self) {
        if let (Some(connection), Some(return_fn)) = (self.connection.take(), self.return_fn.take())
        {
            return_fn(connection);
        }
    }
}

impl<T: PoolableConnection> Debug for PooledConnection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledConnection")
            .field("connection", &self.connection)
            .finish()
    }
}
