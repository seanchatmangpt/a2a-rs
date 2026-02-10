//! High-level service for marketplace integration

use crate::application::MarketplaceEventHandler;
use crate::port::{AccountApprover, EventConsumer};
use std::sync::Arc;
use tracing::{error, info};

/// High-level marketplace service
pub struct MarketplaceService<A: AccountApprover, E: EventConsumer> {
    event_consumer: Arc<E>,
    event_handler: Arc<MarketplaceEventHandler<A>>,
}

impl<A: AccountApprover + 'static, E: EventConsumer + 'static> MarketplaceService<A, E> {
    /// Create a new marketplace service
    ///
    /// # Arguments
    ///
    /// * `event_consumer` - Event consumer implementation (e.g., Pub/Sub)
    /// * `account_approver` - Account approver implementation (e.g., Procurement API)
    /// * `auto_approve` - Whether to automatically approve new entitlements
    pub fn new(event_consumer: Arc<E>, account_approver: Arc<A>, auto_approve: bool) -> Self {
        let event_handler = Arc::new(MarketplaceEventHandler::new(account_approver, auto_approve));

        Self {
            event_consumer,
            event_handler,
        }
    }

    /// Start the service and begin consuming events
    ///
    /// This method runs indefinitely, consuming events from the event consumer
    /// and processing them with the event handler.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting marketplace service");

        let handler = Arc::clone(&self.event_handler);

        self.event_consumer
            .consume(move |event| {
                let handler = Arc::clone(&handler);
                async move {
                    if let Err(e) = handler.handle(event).await {
                        error!("Failed to handle event: {}", e);
                        return Err(e);
                    }
                    Ok(())
                }
            })
            .await?;

        Ok(())
    }
}
