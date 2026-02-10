//! Protocol detector adapter
//!
//! Detects which protocol (MCP or A2A) an incoming request is using based on:
//! 1. URL path patterns
//! 2. HTTP headers
//! 3. Content-Type headers
//! 4. Request body structure

use crate::domain::protocol::{DetectedProtocol, DetectionMethod, Protocol};
use crate::port::ProtocolDetector;
use async_trait::async_trait;
use axum::http::{HeaderMap, Uri};

/// Path-based protocol detector
///
/// Detects protocol by examining the request path:
/// - `/mcp/*` or `/mcp` → MCP
/// - `/tasks/*` or `/.well-known/agent-card` → A2A
/// - Ambiguous paths check headers and body
pub struct PathBasedDetector {
    default: Protocol,
}

impl PathBasedDetector {
    /// Create a new path-based detector with A2A as default
    pub fn new() -> Self {
        Self {
            default: Protocol::A2a,
        }
    }

    /// Create a new path-based detector with specified default
    pub fn with_default(default: Protocol) -> Self {
        Self { default }
    }

    /// Detect from path alone
    fn detect_from_path(&self, path: &str) -> Option<DetectedProtocol> {
        // MCP paths
        if path.starts_with("/mcp") {
            return Some(DetectedProtocol::high_confidence(
                Protocol::Mcp,
                DetectionMethod::Path,
            ));
        }

        // A2A paths
        if path.starts_with("/tasks")
            || path.starts_with("/.well-known/agent-card")
            || path.starts_with("/agents")
            || path.starts_with("/messages")
            || path.starts_with("/notifications")
        {
            return Some(DetectedProtocol::high_confidence(
                Protocol::A2a,
                DetectionMethod::Path,
            ));
        }

        None
    }

    /// Detect from headers
    fn detect_from_headers(&self, headers: &HeaderMap) -> Option<DetectedProtocol> {
        // Check for MCP-specific headers
        if headers.contains_key("mcp-session-id") {
            return Some(DetectedProtocol::high_confidence(
                Protocol::Mcp,
                DetectionMethod::Header,
            ));
        }

        // Check Content-Type
        if let Some(content_type) = headers.get("content-type").and_then(|v| v.to_str().ok()) {
            if content_type.contains("application/json-rpc") {
                // Could be either, but slightly favor MCP
                return Some(DetectedProtocol::medium_confidence(
                    Protocol::Mcp,
                    DetectionMethod::ContentType,
                ));
            }
        }

        // Check Accept header for SSE
        if let Some(accept) = headers.get("accept").and_then(|v| v.to_str().ok()) {
            if accept.contains("text/event-stream") {
                // Both protocols support SSE, but check path or other hints
                // Slightly favor MCP for now
                return Some(DetectedProtocol::medium_confidence(
                    Protocol::Mcp,
                    DetectionMethod::Header,
                ));
            }
        }

        None
    }

    /// Detect from body structure
    fn detect_from_body(&self, body: &[u8]) -> Option<DetectedProtocol> {
        // Try to parse as JSON
        if let Ok(json_str) = std::str::from_utf8(body) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(json_str) {
                // Check for JSON-RPC 2.0 structure
                if let Some(obj) = json_val.as_object() {
                    // Check for jsonrpc field
                    if obj.contains_key("jsonrpc") {
                        // Could be either protocol (both use JSON-RPC)
                        // Check for method field to distinguish
                        if let Some(method) = obj.get("method").and_then(|m| m.as_str()) {
                            // MCP methods typically start with lowercase namespace
                            if method.starts_with("tools/")
                                || method.starts_with("resources/")
                                || method.starts_with("prompts/")
                                || method.starts_with("completion/")
                            {
                                return Some(DetectedProtocol::high_confidence(
                                    Protocol::Mcp,
                                    DetectionMethod::Body,
                                ));
                            }

                            // A2A methods
                            if method.starts_with("agent/")
                                || method.starts_with("task/")
                                || method.starts_with("message/")
                            {
                                return Some(DetectedProtocol::high_confidence(
                                    Protocol::A2a,
                                    DetectionMethod::Body,
                                ));
                            }
                        }

                        // Generic JSON-RPC, can't determine
                        return Some(DetectedProtocol::low_confidence(
                            self.default,
                            DetectionMethod::Body,
                        ));
                    }

                    // Check for A2A-specific fields
                    if obj.contains_key("task_id")
                        || obj.contains_key("taskId")
                        || obj.contains_key("agent_card")
                        || obj.contains_key("agentCard")
                    {
                        return Some(DetectedProtocol::high_confidence(
                            Protocol::A2a,
                            DetectionMethod::Body,
                        ));
                    }
                }
            }
        }

        None
    }
}

impl Default for PathBasedDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolDetector for PathBasedDetector {
    async fn detect(
        &self,
        uri: &Uri,
        headers: &HeaderMap,
        body: Option<&[u8]>,
    ) -> DetectedProtocol {
        let path = uri.path();

        // Try path-based detection first (highest confidence)
        if let Some(detected) = self.detect_from_path(path) {
            return detected;
        }

        // Try header-based detection
        if let Some(detected) = self.detect_from_headers(headers) {
            return detected;
        }

        // Try body-based detection if available
        if let Some(body_bytes) = body {
            if let Some(detected) = self.detect_from_body(body_bytes) {
                return detected;
            }
        }

        // Default fallback
        DetectedProtocol {
            protocol: self.default,
            confidence: 50,
            method: DetectionMethod::Default,
        }
    }

    fn default_protocol(&self) -> Protocol {
        self.default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Uri;

    #[tokio::test]
    async fn test_detect_mcp_from_path() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/mcp".parse().unwrap();
        let headers = HeaderMap::new();

        let result = detector.detect(&uri, &headers, None).await;
        assert_eq!(result.protocol, Protocol::Mcp);
        assert_eq!(result.method, DetectionMethod::Path);
        assert_eq!(result.confidence, 100);
    }

    #[tokio::test]
    async fn test_detect_a2a_from_path() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/tasks/send".parse().unwrap();
        let headers = HeaderMap::new();

        let result = detector.detect(&uri, &headers, None).await;
        assert_eq!(result.protocol, Protocol::A2a);
        assert_eq!(result.method, DetectionMethod::Path);
        assert_eq!(result.confidence, 100);
    }

    #[tokio::test]
    async fn test_detect_agent_card_path() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/.well-known/agent-card".parse().unwrap();
        let headers = HeaderMap::new();

        let result = detector.detect(&uri, &headers, None).await;
        assert_eq!(result.protocol, Protocol::A2a);
        assert_eq!(result.method, DetectionMethod::Path);
    }

    #[tokio::test]
    async fn test_detect_mcp_from_header() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/api/v1".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("mcp-session-id", "test-123".parse().unwrap());

        let result = detector.detect(&uri, &headers, None).await;
        assert_eq!(result.protocol, Protocol::Mcp);
        assert_eq!(result.method, DetectionMethod::Header);
        assert_eq!(result.confidence, 100);
    }

    #[tokio::test]
    async fn test_detect_mcp_from_body() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/api".parse().unwrap();
        let headers = HeaderMap::new();
        let body = br#"{"jsonrpc":"2.0","method":"tools/list","id":1}"#;

        let result = detector.detect(&uri, &headers, Some(body)).await;
        assert_eq!(result.protocol, Protocol::Mcp);
        assert_eq!(result.method, DetectionMethod::Body);
        assert_eq!(result.confidence, 100);
    }

    #[tokio::test]
    async fn test_detect_a2a_from_body() {
        let detector = PathBasedDetector::new();
        let uri: Uri = "/api".parse().unwrap();
        let headers = HeaderMap::new();
        let body = br#"{"jsonrpc":"2.0","method":"task/send","params":{"taskId":"123"}}"#;

        let result = detector.detect(&uri, &headers, Some(body)).await;
        assert_eq!(result.protocol, Protocol::A2a);
        assert_eq!(result.method, DetectionMethod::Body);
        assert_eq!(result.confidence, 100);
    }

    #[tokio::test]
    async fn test_default_fallback() {
        let detector = PathBasedDetector::with_default(Protocol::Mcp);
        let uri: Uri = "/unknown".parse().unwrap();
        let headers = HeaderMap::new();

        let result = detector.detect(&uri, &headers, None).await;
        assert_eq!(result.protocol, Protocol::Mcp);
        assert_eq!(result.method, DetectionMethod::Default);
        assert_eq!(result.confidence, 50);
    }
}
