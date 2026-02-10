//! Port trait for protocol detection
//!
//! Defines the interface for detecting which protocol (MCP or A2A) an incoming request is using.

use crate::domain::protocol::{DetectedProtocol, Protocol};
use async_trait::async_trait;
use axum::http::{HeaderMap, Uri};

/// Protocol detector trait
///
/// Implementations detect which protocol (MCP or A2A) an incoming HTTP request is using
/// based on headers, path, content-type, and/or request body.
#[async_trait]
pub trait ProtocolDetector: Send + Sync {
    /// Detect protocol from request metadata
    ///
    /// # Arguments
    /// * `uri` - The request URI
    /// * `headers` - The request headers
    /// * `body` - Optional request body preview (first few bytes)
    ///
    /// # Returns
    /// A `DetectedProtocol` indicating which protocol was detected and confidence level
    async fn detect(&self, uri: &Uri, headers: &HeaderMap, body: Option<&[u8]>)
    -> DetectedProtocol;

    /// Get the default protocol when detection is ambiguous
    fn default_protocol(&self) -> Protocol {
        Protocol::A2a
    }
}
