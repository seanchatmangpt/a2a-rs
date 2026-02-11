//! Unit tests for port trait definitions
//!
//! This module tests the contract and behavior of all port traits defined in a2a-rs/src/port/.
//!
//! ## Test Organization
//!
//! - `task_manager_test.rs` - Tests for `AsyncTaskManager` port
//! - `message_handler_test.rs` - Tests for `AsyncMessageHandler` port
//! - `notification_manager_test.rs` - Tests for `AsyncNotificationManager` port
//! - `streaming_handler_test.rs` - Tests for `AsyncStreamingHandler` port
//! - `authenticator_test.rs` - Tests for `Authenticator` and related auth ports
//! - `memory_store_test.rs` - Tests for `MemoryStore` port

mod task_manager_test;
mod message_handler_test;
mod notification_manager_test;
mod streaming_handler_test;
mod authenticator_test;
mod memory_store_test;
