/// Stub implementation for non-macOS platforms
use crate::domain::{
    ActuationCommand, ActuationError, ActuationOutcome, ActuationStatus, ActuationType,
};
use crate::port::{Actuator, CapabilityProvider, ConfirmationProvider};
use async_trait::async_trait;

/// Stub actuator that returns errors on non-macOS platforms
#[derive(Debug, Default)]
pub struct StubActuator;

impl StubActuator {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Actuator for StubActuator {
    async fn execute(
        &self,
        _command: ActuationCommand,
    ) -> crate::domain::ActuationResult<ActuationOutcome> {
        Err(ActuationError::UnsupportedPlatform)
    }

    async fn check_permission(
        &self,
        _command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool> {
        Err(ActuationError::UnsupportedPlatform)
    }

    async fn get_status(
        &self,
        _command_id: &str,
    ) -> crate::domain::ActuationResult<ActuationStatus> {
        Err(ActuationError::UnsupportedPlatform)
    }

    async fn cancel(&self, _command_id: &str) -> crate::domain::ActuationResult<()> {
        Err(ActuationError::UnsupportedPlatform)
    }
}

#[async_trait]
impl CapabilityProvider for StubActuator {
    async fn get_capabilities(&self) -> Vec<ActuationType> {
        vec![]
    }

    async fn has_capability(&self, _capability: &ActuationType) -> bool {
        false
    }
}

/// Stub confirmation provider
#[derive(Debug)]
pub struct StubConfirmationProvider;

#[async_trait]
impl ConfirmationProvider for StubConfirmationProvider {
    async fn request_confirmation(
        &self,
        _command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool> {
        Err(ActuationError::UnsupportedPlatform)
    }
}
