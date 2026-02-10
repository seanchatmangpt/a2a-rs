# osiris-macos

macOS Actuator Agent for the A2A Protocol - bounded real-world actuation.

## Overview

`osiris-macos` is a specialized A2A agent implementation that provides bounded, safe actuation capabilities on macOS systems. It enables controlled interaction with the macOS environment through a hexagonal architecture that separates concerns and ensures safety through explicit bounds.

## Architecture

Follows hexagonal (ports and adapters) architecture:

- **domain/**: Pure types defining actuation commands, bounds, and results
- **port/**: Trait definitions (`Actuator`, `ConfirmationProvider`, `CapabilityProvider`)
- **adapter/**: Platform-specific implementations
  - `macos.rs`: macOS-specific actuator using native commands
  - `stub.rs`: Stub implementation for non-macOS platforms

## Features

- **Launch Applications**: Start macOS applications with permission constraints
- **Execute AppleScript**: Run AppleScript commands with safety bounds
- **Safety Bounds**: All actuations respect configured limits:
  - Timeout constraints
  - Allowed application lists
  - Path restrictions
  - User confirmation requirements
  - Destructive operation controls

## Platform Support

- **macOS**: Full implementation using `osascript`, `open`, and macOS APIs
- **Other platforms**: Stub implementation returns `UnsupportedPlatform` errors

## Usage

### As a Library

```rust
use osiris_macos::{MacOSActuator, ActuationCommand, ActuationType, ActuationBounds};
use osiris_macos::port::Actuator;

#[tokio::main]
async fn main() {
    let actuator = MacOSActuator::new();

    let command = ActuationCommand {
        id: "cmd-001".to_string(),
        command_type: ActuationType::LaunchApplication,
        parameters: serde_json::json!({
            "application": "TextEdit"
        }),
        bounds: ActuationBounds::default(),
    };

    let result = actuator.execute(command).await;
    println!("Result: {:?}", result);
}
```

### As a Daemon

```bash
cargo run -p osiris-macos
```

The daemon will:
1. Initialize the actuator
2. Run a capability test
3. Wait for shutdown signal (Ctrl+C)

## Safety Model

All actuations are subject to `ActuationBounds`:

```rust
ActuationBounds {
    timeout_seconds: 30,                    // Maximum execution time
    allowed_applications: Some(vec![...]),  // Whitelist of apps
    allowed_paths: Some(vec![...]),         // Whitelist of paths
    require_confirmation: true,             // User approval required
    allow_destructive: false,               // Block dangerous operations
}
```

## Actuation Types

- `LaunchApplication`: Start a macOS application
- `ExecuteAppleScript`: Run AppleScript code
- `KeyboardInput`: Simulate keyboard input (planned)
- `MouseAction`: Simulate mouse actions (planned)
- `FileSystemOperation`: File/directory operations (planned)
- `ProcessManagement`: Manage processes (planned)
- `SystemPreference`: Change system settings (planned)

## Dependencies

- `a2a-rs`: Core A2A protocol implementation
- `tokio`: Async runtime
- `serde`, `serde_json`: Serialization
- `thiserror`: Error handling
- `tracing`: Logging
- Platform-specific (macOS):
  - `objc2`: Objective-C bindings
  - `core-foundation`: Core Foundation bindings
  - `core-graphics`: Core Graphics bindings

## Development

```bash
# Check compilation
cargo check -p osiris-macos

# Run tests
cargo test -p osiris-macos

# Run the daemon
cargo run -p osiris-macos
```

## License

MIT
