//! WebAssembly bindings for A2A construct runtime
//!
//! This module provides browser-compatible WASM bindings for executing
//! A2A protocol stations deterministically. All operations are pure computation
//! with no I/O dependencies.
//!
//! # Example Usage (JavaScript)
//!
//! ```javascript
//! import init, { execute_station } from './a2a_wasm.js';
//!
//! await init();
//!
//! const request = {
//!   jsonrpc: "2.0",
//!   id: "1",
//!   method: "tasks/get",
//!   params: { id: "task-123" }
//! };
//!
//! const result = execute_station(JSON.stringify(request), "{}");
//! console.log(JSON.parse(result));
//! ```

use a2a_rs::construct::{
    station::{Ontology, StationRegistry},
    types::JsonRpcId,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Generic JSON-RPC request structure for deserialization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericJsonRpcRequest {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<JsonRpcId>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// Execution result containing response and updated state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionResult {
    /// JSON-RPC response
    response: serde_json::Value,

    /// Serialized ontology state after execution
    state: serde_json::Value,

    /// Whether execution succeeded
    success: bool,
}

/// Error response for WASM execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<JsonRpcId>,
    error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorDetail {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// Execute a station operation with provided ontology state
///
/// # Arguments
///
/// * `packet_json` - JSON-RPC request packet as a JSON string
/// * `state_json` - Current ontology state as a JSON string (use "{}" for empty state)
///
/// # Returns
///
/// JSON string containing:
/// - `response`: JSON-RPC response
/// - `state`: Updated ontology state (to be passed to next call)
/// - `success`: Whether execution succeeded
///
/// # Example
///
/// ```javascript
/// const request = JSON.stringify({
///   jsonrpc: "2.0",
///   id: "req-1",
///   method: "message/send",
///   params: {
///     message: {
///       role: "user",
///       parts: [{ text: "Hello" }]
///     }
///   }
/// });
///
/// const result = execute_station(request, "{}");
/// const { response, state, success } = JSON.parse(result);
/// ```
#[wasm_bindgen]
pub fn execute_station(packet_json: &str, state_json: &str) -> String {
    // Parse the incoming packet
    let request: GenericJsonRpcRequest = match serde_json::from_str(packet_json) {
        Ok(req) => req,
        Err(e) => {
            let error = ErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                error: ErrorDetail {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                    data: None,
                },
            };
            return serde_json::to_string(&error).unwrap_or_else(|_| "{}".to_string());
        }
    };

    // Deserialize or create ontology state
    let mut ontology: Ontology = match serde_json::from_str(state_json) {
        Ok(state) => state,
        Err(_) => Ontology::new(), // Default to empty ontology if parse fails
    };

    // Create station registry
    let mut registry = StationRegistry::new();

    // Dispatch the request
    let response = match registry.dispatch(
        &request.method,
        &mut ontology,
        request.params,
        request.id.clone(),
    ) {
        Ok(resp) => resp,
        Err(refusal) => {
            // Convert refusal to JSON-RPC error response
            let error = ErrorResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                error: ErrorDetail {
                    code: refusal.code,
                    message: refusal.reason,
                    data: refusal.data,
                },
            };

            let result = ExecutionResult {
                response: serde_json::to_value(&error).unwrap_or(serde_json::Value::Null),
                state: serde_json::to_value(&ontology).unwrap_or(serde_json::Value::Null),
                success: false,
            };

            return serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
        }
    };

    // Serialize updated state
    let state = serde_json::to_value(&ontology).unwrap_or(serde_json::Value::Null);

    // Create execution result
    let result = ExecutionResult {
        response,
        state,
        success: true,
    };

    serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
}

/// Execute a station operation with a fresh ontology (stateless mode)
///
/// Use this for simple one-off operations where you don't need to maintain state.
///
/// # Arguments
///
/// * `packet_json` - JSON-RPC request packet as a JSON string
///
/// # Returns
///
/// JSON-RPC response as a JSON string
///
/// # Example
///
/// ```javascript
/// const request = JSON.stringify({
///   jsonrpc: "2.0",
///   id: "req-1",
///   method: "agent/getExtendedCard",
///   params: {}
/// });
///
/// const response = execute_station_stateless(request);
/// ```
#[wasm_bindgen]
pub fn execute_station_stateless(packet_json: &str) -> String {
    let result = execute_station(packet_json, "{}");

    // Extract just the response from the full result
    match serde_json::from_str::<ExecutionResult>(&result) {
        Ok(exec_result) => {
            serde_json::to_string(&exec_result.response).unwrap_or_else(|_| "{}".to_string())
        }
        Err(_) => result, // If parsing fails, return original (likely an error)
    }
}

/// Get the current version of the WASM module
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if a method is supported by the station registry
///
/// # Arguments
///
/// * `method` - Method name (e.g., "message/send", "tasks/get")
///
/// # Returns
///
/// `true` if the method is supported, `false` otherwise
#[wasm_bindgen]
pub fn is_method_supported(method: &str) -> bool {
    let registry = StationRegistry::new();
    registry.has_method(method)
}

/// Get a list of all supported methods
///
/// # Returns
///
/// JSON array of method names as a string
#[wasm_bindgen]
pub fn list_supported_methods() -> String {
    let methods = vec![
        "message/send",
        "message/stream",
        "tasks/get",
        "tasks/cancel",
        "tasks/list",
        "tasks/resubscribe",
        "tasks/pushNotificationConfig/set",
        "tasks/pushNotificationConfig/get",
        "tasks/pushNotificationConfig/list",
        "tasks/pushNotificationConfig/delete",
        "agent/getExtendedCard",
        "agent/getAuthenticatedExtendedCard",
    ];

    serde_json::to_string(&methods).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }

    #[test]
    fn test_is_method_supported() {
        assert!(is_method_supported("message/send"));
        assert!(is_method_supported("tasks/get"));
        assert!(!is_method_supported("unknown/method"));
    }

    #[test]
    fn test_list_supported_methods() {
        let methods = list_supported_methods();
        assert!(methods.contains("message/send"));
        assert!(methods.contains("tasks/get"));
    }

    #[test]
    fn test_execute_station_parse_error() {
        let result = execute_station("invalid json", "{}");
        assert!(result.contains("Parse error"));
    }

    #[test]
    fn test_execute_station_stateless() {
        let request = r#"{
            "jsonrpc": "2.0",
            "id": "test-1",
            "method": "tasks/list",
            "params": {}
        }"#;

        let response = execute_station_stateless(request);
        assert!(response.contains("jsonrpc"));
    }
}
