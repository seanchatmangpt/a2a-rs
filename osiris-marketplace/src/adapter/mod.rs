//! Adapter implementations for osiris-marketplace
//!
//! Concrete implementations of port traits, feature-gated for optional dependencies.

#[cfg(feature = "procurement-api")]
pub mod procurement_api;

#[cfg(feature = "pubsub")]
pub mod pubsub_consumer;

#[cfg(feature = "service-control")]
pub mod service_control;

#[cfg(feature = "procurement-api")]
pub use procurement_api::ProcurementApiClient;

#[cfg(feature = "pubsub")]
pub use pubsub_consumer::PubSubConsumer;

#[cfg(feature = "service-control")]
pub use service_control::ServiceControlReporter;
