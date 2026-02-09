/// Actuator port traits
use crate::domain::{ActuationCommand, ActuationOutcome, ActuationStatus};
use async_trait::async_trait;

/// Port trait for executing bounded actuations
#[async_trait]
pub trait Actuator: Send + Sync {
    /// Execute an actuation command with safety bounds
    async fn execute(
        &self,
        command: ActuationCommand,
    ) -> crate::domain::ActuationResult<ActuationOutcome>;

    /// Check if an actuation is permitted by safety bounds
    async fn check_permission(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool>;

    /// Get the status of a running actuation
    async fn get_status(&self, command_id: &str)
    -> crate::domain::ActuationResult<ActuationStatus>;

    /// Cancel a running actuation
    async fn cancel(&self, command_id: &str) -> crate::domain::ActuationResult<()>;
}

/// Port trait for user confirmation of actuations
#[async_trait]
pub trait ConfirmationProvider: Send + Sync {
    /// Request user confirmation for an actuation
    async fn request_confirmation(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool>;
}

/// Port trait for actuation capability discovery
#[async_trait]
pub trait CapabilityProvider: Send + Sync {
    /// Get the list of supported actuation types
    async fn get_capabilities(&self) -> Vec<crate::domain::ActuationType>;

    /// Check if a specific capability is available
    async fn has_capability(&self, capability: &crate::domain::ActuationType) -> bool;
}
