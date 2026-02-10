//! Event handler for marketplace procurement events

use crate::domain::{ApproveAccountRequest, EntitlementEvent, EntitlementEventType};
use crate::port::AccountApprover;
use std::sync::Arc;
use tracing::{info, warn};

/// Handles entitlement events from Cloud Marketplace
pub struct MarketplaceEventHandler<A: AccountApprover> {
    account_approver: Arc<A>,
    auto_approve: bool,
}

impl<A: AccountApprover> MarketplaceEventHandler<A> {
    /// Create a new event handler
    ///
    /// # Arguments
    ///
    /// * `account_approver` - Account approver implementation
    /// * `auto_approve` - Whether to automatically approve new entitlements
    pub fn new(account_approver: Arc<A>, auto_approve: bool) -> Self {
        Self {
            account_approver,
            auto_approve,
        }
    }

    /// Handle an entitlement event
    ///
    /// # Arguments
    ///
    /// * `event` - The entitlement event to handle
    ///
    /// # Returns
    ///
    /// Ok(()) if the event was handled successfully, Err otherwise
    pub async fn handle(
        &self,
        event: EntitlementEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Handling event: {:?} for entitlement: {}",
            event.event_type, event.entitlement
        );

        match event.event_type {
            EntitlementEventType::EntitlementOfferAccepted => {
                self.handle_offer_accepted(&event).await?;
            }
            EntitlementEventType::EntitlementActive => {
                info!("Entitlement {} is now active", event.entitlement);
            }
            EntitlementEventType::EntitlementCancelled => {
                info!("Entitlement {} was cancelled", event.entitlement);
            }
            EntitlementEventType::EntitlementPlanChanged => {
                info!("Entitlement {} plan changed", event.entitlement);
            }
            EntitlementEventType::EntitlementDeleted => {
                info!("Entitlement {} was deleted", event.entitlement);
            }
            EntitlementEventType::Unknown => {
                warn!("Unknown event type for entitlement {}", event.entitlement);
            }
        }

        Ok(())
    }

    /// Handle an offer accepted event
    async fn handle_offer_accepted(
        &self,
        event: &EntitlementEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Entitlement offer accepted: {}, fetching details",
            event.entitlement
        );

        // Get the entitlement details
        let entitlement = self
            .account_approver
            .get_entitlement(&event.entitlement)
            .await?;

        info!(
            "Retrieved entitlement: account={}, product={}",
            entitlement.account, entitlement.product
        );

        // Get the associated account
        let account = self
            .account_approver
            .get_account(&entitlement.account)
            .await?;

        info!(
            "Retrieved account: name={}, state={:?}",
            account.name, account.state
        );

        // If auto-approve is enabled and account is pending, approve it
        if self.auto_approve && account.state == crate::domain::AccountState::AccountStatePending {
            info!("Auto-approving account: {}", account.name);

            let request = ApproveAccountRequest::default();
            let approved_account = self
                .account_approver
                .approve_account(&account.name, &request)
                .await?;

            info!(
                "Account approved: name={}, state={:?}",
                approved_account.name, approved_account.state
            );
        } else {
            info!(
                "Account {} not auto-approved (auto_approve={}, state={:?})",
                account.name, self.auto_approve, account.state
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Account, AccountState, Entitlement, EntitlementState};
    use crate::port::{AccountApprover, AccountApproverError, AccountApproverResult};
    use async_trait::async_trait;
    use chrono::Utc;

    struct MockAccountApprover {
        should_auto_approve: bool,
    }

    #[async_trait]
    impl AccountApprover for MockAccountApprover {
        async fn get_entitlement(&self, _name: &str) -> AccountApproverResult<Entitlement> {
            Ok(Entitlement {
                name: "providers/test/entitlements/123".to_string(),
                account: "providers/test/accounts/456".to_string(),
                provider: "providers/test".to_string(),
                product: "test-product".to_string(),
                plan: "test-plan".to_string(),
                new_pending_plan: None,
                state: EntitlementState::EntitlementActivationRequested,
                create_time: Some(Utc::now()),
                update_time: Some(Utc::now()),
            })
        }

        async fn get_account(&self, _name: &str) -> AccountApproverResult<Account> {
            Ok(Account {
                name: "providers/test/accounts/456".to_string(),
                provider: "providers/test".to_string(),
                state: AccountState::AccountStatePending,
                input_properties: Default::default(),
                create_time: Some(Utc::now()),
                update_time: Some(Utc::now()),
            })
        }

        async fn approve_account(
            &self,
            name: &str,
            _request: &ApproveAccountRequest,
        ) -> AccountApproverResult<Account> {
            if !self.should_auto_approve {
                return Err(AccountApproverError::InvalidState(
                    "Auto-approve disabled".to_string(),
                ));
            }

            Ok(Account {
                name: name.to_string(),
                provider: "providers/test".to_string(),
                state: AccountState::AccountStateApproved,
                input_properties: Default::default(),
                create_time: Some(Utc::now()),
                update_time: Some(Utc::now()),
            })
        }

        async fn reject_account(
            &self,
            name: &str,
            _reason: &str,
        ) -> AccountApproverResult<Account> {
            Ok(Account {
                name: name.to_string(),
                provider: "providers/test".to_string(),
                state: AccountState::AccountStateRejected,
                input_properties: Default::default(),
                create_time: Some(Utc::now()),
                update_time: Some(Utc::now()),
            })
        }
    }

    #[tokio::test]
    async fn test_handle_offer_accepted_with_auto_approve() {
        let approver = Arc::new(MockAccountApprover {
            should_auto_approve: true,
        });
        let handler = MarketplaceEventHandler::new(approver, true);

        let event = EntitlementEvent {
            event_type: EntitlementEventType::EntitlementOfferAccepted,
            entitlement: "providers/test/entitlements/123".to_string(),
            event_timestamp: Utc::now(),
        };

        let result = handler.handle(event).await;
        assert!(result.is_ok());
    }
}
