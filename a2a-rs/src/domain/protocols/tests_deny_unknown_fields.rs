//! Tests for deny_unknown_fields on JSON-RPC protocol types

use serde_json::json;

use super::json_rpc::*;

#[test]
fn test_jsonrpc_message_rejects_unknown_fields() {
    let json = json!({
        "jsonrpc": "2.0",
        "unknownField": "fail"
    });

    let result: Result<JSONRPCMessage, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "JSONRPCMessage should reject unknown fields"
    );
}

#[test]
fn test_jsonrpc_error_rejects_unknown_fields() {
    let json = json!({
        "code": -32600,
        "message": "Invalid Request",
        "unknownField": "fail"
    });

    let result: Result<JSONRPCError, _> = serde_json::from_value(json);
    assert!(result.is_err(), "JSONRPCError should reject unknown fields");
}

#[test]
fn test_jsonrpc_request_rejects_unknown_fields() {
    let json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "test",
        "unknownField": "fail"
    });

    let result: Result<JSONRPCRequest, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "JSONRPCRequest should reject unknown fields"
    );
}

#[test]
fn test_jsonrpc_response_rejects_unknown_fields() {
    let json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {},
        "unknownField": "fail"
    });

    let result: Result<JSONRPCResponse, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "JSONRPCResponse should reject unknown fields"
    );
}

#[test]
fn test_jsonrpc_notification_rejects_unknown_fields() {
    let json = json!({
        "jsonrpc": "2.0",
        "method": "test",
        "unknownField": "fail"
    });

    let result: Result<JSONRPCNotification, _> = serde_json::from_value(json);
    assert!(
        result.is_err(),
        "JSONRPCNotification should reject unknown fields"
    );
}

// Test that valid JSON-RPC still works

#[test]
fn test_valid_jsonrpc_request_still_works() {
    let json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "test",
        "params": {
            "key": "value"
        }
    });

    let result: Result<JSONRPCRequest, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Valid JSONRPCRequest should deserialize");
}

#[test]
fn test_valid_jsonrpc_response_still_works() {
    let json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "key": "value"
        }
    });

    let result: Result<JSONRPCResponse, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "Valid JSONRPCResponse should deserialize");
}

#[test]
fn test_jsonrpc_request_allows_arbitrary_params() {
    // params is explicitly Option<Value> so arbitrary structure is OK
    let json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "test",
        "params": {
            "arbitraryKey": "value",
            "nested": {
                "data": [1, 2, 3]
            }
        }
    });

    let result: Result<JSONRPCRequest, _> = serde_json::from_value(json);
    assert!(
        result.is_ok(),
        "JSONRPCRequest should allow arbitrary params"
    );
}

#[test]
fn test_jsonrpc_error_allows_arbitrary_data() {
    // data is explicitly Option<Value> so arbitrary structure is OK
    let json = json!({
        "code": -32600,
        "message": "Invalid Request",
        "data": {
            "arbitraryKey": "value",
            "nested": {
                "error": "details"
            }
        }
    });

    let result: Result<JSONRPCError, _> = serde_json::from_value(json);
    assert!(result.is_ok(), "JSONRPCError should allow arbitrary data");
}
