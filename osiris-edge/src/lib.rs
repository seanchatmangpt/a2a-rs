//! Osiris-Edge: Edge-optimized A2A agent runtime
//!
//! Provides Kanban-style WIP (Work-in-Progress) limiting for bounded concurrency
//! and deterministic overload protection.
//!
//! # Features
//!
//! - **Hard WIP limits**: Semaphore-based hard cap on concurrent work
//! - **Deterministic rejection**: No queuing - immediate failure when at capacity
//! - **Bounded response times**: Prevents unbounded queueing delays
//! - **Zero-cost abstraction**: Port/adapter pattern with minimal overhead
//!
//! # Example
//!
//! ```no_run
//! use osiris_edge::{KanbanWipGate, AsyncWipGate};
//!
//! # async fn example() {
//! // Create a gate allowing max 5 concurrent work items
//! let gate = KanbanWipGate::new(5);
//!
//! // Try to execute work
//! match gate.try_acquire().await {
//!     Ok(permit) => {
//!         // Do work while holding permit
//!         println!("Working...");
//!         // Permit auto-released on drop
//!     }
//!     Err(e) => {
//!         // WIP limit reached - emit refusal receipt
//!         eprintln!("Work rejected: {}", e);
//!     }
//! }
//!
//! // Or use the execute helper
//! let result = gate.execute(|| async {
//!     // Do work
//!     Ok::<_, osiris_edge::WipError>(42)
//! }).await;
//! # }
//! ```

pub mod adapter;
pub mod domain;
pub mod port;

// Re-export core types
pub use adapter::{KanbanWipGate, WorkspaceNormalizer};
pub use domain::{EventType, PacketContext, PacketPayload, PacketSource, TypedPacket, WipError};
pub use port::{AsyncWipGate, NormalizationError, PacketNormalizer, WipGate, WipPermit};
