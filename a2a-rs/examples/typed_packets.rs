//! Example demonstrating the typed packet system
//!
//! This example shows how to use the typed packet system to enforce
//! closed-world semantics and eliminate serde_json::Value at boundaries.

use a2a_rs::construct::types::{JsonRpcId, Packet, PacketType, SendMessageRequest};
use a2a_rs::domain::{Message, MessageSendParams};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Typed Packet System Demo ===\n");

    // Create a message
    let message = Message::user_text("Hello, world!".to_string(), "msg-123".to_string());

    // Create params
    let params = MessageSendParams {
        task_id: "task-456".to_string(),
        message,
        send_config: None,
    };

    // Create a typed request with explicit ID
    let request =
        SendMessageRequest::new(params).with_id(JsonRpcId::from_string("req-789".to_string()));

    println!("Request method: {}", request.method());
    println!("Request ID: {:?}", request.id());
    println!();

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&request)?;
    println!("Serialized request:\n{}\n", json);

    // Deserialize from JSON
    let parsed: SendMessageRequest = serde_json::from_str(&json)?;
    println!("Parsed method: {}", parsed.method());
    println!("Parsed ID: {:?}", parsed.id());
    println!();

    // Demonstrate deny_unknown_fields
    println!("=== Testing deny_unknown_fields ===\n");

    let json_with_unknown = r#"{
        "jsonrpc": "2.0",
        "id": "test-123",
        "method": "message/send",
        "params": {
            "taskId": "task-456",
            "message": {
                "role": "user",
                "parts": [{"text": "Hello"}]
            }
        },
        "unknown_field": "this_should_fail"
    }"#;

    match serde_json::from_str::<SendMessageRequest>(json_with_unknown) {
        Ok(_) => println!("ERROR: Should have rejected unknown field!"),
        Err(e) => println!("SUCCESS: Rejected unknown field with error:\n  {}", e),
    }
    println!();

    // Demonstrate PacketType dispatch
    println!("=== PacketType Dispatch ===\n");

    for method in &[
        "message/send",
        "tasks/get",
        "tasks/cancel",
        "agent/getExtendedCard",
    ] {
        if let Some(packet_type) = PacketType::from_method(method) {
            println!("Method '{}' -> {:?}", method, packet_type);
        }
    }
    println!();

    // Demonstrate JSON-RPC ID types
    println!("=== JsonRpcId Types ===\n");

    let id_string = JsonRpcId::from_string("abc-123".to_string());
    let id_number = JsonRpcId::from_number(42);
    let id_null = JsonRpcId::Null;

    println!("String ID: {}", id_string);
    println!("Number ID: {}", id_number);
    println!("Null ID: {}", id_null);

    Ok(())
}
