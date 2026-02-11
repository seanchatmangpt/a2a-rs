//! Business logic adapter implementations

#[cfg(feature = "server")]
pub mod agent_info;
#[cfg(all(feature = "server", feature = "http-client"))]
pub mod enhanced_notification_manager;
#[cfg(feature = "server")]
pub mod firewall;
#[cfg(feature = "server")]
pub mod message_handler;
#[cfg(feature = "server")]
pub mod push_notification;
#[cfg(all(feature = "server", feature = "http-client"))]
pub mod push_notification_enhanced;
#[cfg(feature = "server")]
pub mod request_processor;

// Re-export business implementations
#[cfg(feature = "server")]
pub use agent_info::SimpleAgentInfo;
#[cfg(all(feature = "server", feature = "http-client"))]
pub use enhanced_notification_manager::{EnhancedHttpNotificationSender, EnhancedNotificationManager};
#[cfg(feature = "server")]
pub use firewall::{AdmissionConfig, DefaultAdmissionController};
#[cfg(feature = "server")]
pub use message_handler::DefaultMessageHandler;
#[cfg(all(feature = "server", feature = "http-client"))]
pub use push_notification::HttpPushNotificationSender;
#[cfg(feature = "server")]
pub use push_notification::{
    NoopPushNotificationSender, PushNotificationRegistry, PushNotificationSender,
};
#[cfg(all(feature = "server", feature = "http-client"))]
pub use push_notification_enhanced::{
    DeadLetterEntry, DeliveryStatus, EnhancedHttpPushNotificationSender,
    HttpPushNotificationConfig, InMemoryDeadLetterQueue, InMemoryDeliveryTracker,
};
#[cfg(feature = "server")]
pub use request_processor::DefaultRequestProcessor;
