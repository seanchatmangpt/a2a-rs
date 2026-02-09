//! Domain types for osiris-marketplace
//!
//! Pure types with no external dependencies. This layer defines the core
//! business entities from the Google Cloud Marketplace Partner API.

pub mod entitlement;

pub use entitlement::{
    Account, AccountState, ApproveAccountRequest, ApproveAccountResponse, Entitlement,
    EntitlementEvent, EntitlementEventType, EntitlementState, PubSubMessage,
};
