/*!
# osiris-macos daemon

macOS Actuator Agent daemon implementing the A2A Protocol.

Runs as a background service, exposing actuation capabilities via A2A protocol.
*/

#[cfg(target_os = "macos")]
use osiris_macos::{
    ActuationBounds, ActuationCommand, ActuationType, Actuator, CliConfirmationProvider,
    MacOSActuator,
};
#[cfg(target_os = "macos")]
use std::sync::Arc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting osiris-macos daemon");

    #[cfg(not(target_os = "macos"))]
    {
        error!("osiris-macos can only run on macOS");
        return Err("Unsupported platform".into());
    }

    #[cfg(target_os = "macos")]
    {
        // Create actuator with confirmation provider
        let confirmation_provider = Arc::new(CliConfirmationProvider);
        let actuator = Arc::new(MacOSActuator::with_confirmation(confirmation_provider));

        info!(
            "Osiris macOS Actuator v{} initialized",
            env!("CARGO_PKG_VERSION")
        );
        info!("Capabilities: Launch Applications, Execute AppleScript");

        // Demonstrate capabilities with a simple test
        info!("Running capability test...");

        let test_command = ActuationCommand {
            id: "test-001".to_string(),
            command_type: ActuationType::ExecuteAppleScript,
            parameters: serde_json::json!({
                "script": "return \"Osiris macOS Actuator is ready\""
            }),
            bounds: ActuationBounds::default(),
        };

        match actuator.execute(test_command).await {
            Ok(result) => {
                if result.success {
                    info!("Capability test passed: {:?}", result.output);
                } else {
                    error!("Capability test failed: {:?}", result.error);
                }
            }
            Err(e) => {
                error!("Capability test error: {}", e);
            }
        }

        info!("Osiris macOS Actuator daemon ready");
        info!("To integrate with A2A server, use osiris_macos library in your application");

        // Keep the daemon running
        tokio::signal::ctrl_c().await?;
        info!("Shutting down osiris-macos daemon");
    }

    Ok(())
}
