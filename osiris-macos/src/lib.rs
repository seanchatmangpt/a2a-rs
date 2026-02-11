//! # osiris-macos - macOS Actuator Agent
//!
//! Bounded real-world actuation agent for macOS, implementing the A2A Protocol.
//!
//! ## Architecture
//!
//! Follows hexagonal architecture:
//! - **domain/**: Pure types (ActuationCommand, ActuationType, ActuationBounds)
//! - **port/**: Trait definitions (Actuator, ConfirmationProvider, CapabilityProvider)
//! - **adapter/**: Platform-specific implementations (MacOSActuator, CliConfirmationProvider)
//!
//! ## Features
//!
//! - Launch applications with bounded permissions
//! - Execute AppleScripts with safety constraints
//! - User confirmation flow for sensitive operations
//! - Timeout and path restrictions
//! - Status tracking for long-running actuations
//!
//! ## Platform Support
//!
//! Only compiles on macOS. On other platforms, stub implementations return `UnsupportedPlatform` errors.
//!
//! ## Example
//!
//! ```rust,no_run
//! use osiris_macos::{MacOSActuator, ActuationCommand, ActuationType, ActuationBounds};
//! use osiris_macos::port::Actuator;
//!
//! #[tokio::main]
//! async fn main() {
//!     let actuator = MacOSActuator::new();
//!
//!     let command = ActuationCommand {
//!         id: "cmd-001".to_string(),
//!         command_type: ActuationType::LaunchApplication,
//!         parameters: serde_json::json!({
//!             "application": "TextEdit"
//!         }),
//!         bounds: ActuationBounds::default(),
//!     };
//!
//!     let result = actuator.execute(command).await;
//!     println!("Result: {:?}", result);
//! }
//! ```

#![warn(missing_docs)]

/// Domain types - pure, platform-agnostic
pub mod domain;
pub use domain::error;

/// Port traits - interfaces for actuation
pub mod port;

/// Adapter implementations - platform-specific
pub mod adapter;

// Re-export commonly used types
pub use domain::{
    ActuationBounds, ActuationCommand, ActuationError, ActuationOutcome, ActuationResult,
    ActuationStatus, ActuationType,
};

pub use port::{Actuator, CapabilityProvider, ConfirmationProvider};

#[cfg(target_os = "macos")]
pub use adapter::macos::{CliConfirmationProvider, MacOSActuator};

#[cfg(not(target_os = "macos"))]
pub use adapter::stub::{StubActuator, StubConfirmationProvider};
