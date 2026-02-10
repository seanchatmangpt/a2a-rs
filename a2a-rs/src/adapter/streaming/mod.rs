//! Streaming adapter implementations

#[cfg(feature = "streaming")]
pub mod sse;

#[cfg(feature = "zerocopy")]
pub mod zerocopy_handler;

// Re-exports
#[cfg(feature = "streaming")]
pub use sse::{SseConfig, SseStreamingHandler, create_sse_stream, task_sse_stream};
