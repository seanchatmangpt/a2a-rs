//! Unified bidirectional bridge between MCP and A2A protocols
//!
//! Provides seamless translation allowing:
//! - MCP clients to invoke A2A agents
//! - A2A clients to use MCP tools

use crate::domain::protocol::BridgeConfig;
use a2a_mcp::{
    adapter::{AgentToToolAdapter, ToolToAgentAdapter},
    message::MessageConverter,
};
use a2a_rs::domain::{
    agent::AgentCard,
    message::Message,
    task::{Task, TaskState},
};
use rmcp::{Tool, ToolCall, ToolResponse};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Bidirectional protocol bridge
///
/// Translates between MCP and A2A protocols in both directions:
/// 1. MCP → A2A: Expose A2A agents as MCP tools
/// 2. A2A → MCP: Expose MCP tools as A2A agents
pub struct UnifiedBridge {
    /// Configuration
    config: BridgeConfig,
    /// MCP-to-A2A adapter
    agent_to_tool: Arc<AgentToToolAdapter>,
    /// A2A-to-MCP adapter
    tool_to_agent: Arc<ToolToAgentAdapter>,
    /// Message converter
    converter: Arc<MessageConverter>,
    /// Registered A2A agents (URL -> AgentCard)
    agents: Arc<RwLock<HashMap<String, AgentCard>>>,
    /// Registered MCP tools
    tools: Arc<RwLock<Vec<Tool>>>,
    /// Active bridge operations
    active_bridges: Arc<RwLock<HashMap<String, BridgeOperation>>>,
}

/// Represents an active bridge operation
#[derive(Debug, Clone)]
struct BridgeOperation {
    /// Operation ID
    id: String,
    /// Source protocol
    from: String,
    /// Target protocol
    to: String,
    /// Created timestamp
    created_at: chrono::DateTime<chrono::Utc>,
}

impl UnifiedBridge {
    /// Create a new unified bridge
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            agent_to_tool: Arc::new(AgentToToolAdapter::new()),
            tool_to_agent: Arc::new(ToolToAgentAdapter::new(
                Vec::new(),
                "Unified Bridge Agent".to_string(),
                "Exposes MCP tools as A2A agent capabilities".to_string(),
            )),
            converter: Arc::new(MessageConverter::new()),
            agents: Arc::new(RwLock::new(HashMap::new())),
            tools: Arc::new(RwLock::new(Vec::new())),
            active_bridges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an A2A agent to be exposed as MCP tools
    pub async fn register_a2a_agent(&self, url: String, agent_card: AgentCard) {
        if !self.config.enable_a2a_to_mcp {
            warn!("A2A-to-MCP bridging disabled, skipping agent registration");
            return;
        }

        info!("Registering A2A agent: {} at {}", agent_card.name, url);

        // Store agent card
        {
            let mut agents = self.agents.write().await;
            agents.insert(url.clone(), agent_card.clone());
        }

        // Generate MCP tools from agent skills
        let agent_tools = self.agent_to_tool.generate_tools(&agent_card, &url);

        // Add to tool collection
        {
            let mut tools = self.tools.write().await;
            tools.extend(agent_tools);
        }

        debug!(
            "Registered {} tools from A2A agent",
            agent_card.skills.len()
        );
    }

    /// Register an MCP tool to be exposed as A2A agent capability
    pub async fn register_mcp_tool(&self, tool: Tool) {
        if !self.config.enable_mcp_to_a2a {
            warn!("MCP-to-A2A bridging disabled, skipping tool registration");
            return;
        }

        info!("Registering MCP tool: {}", tool.name);

        let mut tools = self.tools.write().await;
        tools.push(tool);
    }

    /// Get all registered MCP tools
    pub async fn get_mcp_tools(&self) -> Vec<Tool> {
        let tools = self.tools.read().await;
        tools.clone()
    }

    /// Get the A2A agent card representing all MCP tools
    pub async fn get_agent_card(&self) -> AgentCard {
        let tools = self.tools.read().await;
        let tool_to_agent = ToolToAgentAdapter::new(
            tools.clone(),
            "Unified Bridge Agent".to_string(),
            "Exposes MCP tools as A2A agent capabilities".to_string(),
        );
        tool_to_agent.generate_agent_card()
    }

    /// Bridge an MCP tool call to an A2A agent
    ///
    /// Takes an MCP tool call and translates it to an A2A task
    pub async fn bridge_mcp_to_a2a(&self, tool_call: &ToolCall) -> Result<Task, BridgeError> {
        if !self.config.enable_mcp_to_a2a {
            return Err(BridgeError::BridgingDisabled(
                "MCP-to-A2A bridging is disabled".into(),
            ));
        }

        // Check concurrency limit
        self.check_concurrency_limit().await?;

        let operation_id = Uuid::new_v4().to_string();
        self.track_bridge_operation(&operation_id, "MCP", "A2A")
            .await;

        debug!(
            "Bridging MCP tool call to A2A: {} (op: {})",
            tool_call.method, operation_id
        );

        // Parse tool method to extract agent URL and skill name
        let (agent_url, skill_name) = self
            .agent_to_tool
            .parse_tool_method(&tool_call.method)
            .map_err(|e| BridgeError::InvalidToolMethod(e.to_string()))?;

        // Get agent card
        let agent_card = {
            let agents = self.agents.read().await;
            agents
                .get(&agent_url)
                .ok_or_else(|| BridgeError::AgentNotFound(agent_url.clone()))?
                .clone()
        };

        // Convert tool call to A2A task
        let task = self
            .agent_to_tool
            .tool_call_to_task(tool_call, &agent_card, &skill_name)
            .map_err(|e| BridgeError::Conversion(e.to_string()))?;

        self.complete_bridge_operation(&operation_id).await;

        Ok(task)
    }

    /// Bridge an A2A message to an MCP tool call
    ///
    /// Takes an A2A message and translates it to an MCP tool call
    pub async fn bridge_a2a_to_mcp(&self, message: &Message) -> Result<ToolCall, BridgeError> {
        if !self.config.enable_a2a_to_mcp {
            return Err(BridgeError::BridgingDisabled(
                "A2A-to-MCP bridging is disabled".into(),
            ));
        }

        // Check concurrency limit
        self.check_concurrency_limit().await?;

        let operation_id = Uuid::new_v4().to_string();
        self.track_bridge_operation(&operation_id, "A2A", "MCP")
            .await;

        debug!(
            "Bridging A2A message to MCP tool call (op: {})",
            operation_id
        );

        // Extract tool name and parameters from message
        let (tool_name, params) = self
            .tool_to_agent
            .extract_tool_call(message)
            .map_err(|e| BridgeError::Conversion(e.to_string()))?;

        // Create MCP tool call
        let tool_call = ToolCall {
            method: tool_name,
            params,
        };

        self.complete_bridge_operation(&operation_id).await;

        Ok(tool_call)
    }

    /// Bridge an MCP tool response back to A2A task result
    pub async fn bridge_mcp_response_to_a2a(
        &self,
        response: &ToolResponse,
        task_id: &str,
    ) -> Result<Task, BridgeError> {
        debug!("Bridging MCP response back to A2A task: {}", task_id);

        // Create agent message from tool response
        let agent_message = Message {
            role: "agent".to_string(),
            parts: vec![a2a_rs::domain::message::MessagePart::Data {
                data: response.result.clone(),
                mime_type: Some("application/json".to_string()),
            }],
        };

        // Create updated task
        let task = Task {
            id: task_id.to_string(),
            status: a2a_rs::domain::task::TaskStatus {
                state: TaskState::Completed,
                message: Some("Task completed via MCP bridge".to_string()),
            },
            messages: vec![agent_message],
            artifacts: Vec::new(),
            history_ttl: Some(3600),
            metadata: Some(serde_json::json!({
                "bridged": true,
                "bridge_direction": "mcp-to-a2a"
            })),
        };

        Ok(task)
    }

    /// Bridge an A2A task result back to MCP tool response
    pub async fn bridge_a2a_task_to_mcp(&self, task: &Task) -> Result<ToolResponse, BridgeError> {
        debug!("Bridging A2A task result back to MCP: {}", task.id);

        // Convert task to tool response
        let response = self
            .tool_to_agent
            .task_to_tool_response(task)
            .map_err(|e| BridgeError::Conversion(e.to_string()))?;

        Ok(response)
    }

    /// Get bridge statistics
    pub async fn get_statistics(&self) -> BridgeStatistics {
        let active = self.active_bridges.read().await;
        let tools_count = self.tools.read().await.len();
        let agents_count = self.agents.read().await.len();

        BridgeStatistics {
            active_operations: active.len(),
            total_tools: tools_count,
            total_agents: agents_count,
            mcp_to_a2a_enabled: self.config.enable_mcp_to_a2a,
            a2a_to_mcp_enabled: self.config.enable_a2a_to_mcp,
        }
    }

    /// Check if concurrency limit reached
    async fn check_concurrency_limit(&self) -> Result<(), BridgeError> {
        let active = self.active_bridges.read().await;
        if active.len() >= self.config.max_concurrent_bridges {
            return Err(BridgeError::ConcurrencyLimitReached(
                self.config.max_concurrent_bridges,
            ));
        }
        Ok(())
    }

    /// Track a new bridge operation
    async fn track_bridge_operation(&self, operation_id: &str, from: &str, to: &str) {
        let operation = BridgeOperation {
            id: operation_id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            created_at: chrono::Utc::now(),
        };

        let mut active = self.active_bridges.write().await;
        active.insert(operation_id.to_string(), operation);
    }

    /// Complete a bridge operation
    async fn complete_bridge_operation(&self, operation_id: &str) {
        let mut active = self.active_bridges.write().await;
        active.remove(operation_id);
    }
}

/// Bridge error types
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Bridging disabled: {0}")]
    BridgingDisabled(String),

    #[error("Invalid tool method format: {0}")]
    InvalidToolMethod(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Conversion error: {0}")]
    Conversion(String),

    #[error("Concurrency limit reached: {0}")]
    ConcurrencyLimitReached(usize),
}

/// Bridge statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStatistics {
    pub active_operations: usize,
    pub total_tools: usize,
    pub total_agents: usize,
    pub mcp_to_a2a_enabled: bool,
    pub a2a_to_mcp_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_creation() {
        let config = BridgeConfig::default();
        let bridge = UnifiedBridge::new(config);

        let stats = bridge.get_statistics().await;
        assert_eq!(stats.active_operations, 0);
        assert_eq!(stats.total_tools, 0);
        assert_eq!(stats.total_agents, 0);
        assert!(stats.mcp_to_a2a_enabled);
        assert!(stats.a2a_to_mcp_enabled);
    }

    #[tokio::test]
    async fn test_register_mcp_tool() {
        let config = BridgeConfig::default();
        let bridge = UnifiedBridge::new(config);

        let tool = Tool {
            name: "test_tool".to_string(),
            description: "Test tool".to_string(),
            parameters: None,
        };

        bridge.register_mcp_tool(tool).await;

        let stats = bridge.get_statistics().await;
        assert_eq!(stats.total_tools, 1);
    }

    #[tokio::test]
    async fn test_bridging_disabled() {
        let mut config = BridgeConfig::default();
        config.enable_mcp_to_a2a = false;

        let bridge = UnifiedBridge::new(config);

        let tool_call = ToolCall {
            method: "test".to_string(),
            params: serde_json::json!({}),
        };

        let result = bridge.bridge_mcp_to_a2a(&tool_call).await;
        assert!(matches!(result, Err(BridgeError::BridgingDisabled(_))));
    }
}
