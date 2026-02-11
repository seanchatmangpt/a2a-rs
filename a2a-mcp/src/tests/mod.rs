//! Tests for a2a-mcp integration

#[cfg(test)]
mod message_converter_tests {
    use crate::message::MessageConverter;
    use a2a_rs::domain::{Message, Part, Role, Task, TaskStatus, TaskState};
    use serde_json::{Map, Value};
    use std::sync::Arc;

    #[test]
    fn test_message_converter_creation() {
        let _converter = MessageConverter::new();
        assert!(true); // Test that we can create a converter
    }

    #[test]
    fn test_extract_agent_message() {
        let converter = MessageConverter::new();

        let mut data_map = Map::new();
        data_map.insert("result".to_string(), Value::String("success".to_string()));

        let user_message = Message {
            role: Role::User,
            parts: vec![Part::Text {
                text: "Hello".to_string(),
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: "msg-1".to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        };

        let agent_message = Message {
            role: Role::Agent,
            parts: vec![
                Part::Text {
                    text: "Test response".to_string(),
                    metadata: None,
                },
                Part::Data {
                    data: data_map.clone(),
                    metadata: None,
                },
            ],
            metadata: None,
            reference_task_ids: None,
            message_id: "msg-123".to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        };

        let task = Task {
            id: "task-1".to_string(),
            context_id: "ctx-1".to_string(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![user_message, agent_message]),
            metadata: None,
            kind: "task".to_string(),
        };

        let extracted = converter.extract_agent_message(&task).unwrap();
        assert_eq!(extracted.role, Role::Agent);
        assert_eq!(extracted.message_id, "msg-123");
    }

    #[test]
    fn test_extract_user_message() {
        let converter = MessageConverter::new();

        let user_message = Message {
            role: Role::User,
            parts: vec![Part::Text {
                text: "Hello".to_string(),
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: "msg-1".to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        };

        let agent_message = Message {
            role: Role::Agent,
            parts: vec![Part::Text {
                text: "Hi there".to_string(),
                metadata: None,
            }],
            metadata: None,
            reference_task_ids: None,
            message_id: "msg-2".to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        };

        let task = Task {
            id: "task-1".to_string(),
            context_id: "ctx-1".to_string(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
                timestamp: Some(chrono::Utc::now()),
            },
            artifacts: None,
            history: Some(vec![user_message, agent_message]),
            metadata: None,
            kind: "task".to_string(),
        };

        let extracted = converter.extract_user_message(&task).unwrap();
        assert_eq!(extracted.role, Role::User);
        assert_eq!(extracted.message_id, "msg-1");
    }

    #[test]
    fn test_extract_data_from_message() {
        let converter = MessageConverter::new();

        let mut data_map = Map::new();
        data_map.insert("result".to_string(), Value::String("success".to_string()));

        let message = Message {
            role: Role::Agent,
            parts: vec![
                Part::Text {
                    text: "Test response".to_string(),
                    metadata: None,
                },
                Part::Data {
                    data: data_map.clone(),
                    metadata: None,
                },
            ],
            metadata: None,
            reference_task_ids: None,
            message_id: "msg-123".to_string(),
            task_id: None,
            context_id: None,
            extensions: None,
            kind: "message".to_string(),
        };

        let extracted_data = converter.extract_data(&message).unwrap();
        assert_eq!(extracted_data.get("result"), Some(&Value::String("success".to_string())));
    }

    #[test]
    fn test_a2a_to_rmcp_transport() {
        use crate::transport::a2a_to_rmcp::A2aToRmcpTransport;

        let converter = Arc::new(MessageConverter::new());
        let transport = A2aToRmcpTransport::new(converter);

        // Test that transport can be created
        assert!(true);
    }

    #[test]
    fn test_rmcp_to_a2a_transport() {
        use crate::transport::rmcp_to_a2a::RmcpToA2aTransport;

        let converter = Arc::new(MessageConverter::new());
        let transport = RmcpToA2aTransport::new(converter);

        // Test that transport can be created
        assert!(true);
    }
}

#[cfg(test)]
mod adapter_tests {
    use crate::adapter::tool_to_agent::{ToolToAgentAdapter, Tool};
    use a2a_rs::domain::Role;

    #[test]
    fn test_tool_to_agent_adapter() {
        let tools = vec![Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
        }];

        let adapter = ToolToAgentAdapter::new(
            tools,
            "Test Agent".to_string(),
            "An agent for testing".to_string(),
        );

        let agent_card = adapter.generate_agent_card();

        assert_eq!(agent_card.name, "Test Agent");
        assert_eq!(agent_card.description, "An agent for testing");
        assert_eq!(agent_card.skills.len(), 1);
        assert_eq!(agent_card.skills[0].name, "test_tool");
    }

    #[test]
    fn test_extract_tool_call() {
        let tools = vec![Tool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
        }];

        let adapter = ToolToAgentAdapter::new(
            tools,
            "Test Agent".to_string(),
            "An agent for testing".to_string(),
        );

        // Test with tool call message
        let message = a2a_rs::domain::Message::builder()
            .role(Role::User)
            .message_id("msg-123".to_string())
            .parts(vec![
                a2a_rs::domain::Part::Text {
                    text: "Call tool: test_tool".to_string(),
                    metadata: None,
                },
                a2a_rs::domain::Part::Data {
                    data: serde_json::json!({"input": "test_input"}).as_object().unwrap().clone(),
                    metadata: None,
                },
            ])
            .build();

        let result = adapter.extract_tool_call(&message);

        // Verify extraction works
        assert!(result.is_ok());
        let (tool_name, params) = result.unwrap();
        assert_eq!(tool_name, "test_tool");
        assert_eq!(params["input"], "test_input");
    }
}
