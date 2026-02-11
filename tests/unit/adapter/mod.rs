//! Adapter implementation unit tests
//!
//! This module contains comprehensive unit tests for all adapter implementations:
//! - HTTP client adapter
//! - HTTP server adapter
//! - WebSocket client adapter
//! - WebSocket server adapter
//! - Authentication adapters
//!
//! Tests focus on business logic, error handling, and edge cases
//! using mock implementations of ports.

mod auth_adapter_test;
mod http_client_test;
mod http_server_test;
mod websocket_client_test;
mod websocket_server_test;
