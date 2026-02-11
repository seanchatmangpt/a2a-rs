//! Adapter that exposes RMCP tools as A2A agents

use crate::error::{Error, Result};
use crate::message::MessageConverter;
use a2a_rs::{AgentCard, AgentSkill, Message, Part};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool definition from RMCP
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
}

/// Adapts RMCP tools to A2A agent capabilities
pub struct ToolToAgentAdapter {
    tools: Vec<Tool>,
    agent_name: String,
    agent_description: String,
    converter: Arc<MessageConverter>,
}

impl ToolToAgentAdapter {
    /// Create a new adapter with given tools
    pub fn new(
        tools: Vec<Tool>,
        agent_name: String,
        agent_description: String,
    ) -> Self {
        Self {
            tools,
            agent_name,
            agent_description,
            converter: Arc::new(MessageConverter::new()),
        }
    }

    /// Generate A2A agent card from RMCP tools
    pub fn generate_agent_card(&self) -> AgentCard {
        // Create skills from tools
        let skills = self
            .tools
            .iter()
            .map(|tool| AgentSkill {
                id: tool.name.clone(),
                name: tool.name.clone(),
                description: tool.description.clone(),
                tags: vec!["mcp".to_string(), "tool".to_string()],
                examples: None,
                input_modes: Some(vec!["text".to_string()]),
                output_modes: Some(vec!["text".to_string()]),
                security: None,
            })
            .collect();

        // Create security schemes
        let mut security_schemes = HashMap::new();
        security_schemes.insert(
            "bearer".to_string(),
            a2a_rs::SecurityScheme::Http {
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
                description: Some("Bearer token authentication".to_string()),
            },
        );

        a2a_rs::AgentCardBuilder::default()
            .name(self.agent_name.clone())
            .description(self.agent_description.clone())
            .url("https://example.com/agent".to_string()) // Would be configured
            .provider(None)
            .version("1.0.0".to_string())
            .protocol_version("0.3.0".to_string())
            .preferred_transport("JSONRPC".to_string())
            .additional_interfaces(None)
            .icon_url(None)
            .documentation_url(None)
            .capabilities(a2a_rs::AgentCapabilities {
                streaming: true,
                push_notifications: false,
                state_transition_history: true,
                extensions: None,
            })
            .security_schemes(Some(security_schemes))
            .security(None)
            .default_input_modes(vec!["text".to_string()])
            .default_output_modes(vec!["text".to_string()])
            .skills(skills)
            .signatures(None)
            .supports_authenticated_extended_card(None)
            .build()
    }

    /// Find a tool by name
    pub fn find_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name == name)
    }

    /// Extract tool name and parameters from an A2A message
    pub fn extract_tool_call(&self, message: &Message) -> Result<(String, Value)> {
        // Try to find a text part with tool call instruction
        let tool_name = message
            .parts
            .iter()
            .find_map(|part| match part {
                Part::Text { text, .. } => {
                    // Parse tool call from text
                    if text.starts_with("Call tool: ") {
                        Some(text.trim_start_matches("Call tool: ").to_string())
                    } else {
                        // Use entire text as tool name
                        Some(text.clone())
                    }
                }
                _ => None,
            })
            .ok_or_else(|| Error::Translation("Unable to extract tool name from message".into()))?;

        // Try to find a data part with parameters
        let params_map = message
            .parts
            .iter()
            .find_map(|part| match part {
                Part::Data { data, .. } => Some(data.clone()),
                _ => None,
            })
            .unwrap_or_else(serde_json::Map::new);

        // Convert Map to Value::Object
        let params = Value::Object(params_map);

        Ok((tool_name, params))
    }
}
