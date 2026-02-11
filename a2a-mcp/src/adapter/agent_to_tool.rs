//! Adapter that exposes A2A agents as RMCP tools

use crate::error::{Error, Result};
use crate::message::MessageConverter;
use a2a_rs::{AgentCard, AgentSkill, Message, Part, Role, Task, TaskState, TaskStatus};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Tool definition for RMCP
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
}

/// Adapts A2A agents to RMCP tool capabilities
pub struct AgentToToolAdapter {
    pub converter: Arc<MessageConverter>,
    agent_cache: HashMap<String, AgentCard>,
}

impl AgentToToolAdapter {
    /// Create a new adapter
    pub fn new() -> Self {
        Self {
            converter: Arc::new(MessageConverter::new()),
            agent_cache: HashMap::new(),
        }
    }

    /// Add an agent to cache
    pub fn add_agent(&mut self, url: String, card: AgentCard) {
        self.agent_cache.insert(url, card);
    }

    /// Get an agent from cache
    pub fn get_agent(&self, url: &str) -> Option<&AgentCard> {
        self.agent_cache.get(url)
    }

    /// Generate RMCP tools from an A2A agent
    pub fn generate_tools(&self, agent: &AgentCard, agent_url: &str) -> Vec<Tool> {
        agent
            .skills
            .iter()
            .map(|skill| self.skill_to_tool(skill, agent, agent_url))
            .collect()
    }

    /// Convert an A2A skill to an RMCP tool
    fn skill_to_tool(&self, skill: &AgentSkill, agent: &AgentCard, agent_url: &str) -> Tool {
        let tool_name = format!("{}:{}", agent_url, skill.name);

        Tool {
            name: tool_name,
            description: format!("{} - {}", agent.description, skill.description),
        }
    }

    /// Convert tool call to task parameters
    pub fn tool_call_to_task(
        &self,
        tool_name: &str,
        params: &Value,
        _agent_card: &AgentCard,
    ) -> Result<Task> {
        let task_id = Uuid::new_v4().to_string();
        let context_id = Uuid::new_v4().to_string();
        let message_id = Uuid::new_v4().to_string();

        // Create a message from tool call
        let text = format!("Call tool: {}", tool_name);

        // Convert Value to Map<String, Value>
        let data_map = if let Some(obj) = params.as_object() {
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        let message = Message {
            role: Role::User,
            parts: vec![
                Part::Text {
                    text,
                    metadata: None,
                },
                Part::Data {
                    data: data_map,
                    metadata: None,
                },
            ],
            metadata: None,
            reference_task_ids: None,
            message_id,
            task_id: Some(task_id.clone()),
            context_id: Some(context_id.clone()),
            extensions: None,
            kind: "message".to_string(),
        };

        // Create a task with the message
        Ok(Task {
            id: task_id,
            context_id,
            status: TaskStatus {
                state: TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![message]),
            metadata: None,
            kind: "task".to_string(),
        })
    }

    /// Parse tool method string in format "agent_url:method"
    pub fn parse_tool_method(&self, tool_method: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = tool_method.split(':').collect();
        if parts.len() != 2 {
            return Err(Error::InvalidToolMethod(tool_method.to_string()));
        }

        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}
