/// Domain layer - Pure types for macOS actuation
/// Zero external dependencies, cross-platform type definitions
pub mod actuator;
pub mod error;

pub use actuator::*;
pub use error::*;
