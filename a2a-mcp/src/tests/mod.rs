//! Tests for a2a-mcp integration

#[cfg(test)]
mod message_converter_tests {
    use crate::message::MessageConverter;
    use a2a_rs::domain::{Message, Part, Role};
    use rmcp::{ClientJsonRpcMessage, ServerJsonRpcMessage};
    use serde::json;
    use serde_json::{json, Map, Value};

    #[test]
    fn test_rmcp_to_a2a_request() {
        let converter = MessageConverter::new();

        let rmcp_request = ClientJsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "test_method".to_string(),
            params: Some(json!({"key": "value"})),
        };

        let a2a_message = converter.rmcp_to_a2a_request(&rmcp_request).unwrap();

        assert_eq!(a2a_message.role, Role::User);
        assert_eq!(a2a_message.parts.len(), 2);

        // Check text part
        match &a2a_message.parts[0] {
            Part::Text { text, .. } => {
                assert!(text.contains("test_method"));
            }
            _ => panic!("Expected text part"),
        }

        // Check data part
        match &a2a_message.parts[1] {
            Part::Data { data, .. } => {
                assert_eq!(data.get("key"), Some(&Value::String("value".to_string())));
            }
            _ => panic!("Expected data part"),
        }
    }

    #[test]
    fn test_a2a_to_rmcp_response() {
        let converter = MessageConverter::new();

        let mut data_map = Map::new();
        data_map.insert("result".to_string(), Value::String("success".to_string()));

        let a2a_message = Message {
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

        let id = Some(json!(123));
        let rmcp_response = converter
            .a2a_to_rmcp_response(&a2a_message, id.clone())
            .unwrap();

        assert_eq!(rmcp_response.jsonrpc, "2.0");
        assert_eq!(rmcp_response.id, id);
        assert!(rmcp_response.error.is_none());

        // Data part should be prioritized over text
        assert_eq!(rmcp_response.result.unwrap()["result"], "success");
    }
}

#[cfg(test)]
mod adapter_tests {
    use crate::adapter::tool_to_agent::{ToolToAgentAdapter, Tool};
    use a2a_rs::domain::{Role, TaskState};
    use rmcp::ToolCall;
    use serde_json::json;

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

        let (tool_name, params) = adapter.extract_tool_call(&message).unwrap();

        assert_eq!(tool_name, "test_tool");
        assert_eq!(params["input"], "test_input");
    }
}
