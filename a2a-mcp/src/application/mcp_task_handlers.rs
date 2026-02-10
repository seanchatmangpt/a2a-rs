//! JSON-RPC handlers for MCP task operations
//!
//! Exposes tasks/get and tasks/result JSON-RPC methods.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::domain::{McpTaskGetParams, McpTaskResultParams};
use crate::error::{Error, Result};
use crate::port::mcp_task_manager::McpTaskManager;

/// JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Create an error response with data
    pub fn error_with_data(
        id: Option<Value>,
        code: i32,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }
}

/// MCP task JSON-RPC handler
pub struct McpTaskHandler {
    task_manager: Arc<dyn McpTaskManager>,
}

impl McpTaskHandler {
    /// Create a new handler with the given task manager
    pub fn new(task_manager: Arc<dyn McpTaskManager>) -> Self {
        Self { task_manager }
    }

    /// Handle a JSON-RPC request
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "tasks/get" => self.handle_tasks_get(request).await,
            "tasks/result" => self.handle_tasks_result(request).await,
            "tasks/list" => self.handle_tasks_list(request).await,
            "tasks/cancel" => self.handle_tasks_cancel(request).await,
            _ => JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    /// Handle tasks/get method
    async fn handle_tasks_get(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = match self.parse_params::<McpTaskGetParams>(request.params.clone()) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(request.id, -32602, e.to_string());
            }
        };

        match self.task_manager.get_task(&params.task_id).await {
            Ok(task) => {
                let result = serde_json::to_value(task).unwrap_or(Value::Null);
                JsonRpcResponse::success(request.id, result)
            }
            Err(Error::TaskNotFound(task_id)) => {
                JsonRpcResponse::error(request.id, -32000, format!("Task not found: {}", task_id))
            }
            Err(e) => JsonRpcResponse::error(request.id, -32000, e.to_string()),
        }
    }

    /// Handle tasks/result method
    async fn handle_tasks_result(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = match self.parse_params::<McpTaskResultParams>(request.params.clone()) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(request.id, -32602, e.to_string());
            }
        };

        match self.task_manager.get_task_result(&params.task_id).await {
            Ok(result) => {
                let result_value = serde_json::to_value(result).unwrap_or(Value::Null);
                JsonRpcResponse::success(request.id, result_value)
            }
            Err(Error::TaskNotFound(task_id)) => {
                JsonRpcResponse::error(request.id, -32000, format!("Task not found: {}", task_id))
            }
            Err(e) => JsonRpcResponse::error(request.id, -32000, e.to_string()),
        }
    }

    /// Handle tasks/list method
    async fn handle_tasks_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match self.task_manager.list_tasks().await {
            Ok(tasks) => {
                let result = serde_json::to_value(tasks).unwrap_or(Value::Null);
                JsonRpcResponse::success(request.id, result)
            }
            Err(e) => JsonRpcResponse::error(request.id, -32000, e.to_string()),
        }
    }

    /// Handle tasks/cancel method
    async fn handle_tasks_cancel(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let params = match self.parse_params::<McpTaskGetParams>(request.params.clone()) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(request.id, -32602, e.to_string());
            }
        };

        match self.task_manager.cancel_task(&params.task_id).await {
            Ok(()) => JsonRpcResponse::success(request.id, Value::Null),
            Err(Error::TaskNotFound(task_id)) => {
                JsonRpcResponse::error(request.id, -32000, format!("Task not found: {}", task_id))
            }
            Err(e) => JsonRpcResponse::error(request.id, -32000, e.to_string()),
        }
    }

    /// Helper to parse parameters
    fn parse_params<T: for<'de> Deserialize<'de>>(&self, params: Option<Value>) -> Result<T> {
        match params {
            Some(p) => serde_json::from_value(p).map_err(|e| Error::Json(e)),
            None => Err(Error::Translation("Missing parameters".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::TaskWrapper;

    #[tokio::test]
    async fn test_handle_tasks_get() {
        let wrapper = Arc::new(TaskWrapper::new());
        let handler = McpTaskHandler::new(wrapper.clone());

        // Create a task
        let task = wrapper
            .create_task(|| async { Ok(Value::String("test".to_string())) })
            .await
            .unwrap();

        // Create JSON-RPC request
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "tasks/get".to_string(),
            params: Some(serde_json::json!({ "taskId": task.id })),
        };

        let response = handler.handle_request(request).await;
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_tasks_result() {
        let wrapper = Arc::new(TaskWrapper::new());
        let handler = McpTaskHandler::new(wrapper.clone());

        // Create a task
        let task = wrapper
            .create_task(|| async { Ok(Value::String("test result".to_string())) })
            .await
            .unwrap();

        // Wait for completion
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create JSON-RPC request
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "tasks/result".to_string(),
            params: Some(serde_json::json!({ "taskId": task.id })),
        };

        let response = handler.handle_request(request).await;
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_tasks_list() {
        let wrapper = Arc::new(TaskWrapper::new());
        let handler = McpTaskHandler::new(wrapper.clone());

        // Create some tasks
        wrapper
            .create_task(|| async { Ok(Value::Null) })
            .await
            .unwrap();
        wrapper
            .create_task(|| async { Ok(Value::Null) })
            .await
            .unwrap();

        // Create JSON-RPC request
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "tasks/list".to_string(),
            params: None,
        };

        let response = handler.handle_request(request).await;
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[tokio::test]
    async fn test_handle_task_not_found() {
        let wrapper = Arc::new(TaskWrapper::new());
        let handler = McpTaskHandler::new(wrapper);

        // Create JSON-RPC request for non-existent task
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "tasks/get".to_string(),
            params: Some(serde_json::json!({ "taskId": "non-existent" })),
        };

        let response = handler.handle_request(request).await;
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32000);
    }
}
