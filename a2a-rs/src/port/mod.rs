//! Ports (interfaces) for the A2A protocol
//!
//! Ports define the interfaces that our application needs, independent of implementation details.
//! They represent the "what" - what operations our application needs to perform.
//!
//! ## Organization
//!
//! - **Business capability ports**: Focused interfaces for specific business capabilities
//!   - `authenticator`: Authentication and authorization
//!   - `message_handler`: Message processing
//!   - `task_manager`: Task lifecycle management
//!   - `notification_manager`: Push notifications
//!   - `streaming_handler`: Real-time updates
//!   - `metrics_collector`: Metrics collection and observability
//!   - `connection_pool`: Connection pooling for resource management

// Business capability ports (focused domain interfaces)
pub mod admission;
pub mod authenticator;
pub mod batch_processor;
pub mod connection_pool;
pub mod memory_store;
pub mod message_handler;
pub mod message_queue;
pub mod message_store;
pub mod metrics_collector;
pub mod notification_manager;
pub mod streaming_handler;
pub mod task_manager;

#[cfg(feature = "compression")]
pub mod compression;

#[cfg(feature = "zerocopy")]
pub mod zerocopy_transport;

// Re-export business capability interfaces
pub use admission::{AdmissionController, AsyncAdmissionController};
pub use authenticator::{
    AuthContext, AuthContextExtractor, AuthPrincipal, Authenticator, CompositeAuthenticator,
};
#[cfg(feature = "server")]
pub use batch_processor::BatchProcessor;
pub use batch_processor::{BatchConfig, BatchItemResult, BatchResult};
pub use connection_pool::{ConnectionPool, PoolConfig, PoolStats};
pub use memory_store::{MemoryEntry, MemoryQuery, MemoryStats, MemoryStore};
pub use message_handler::{AsyncMessageHandler, MessageHandler};
pub use message_queue::{MessageQueue, Priority, QueueMetrics};
#[cfg(feature = "server")]
pub use message_store::{MessageQuery, MessageQueryResult, MessageStore};
pub use metrics_collector::{MetricsCollector, NoopMetricsCollector};
pub use notification_manager::{AsyncNotificationManager, NotificationManager};
pub use streaming_handler::{
    AsyncStreamingHandler, StreamingHandler, Subscriber as StreamingSubscriber, UpdateEvent,
};
pub use task_manager::{AsyncTaskManager, TaskManager};

#[cfg(feature = "zerocopy")]
pub use zerocopy_transport::{BufferStats, ZeroCopyTransport};
