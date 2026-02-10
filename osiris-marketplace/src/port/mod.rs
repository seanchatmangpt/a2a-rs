//! Port traits for osiris-marketplace
//!
//! Trait definitions that adapters must implement. These define the boundaries
//! of the hexagonal architecture.

pub mod account_approver;
pub mod event_consumer;
pub mod usage_reporter;

pub use account_approver::{AccountApprover, AccountApproverError, AccountApproverResult};
pub use event_consumer::{EventConsumer, EventConsumerError, EventConsumerResult};
pub use usage_reporter::{UsageReporter, UsageReporterError, UsageReporterResult};
