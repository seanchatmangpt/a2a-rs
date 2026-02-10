use crate::domain::core::agent::AgentCard;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Health status of a registered agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HealthStatus {
    /// Agent is healthy and operational
    Healthy,
    /// Agent is degraded but still functioning
    Degraded,
    /// Agent is unhealthy or unreachable
    Unhealthy,
    /// Health status unknown
    Unknown,
}

/// Information about a registered agent in the service registry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistration {
    /// Unique identifier for this agent instance
    pub agent_id: String,
    /// The agent's card containing capabilities and metadata
    pub agent_card: AgentCard,
    /// Endpoint URL where the agent can be reached
    pub endpoint: String,
    /// Current health status
    pub health_status: HealthStatus,
    /// When this registration was created
    pub registered_at: DateTime<Utc>,
    /// When this registration was last updated
    pub updated_at: DateTime<Utc>,
    /// When this registration expires (TTL)
    pub expires_at: DateTime<Utc>,
    /// Additional metadata for this registration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Criteria for querying agents from the registry
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentQueryCriteria {
    /// Filter by capability tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    /// Filter by skill tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_tags: Option<Vec<String>>,
    /// Filter by health status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_status: Option<HealthStatus>,
    /// Filter by protocol version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    /// Filter by transport protocol
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    /// Filter by metadata key-value pairs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

impl AgentQueryCriteria {
    /// Create a new empty query criteria
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by capability tags
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = Some(capabilities);
        self
    }

    /// Filter by skill tags
    pub fn with_skill_tags(mut self, skill_tags: Vec<String>) -> Self {
        self.skill_tags = Some(skill_tags);
        self
    }

    /// Filter by health status
    pub fn with_health_status(mut self, health_status: HealthStatus) -> Self {
        self.health_status = Some(health_status);
        self
    }

    /// Filter by protocol version
    pub fn with_protocol_version(mut self, protocol_version: String) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }

    /// Filter by transport protocol
    pub fn with_transport(mut self, transport: String) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Filter by metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Result of a health check operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckResult {
    /// Agent identifier
    pub agent_id: String,
    /// Health status determined by the check
    pub status: HealthStatus,
    /// When the check was performed
    pub checked_at: DateTime<Utc>,
    /// Optional message describing the health status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time_ms: Option<u64>,
}

/// Statistics about the agent registry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryStats {
    /// Total number of registered agents
    pub total_agents: usize,
    /// Number of healthy agents
    pub healthy_agents: usize,
    /// Number of degraded agents
    pub degraded_agents: usize,
    /// Number of unhealthy agents
    pub unhealthy_agents: usize,
    /// Number of agents with unknown health
    pub unknown_agents: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::core::agent::{AgentCapabilities, AgentSkill};

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Healthy\"");

        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, HealthStatus::Healthy);
    }

    #[test]
    fn test_query_criteria_builder() {
        let criteria = AgentQueryCriteria::new()
            .with_capabilities(vec!["streaming".to_string()])
            .with_health_status(HealthStatus::Healthy)
            .with_protocol_version("0.3.0".to_string());

        assert_eq!(criteria.capabilities, Some(vec!["streaming".to_string()]));
        assert_eq!(criteria.health_status, Some(HealthStatus::Healthy));
        assert_eq!(criteria.protocol_version, Some("0.3.0".to_string()));
    }

    #[test]
    fn test_agent_registration_serialization() {
        let agent_card = AgentCard::builder()
            .name("Test Agent".to_string())
            .description("A test agent".to_string())
            .url("https://example.com".to_string())
            .version("1.0.0".to_string())
            .capabilities(AgentCapabilities::default())
            .skills(vec![])
            .build();

        let registration = AgentRegistration {
            agent_id: "agent-123".to_string(),
            agent_card,
            endpoint: "https://example.com/api".to_string(),
            health_status: HealthStatus::Healthy,
            registered_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            metadata: None,
        };

        let json = serde_json::to_value(&registration).unwrap();
        assert_eq!(json["agentId"], "agent-123");
        assert_eq!(json["healthStatus"], "Healthy");
    }
}
