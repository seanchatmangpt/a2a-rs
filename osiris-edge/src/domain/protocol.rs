//! Domain types for protocol detection and bridging
//!
//! Defines the protocol types (MCP, A2A) and related domain logic for the unified server.

use serde::{Deserialize, Serialize};

/// Supported protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// Model Context Protocol (MCP)
    Mcp,
    /// Agent-to-Agent Protocol (A2A)
    A2a,
}

impl Protocol {
    /// Returns the protocol name as a string
    pub fn as_str(&self) -> &str {
        match self {
            Protocol::Mcp => "mcp",
            Protocol::A2a => "a2a",
        }
    }
}

/// Protocol detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedProtocol {
    /// The detected protocol
    pub protocol: Protocol,
    /// Confidence level (0.0 to 1.0)
    pub confidence: u8,
    /// Detection method used
    pub method: DetectionMethod,
}

impl DetectedProtocol {
    /// Create a new detected protocol with high confidence
    pub fn high_confidence(protocol: Protocol, method: DetectionMethod) -> Self {
        Self {
            protocol,
            confidence: 100,
            method,
        }
    }

    /// Create a new detected protocol with medium confidence
    pub fn medium_confidence(protocol: Protocol, method: DetectionMethod) -> Self {
        Self {
            protocol,
            confidence: 70,
            method,
        }
    }

    /// Create a new detected protocol with low confidence
    pub fn low_confidence(protocol: Protocol, method: DetectionMethod) -> Self {
        Self {
            protocol,
            confidence: 40,
            method,
        }
    }
}

/// Detection method used to identify the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    /// Detected from URL path
    Path,
    /// Detected from HTTP headers
    Header,
    /// Detected from Content-Type
    ContentType,
    /// Detected from request body structure
    Body,
    /// Fallback/default protocol
    Default,
}

impl DetectionMethod {
    /// Returns the detection method name as a string
    pub fn as_str(&self) -> &str {
        match self {
            DetectionMethod::Path => "path",
            DetectionMethod::Header => "header",
            DetectionMethod::ContentType => "content-type",
            DetectionMethod::Body => "body",
            DetectionMethod::Default => "default",
        }
    }
}

/// Bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeConfig {
    /// Enable MCP-to-A2A bridging
    pub enable_mcp_to_a2a: bool,
    /// Enable A2A-to-MCP bridging
    pub enable_a2a_to_mcp: bool,
    /// A2A agent URL for MCP clients to invoke
    pub a2a_agent_url: Option<String>,
    /// MCP server URL for A2A clients to use
    pub mcp_server_url: Option<String>,
    /// Maximum concurrent bridge operations
    pub max_concurrent_bridges: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enable_mcp_to_a2a: true,
            enable_a2a_to_mcp: true,
            a2a_agent_url: None,
            mcp_server_url: None,
            max_concurrent_bridges: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_as_str() {
        assert_eq!(Protocol::Mcp.as_str(), "mcp");
        assert_eq!(Protocol::A2a.as_str(), "a2a");
    }

    #[test]
    fn test_detected_protocol_confidence() {
        let high = DetectedProtocol::high_confidence(Protocol::Mcp, DetectionMethod::Path);
        assert_eq!(high.confidence, 100);

        let medium = DetectedProtocol::medium_confidence(Protocol::A2a, DetectionMethod::Header);
        assert_eq!(medium.confidence, 70);

        let low = DetectedProtocol::low_confidence(Protocol::Mcp, DetectionMethod::Body);
        assert_eq!(low.confidence, 40);
    }

    #[test]
    fn test_detection_method_as_str() {
        assert_eq!(DetectionMethod::Path.as_str(), "path");
        assert_eq!(DetectionMethod::Header.as_str(), "header");
        assert_eq!(DetectionMethod::ContentType.as_str(), "content-type");
        assert_eq!(DetectionMethod::Body.as_str(), "body");
        assert_eq!(DetectionMethod::Default.as_str(), "default");
    }

    #[test]
    fn test_bridge_config_default() {
        let config = BridgeConfig::default();
        assert!(config.enable_mcp_to_a2a);
        assert!(config.enable_a2a_to_mcp);
        assert_eq!(config.max_concurrent_bridges, 100);
    }
}
