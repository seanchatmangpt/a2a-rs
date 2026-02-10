//! Queue adapter implementations
//!
//! Provides concrete implementations of the MessageQueue port.

pub mod memory;

pub use memory::InMemoryMessageQueue;
