/// Adapter layer - Platform-specific implementations
/// Feature-gated and uses external crates

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod stub;

// Re-export the appropriate implementation
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
pub use stub::*;
