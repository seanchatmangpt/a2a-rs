//! Domain types for Google Cloud Marketplace entitlement events.
//!
//! These types represent the Partner Pub/Sub events and procurement resources
//! as defined in the Cloud Commerce Partner Procurement API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Entitlement event types from Partner Pub/Sub
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntitlementEventType {
    /// Triggered when a customer accepts an offer
    EntitlementOfferAccepted,
    /// Triggered when an entitlement becomes active
    EntitlementActive,
    /// Triggered when an entitlement is cancelled
    EntitlementCancelled,
    /// Triggered when an entitlement plan changes
    EntitlementPlanChanged,
    /// Triggered when an entitlement is deleted
    EntitlementDeleted,
    /// Unknown event type
    #[serde(other)]
    Unknown,
}

/// Partner Pub/Sub message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PubSubMessage {
    /// Base64-encoded message data
    pub data: String,
    /// Message attributes
    #[serde(default)]
    pub attributes: std::collections::HashMap<String, String>,
    /// Message ID
    pub message_id: String,
    /// Publish time
    pub publish_time: DateTime<Utc>,
}

/// Entitlement event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementEvent {
    /// Event type
    pub event_type: EntitlementEventType,
    /// Entitlement resource name (e.g., "providers/{provider}/entitlements/{entitlement}")
    pub entitlement: String,
    /// Event timestamp
    pub event_timestamp: DateTime<Utc>,
}

/// Account resource state in the Procurement API
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccountState {
    /// Account is pending approval
    AccountStatePending,
    /// Account has been approved
    AccountStateApproved,
    /// Account has been rejected
    AccountStateRejected,
    /// Account has been deleted
    AccountStateDeleted,
    /// Unknown state
    #[serde(other)]
    AccountStateUnspecified,
}

/// Account resource from the Procurement API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Resource name (e.g., "providers/{provider}/accounts/{account}")
    pub name: String,
    /// Provider that this account belongs to
    pub provider: String,
    /// State of the account
    pub state: AccountState,
    /// Input properties from the customer
    #[serde(default)]
    pub input_properties: std::collections::HashMap<String, String>,
    /// Account creation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<DateTime<Utc>>,
    /// Account last update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<DateTime<Utc>>,
}

/// Entitlement resource from the Procurement API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entitlement {
    /// Resource name (e.g., "providers/{provider}/entitlements/{entitlement}")
    pub name: String,
    /// Account associated with this entitlement
    pub account: String,
    /// Provider that this entitlement belongs to
    pub provider: String,
    /// Product external name (SKU)
    pub product: String,
    /// Plan identifier
    pub plan: String,
    /// New pending plan (if plan change is in progress)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_pending_plan: Option<String>,
    /// State of the entitlement
    pub state: EntitlementState,
    /// Entitlement creation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<DateTime<Utc>>,
    /// Entitlement last update time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_time: Option<DateTime<Utc>>,
}

/// Entitlement state in the Procurement API
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntitlementState {
    /// Entitlement is pending
    EntitlementActivationRequested,
    /// Entitlement is active
    EntitlementActive,
    /// Plan change is pending
    EntitlementPlanChangeRequested,
    /// Plan change has been approved
    EntitlementPlanChangeApproved,
    /// Entitlement is pending cancellation
    EntitlementPendingCancellation,
    /// Entitlement is cancelled
    EntitlementCancelled,
    /// Entitlement is deleted
    EntitlementDeleted,
    /// Unknown state
    #[serde(other)]
    EntitlementStateUnspecified,
}

/// Request to approve an account
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveAccountRequest {
    /// Approval name (format: "approved")
    pub approval_name: String,
    /// Optional properties to set on approval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, String>>,
    /// Optional reason for approval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Default for ApproveAccountRequest {
    fn default() -> Self {
        Self {
            approval_name: "approved".to_string(),
            properties: None,
            reason: None,
        }
    }
}

/// Response from approving an account
///
/// Empty response - approval is confirmed via account state change
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveAccountResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_serialization() {
        let event = EntitlementEventType::EntitlementOfferAccepted;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""ENTITLEMENT_OFFER_ACCEPTED""#);
    }

    #[test]
    fn test_account_state_serialization() {
        let state = AccountState::AccountStateApproved;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, r#""ACCOUNT_STATE_APPROVED""#);
    }

    #[test]
    fn test_approve_request_default() {
        let req = ApproveAccountRequest::default();
        assert_eq!(req.approval_name, "approved");
        assert!(req.properties.is_none());
        assert!(req.reason.is_none());
    }
}
