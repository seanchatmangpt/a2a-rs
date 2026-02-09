//! osiris-marketplace - Google Cloud Marketplace Partner Integration
//!
//! This crate provides a hexagonal architecture implementation for integrating
//! with the Google Cloud Marketplace Partner APIs:
//!
//! - Consume entitlement events from Partner Pub/Sub topic
//! - Approve account resources via Partner Procurement API
//!
//! ## Architecture
//!
//! - `domain/` - Core types for entitlements, accounts, and events
//! - `port/` - Trait definitions for EventConsumer and AccountApprover
//! - `adapter/` - Implementations using google-cloud-pubsub and reqwest
//!
//! ## Feature Flags
//!
//! - `pubsub` - Enable Google Cloud Pub/Sub consumer adapter
//! - `procurement-api` - Enable Procurement API client adapter
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
    EntitlementEventType, PubSubMessage,
};

// Re-export port traits
pub use port::{AccountApprover, AccountApproverError, EventConsumer, EventConsumerError};

// Re-export application types
pub use application::{MarketplaceEventHandler, MarketplaceService};
