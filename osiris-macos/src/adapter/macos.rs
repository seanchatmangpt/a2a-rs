/// macOS-specific actuator implementation
use crate::domain::{
    ActuationCommand, ActuationError, ActuationOutcome, ActuationStatus, ActuationType,
};
use crate::port::{Actuator, CapabilityProvider, ConfirmationProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// macOS actuator implementation
pub struct MacOSActuator {
    /// Active actuations being tracked
    active_actuations: Arc<RwLock<HashMap<String, ActuationStatus>>>,
    /// Confirmation provider for user approval
    confirmation_provider: Option<Arc<dyn ConfirmationProvider>>,
}

impl std::fmt::Debug for MacOSActuator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacOSActuator")
            .field("active_actuations", &self.active_actuations)
            .field("confirmation_provider", &self.confirmation_provider.as_ref().map(|_| "..."))
            .finish()
    }
}

impl MacOSActuator {
    /// Create a new macOS actuator
    pub fn new() -> Self {
        Self {
            active_actuations: Arc::new(RwLock::new(HashMap::new())),
            confirmation_provider: None,
        }
    }

    /// Create a new macOS actuator with confirmation provider
    pub fn with_confirmation(confirmation_provider: Arc<dyn ConfirmationProvider>) -> Self {
        Self {
            active_actuations: Arc::new(RwLock::new(HashMap::new())),
            confirmation_provider: Some(confirmation_provider),
        }
    }

    /// Validate command against safety bounds
    async fn validate_command(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<()> {
        // Check timeout bounds
        if command.bounds.timeout_seconds > 300 {
            return Err(ActuationError::InvalidParameters(
                "Timeout cannot exceed 300 seconds".to_string(),
            ));
        }

        // Validate based on command type
        match command.command_type {
            ActuationType::LaunchApplication => {
                // Validate application bounds if specified
                if let Some(ref allowed) = command.bounds.allowed_applications {
                    let app_name = command
                        .parameters
                        .get("application")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            ActuationError::InvalidParameters(
                                "Missing application parameter".to_string(),
                            )
                        })?;

                    if !allowed.contains(&app_name.to_string()) {
                        return Err(ActuationError::NotPermitted(format!(
                            "Application '{}' not in allowed list",
                            app_name
                        )));
                    }
                }
            }
            ActuationType::FileSystemOperation => {
                if command.bounds.allow_destructive {
                    return Err(ActuationError::NotPermitted(
                        "Destructive file system operations require explicit permission"
                            .to_string(),
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Execute the actual actuation (platform-specific)
    async fn execute_platform(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<serde_json::Value> {
        match command.command_type {
            ActuationType::LaunchApplication => self.launch_application(command).await,
            ActuationType::ExecuteAppleScript => self.execute_applescript(command).await,
            _ => Err(ActuationError::Internal(format!(
                "Actuation type {:?} not yet implemented",
                command.command_type
            ))),
        }
    }

    /// Launch a macOS application
    async fn launch_application(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<serde_json::Value> {
        let app_name = command
            .parameters
            .get("application")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ActuationError::InvalidParameters("Missing application parameter".to_string())
            })?;

        tracing::info!("Launching application: {}", app_name);

        // Use macOS `open` command
        let output = tokio::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .output()
            .await
            .map_err(|e| {
                ActuationError::SystemError(format!("Failed to launch application: {}", e))
            })?;

        if output.status.success() {
            Ok(serde_json::json!({
                "application": app_name,
                "status": "launched"
            }))
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(ActuationError::ApplicationNotFound(format!(
                "Failed to launch {}: {}",
                app_name, error
            )))
        }
    }

    /// Execute an AppleScript
    async fn execute_applescript(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<serde_json::Value> {
        let script = command
            .parameters
            .get("script")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ActuationError::InvalidParameters("Missing script parameter".to_string())
            })?;

        tracing::info!("Executing AppleScript");

        // Use osascript command
        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await
            .map_err(|e| ActuationError::SystemError(format!("Failed to execute script: {}", e)))?;

        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            Ok(serde_json::json!({
                "output": result.trim()
            }))
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(ActuationError::SystemError(format!(
                "AppleScript error: {}",
                error
            )))
        }
    }
}

impl Default for MacOSActuator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Actuator for MacOSActuator {
    async fn execute(
        &self,
        command: ActuationCommand,
    ) -> crate::domain::ActuationResult<ActuationOutcome> {
        let start_time = std::time::Instant::now();

        // Update status to in progress
        {
            let mut actuations = self.active_actuations.write().await;
            actuations.insert(command.id.clone(), ActuationStatus::InProgress);
        }

        // Validate command
        if let Err(e) = self.validate_command(&command).await {
            let mut actuations = self.active_actuations.write().await;
            actuations.insert(command.id.clone(), ActuationStatus::Failed);
            return Err(e);
        }

        // Check for user confirmation if required
        if command.bounds.require_confirmation {
            if let Some(ref provider) = self.confirmation_provider {
                let mut actuations = self.active_actuations.write().await;
                actuations.insert(command.id.clone(), ActuationStatus::PendingConfirmation);
                drop(actuations);

                let confirmed = provider.request_confirmation(&command).await?;
                if !confirmed {
                    let mut actuations = self.active_actuations.write().await;
                    actuations.insert(command.id.clone(), ActuationStatus::Cancelled);
                    return Err(ActuationError::UserCancelled);
                }

                let mut actuations = self.active_actuations.write().await;
                actuations.insert(command.id.clone(), ActuationStatus::InProgress);
            }
        }

        // Execute with timeout
        let timeout = std::time::Duration::from_secs(command.bounds.timeout_seconds);
        let result = tokio::time::timeout(timeout, self.execute_platform(&command)).await;

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let mut actuations = self.active_actuations.write().await;
                actuations.insert(command.id.clone(), ActuationStatus::Completed);

                Ok(ActuationOutcome {
                    command_id: command.id,
                    success: true,
                    output: Some(output),
                    error: None,
                    execution_time_ms,
                })
            }
            Ok(Err(e)) => {
                let mut actuations = self.active_actuations.write().await;
                actuations.insert(command.id.clone(), ActuationStatus::Failed);

                Ok(ActuationOutcome {
                    command_id: command.id,
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    execution_time_ms,
                })
            }
            Err(_) => {
                let mut actuations = self.active_actuations.write().await;
                actuations.insert(command.id.clone(), ActuationStatus::Failed);

                Err(ActuationError::Timeout(command.bounds.timeout_seconds))
            }
        }
    }

    async fn check_permission(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool> {
        self.validate_command(command).await.map(|_| true)
    }

    async fn get_status(
        &self,
        command_id: &str,
    ) -> crate::domain::ActuationResult<ActuationStatus> {
        let actuations = self.active_actuations.read().await;
        actuations
            .get(command_id)
            .cloned()
            .ok_or_else(|| ActuationError::Internal(format!("Command {} not found", command_id)))
    }

    async fn cancel(&self, command_id: &str) -> crate::domain::ActuationResult<()> {
        let mut actuations = self.active_actuations.write().await;
        actuations.insert(command_id.to_string(), ActuationStatus::Cancelled);
        Ok(())
    }
}

#[async_trait]
impl CapabilityProvider for MacOSActuator {
    async fn get_capabilities(&self) -> Vec<ActuationType> {
        vec![
            ActuationType::LaunchApplication,
            ActuationType::ExecuteAppleScript,
        ]
    }

    async fn has_capability(&self, capability: &ActuationType) -> bool {
        matches!(
            capability,
            ActuationType::LaunchApplication | ActuationType::ExecuteAppleScript
        )
    }
}

/// Simple CLI-based confirmation provider
#[derive(Debug)]
pub struct CliConfirmationProvider;

#[async_trait]
impl ConfirmationProvider for CliConfirmationProvider {
    async fn request_confirmation(
        &self,
        command: &ActuationCommand,
    ) -> crate::domain::ActuationResult<bool> {
        println!("\n=== Actuation Request ===");
        println!("Type: {:?}", command.command_type);
        println!(
            "Parameters: {}",
            serde_json::to_string_pretty(&command.parameters).unwrap()
        );
        println!("========================\n");
        println!("Approve this actuation? (y/n): ");

        // For now, auto-approve in daemon mode
        // In production, this would integrate with macOS notification system
        tracing::warn!("Auto-approving actuation in daemon mode");
        Ok(true)
    }
}
