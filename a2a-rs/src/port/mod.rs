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

// Business capability ports (focused domain interfaces)
pub mod authenticator;
pub mod message_handler;
pub mod notification_manager;
pub mod streaming_handler;
pub mod task_manager;

// Re-export business capability interfaces
pub use authenticator::{AuthContext, AuthContextExtractor, AuthPrincipal, Authenticator};

#[cfg(feature = "auth")]
pub use authenticator::CompositeAuthenticator;

#[cfg(feature = "server")]
pub use message_handler::AsyncMessageHandler;
pub use message_handler::MessageHandler;

#[cfg(feature = "server")]
pub use notification_manager::AsyncNotificationManager;
pub use notification_manager::NotificationManager;

#[cfg(feature = "server")]
pub use streaming_handler::{
    AsyncStreamingHandler, StreamingHandler, Subscriber as StreamingSubscriber, UpdateEvent,
};

#[cfg(feature = "server")]
pub use task_manager::AsyncTaskManager;
pub use task_manager::TaskManager;
