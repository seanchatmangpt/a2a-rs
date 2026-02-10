//! osiris-marketplace - Google Cloud Marketplace Partner Integration
//!
//! This crate provides a hexagonal architecture implementation for integrating
//! with the Google Cloud Marketplace Partner APIs:
//!
//! - Consume entitlement events from Partner Pub/Sub topic
//! - Approve account resources via Partner Procurement API
//! - Report operation usage to Cloud Service Control for billing
//!
//! ## Architecture
//!
//! - `domain/` - Core types for entitlements, accounts, events, and usage tracking
//! - `port/` - Trait definitions for EventConsumer, AccountApprover, and UsageReporter
//! - `adapter/` - Implementations using google-cloud-pubsub, reqwest, and google-servicecontrol1
//!
//! ## Feature Flags
//!
//! - `pubsub` - Enable Google Cloud Pub/Sub consumer adapter
//! - `procurement-api` - Enable Procurement API client adapter
//! - `service-control` - Enable Cloud Service Control usage reporter adapter
//! - `full` - Enable all features
//!
//! ## Example
//!
//! ```no_run
//! use osiris_marketplace::{
//!     domain::{EntitlementEvent, EntitlementEventType},
//!     port::{AccountApprover, EventConsumer},
//! };
//!
//! # #[cfg(all(feature = "pubsub", feature = "procurement-api"))]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use osiris_marketplace::adapter::{ProcurementApiClient, PubSubConsumer};
//! use osiris_marketplace::domain::ApproveAccountRequest;
//!
//! // Create clients
//! let consumer = PubSubConsumer::new(
//!     "my-project".to_string(),
//!     "marketplace-events".to_string(),
//! ).await?;
//!
//! let approver = ProcurementApiClient::new(
//!     "my-project".to_string(),
//!     "access-token".to_string(),
//! )?;
//!
//! // Consume events and approve accounts
//! consumer.consume(|event| async {
//!     if event.event_type == EntitlementEventType::EntitlementOfferAccepted {
//!         // Get entitlement and account details
//!         let entitlement = approver.get_entitlement(&event.entitlement).await?;
//!         let account = approver.get_account(&entitlement.account).await?;
//!
//!         // Approve the account
//!         let request = ApproveAccountRequest::default();
//!         approver.approve_account(&account.name, &request).await?;
//!     }
//!     Ok(())
//! }).await?;
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod application;
pub mod domain;
pub mod port;

// Re-export commonly used types
pub use domain::{
    Account, AccountState, ApproveAccountRequest, Entitlement, EntitlementEvent,
    EntitlementEventType, MetricType, OperationType, OperationUsage, PubSubMessage, UsageMetric,
    UsageReport,
};

// Re-export port traits
pub use port::{
    AccountApprover, AccountApproverError, EventConsumer, EventConsumerError, UsageReporter,
    UsageReporterError,
};

// Re-export application types
pub use application::{MarketplaceEventHandler, MarketplaceService, UsageTrackingHandler};
