//! Transport adapter that converts RMCP transport to A2A transport

use crate::error::Result;
use crate::message::MessageConverter;
use a2a_rs::{Message, Task};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Transport adapter that bridges RMCP to A2A
pub struct RmcpToA2aTransport {
    converter: Arc<MessageConverter>,
}

impl RmcpToA2aTransport {
    /// Create a new RMCP to A2A transport adapter
    pub fn new(converter: Arc<MessageConverter>) -> Self {
        Self { converter }
    }

    /// Convert external request to A2A task
    pub async fn convert_request(&self, task_id: &str, message: Message) -> Result<Task> {
        // Create a new task with the message
        Ok(Task {
            id: task_id.to_string(),
            context_id: format!("{}-ctx", task_id),
            status: a2a_rs::TaskStatus {
                state: a2a_rs::TaskState::Submitted,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![message]),
            metadata: None,
            kind: "task".to_string(),
        })
    }

    /// Extract response data from task
    pub async fn extract_response(&self, task: &Task) -> Result<Value> {
        // Get the last agent message
        let agent_message = self.converter.extract_agent_message(task)?;

        // Extract data from message
        self.converter.extract_data(agent_message)
    }
}

/// Trait for handling RMCP to A2A message conversion
#[async_trait]
pub trait RmcpToA2aHandler {
    /// Process an external request as an A2A task
    async fn process_request(&self, task_id: String, message: Message)
    -> Result<Task>;
}
