//! Message conversion between A2A and RMCP protocols

use crate::error::{Error, Result};
use a2a_rs::{Message, Part, Role, Task};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

/// Simple tool call representation (bridge type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub method: String,
    pub params: Value,
}

/// Simple tool response representation (bridge type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub result: Value,
}

/// Simple tool definition (bridge type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
}

/// Converts between RMCP and A2A message formats
#[derive(Debug, Default)]
pub struct MessageConverter {}

impl MessageConverter {
    /// Create a new message converter
    pub fn new() -> Self {
        Self {}
    }

    /// Extract agent message content from a task
    pub fn extract_agent_message<'a>(&self, task: &'a Task) -> Result<&'a Message> {
        task.history
            .as_ref()
            .and_then(|history| {
                history
                    .iter()
                    .filter(|msg| msg.role == Role::Agent)
                    .last()
            })
            .ok_or_else(|| Error::TaskProcessing("No agent message found".into()))
    }

    /// Extract user message content from a task
    pub fn extract_user_message<'a>(&self, task: &'a Task) -> Result<&'a Message> {
        task.history
            .as_ref()
            .and_then(|history| {
                history
                    .iter()
                    .filter(|msg| msg.role == Role::User)
                    .last()
            })
            .ok_or_else(|| Error::TaskProcessing("No user message found".into()))
    }

    /// Extract data content from a message
    pub fn extract_data(&self, msg: &Message) -> Result<Value> {
        msg.parts
            .iter()
            .find_map(|part| match part {
                Part::Data { data, .. } => Some(Value::Object(data.clone())),
                Part::Text { text, .. } => {
                    // Try to parse text as JSON
                    serde_json::from_str(text).ok()
                }
                _ => None,
            })
            .ok_or_else(|| Error::Translation("No data content found in message".into()))
    }
}
