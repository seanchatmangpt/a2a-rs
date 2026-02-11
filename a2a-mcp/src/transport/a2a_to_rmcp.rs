//! Transport adapter that converts A2A transport to RMCP transport

use crate::error::Result;
use crate::message::MessageConverter;
use a2a_rs::{Message, Part, Role};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// Transport adapter that bridges A2A to RMCP
pub struct A2aToRmcpTransport {
    converter: Arc<MessageConverter>,
}

impl A2aToRmcpTransport {
    /// Create a new A2A to RMCP transport adapter
    pub fn new(converter: Arc<MessageConverter>) -> Self {
        Self { converter }
    }

    /// Convert A2A message to RMCP tool call parameters
    pub async fn convert_message_to_tool_call(
        &self,
        msg: &Message,
        method: &str,
    ) -> Result<(String, Value)> {
        // Extract parameters from message
        let params = self.converter.extract_data(msg)?;

        Ok((method.to_string(), params))
    }

    /// Convert RMCP tool response result to A2A message
    pub async fn convert_result_to_message(
        &self,
        result: &Value,
    ) -> Result<Message> {
        // Convert Value to Map<String, Value>
        let data_map = if let Some(obj) = result.as_object() {
            obj.clone()
        } else {
            serde_json::Map::new()
        };

        Ok(Message {
            role: Role::Agent,
            parts: vec![Part::Data {
                data: data_map,
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: Uuid::new_v4().to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        })
    }
}

/// Trait for handling A2A to RMCP message conversion
#[async_trait]
pub trait A2aToRmcpHandler {
    /// Process an A2A task as RMCP tool calls
    async fn process_task(&self, task: &a2a_rs::Task) -> Result<Value>;
}
