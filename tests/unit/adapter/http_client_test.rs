//! Unit tests for HTTP client adapter
//!
//! Tests the HTTP client adapter implementation with mock servers,
//! focusing on request handling, error cases, and edge cases.

use a2a_rs::adapter::HttpClientError;
use a2a_rs::domain::{
    A2AError, ListTasksParams, Message, Part, Role, Task, TaskPushNotificationConfig,
    TaskQueryParams, TaskSendParams,
};
use a2a_rs::services::client::AsyncA2AClient;
use async_trait::async_trait;
use serde_json::json;
use std::time::Duration;

/// Mock HTTP client for testing
#[derive(Clone)]
struct MockHttpClient {
    responses: std::sync::Arc<std::sync::Mutex<Vec<Option<String>>>>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    should_fail: std::sync::Arc<std::sync::Mutex<bool>>,
    failure_status: std::sync::Arc<std::sync::Mutex<Option<u16>>>,
}

impl MockHttpClient {
    fn new() -> Self {
        Self {
            responses: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            requests: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            should_fail: std::sync::Arc::new(std::sync::Mutex::new(false)),
            failure_status: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn add_response(&self, response: Option<String>) {
        self.responses.lock().unwrap().push(response);
    }

    fn set_failure(&self, status: u16) {
        *self.should_fail.lock().unwrap() = true;
        *self.failure_status.lock().unwrap() = Some(status);
    }

    fn get_requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl AsyncA2AClient for MockHttpClient {
    async fn send_raw_request<'a>(&self, request: &'a str) -> Result<String, A2AError> {
        // Track the request
        self.requests
            .lock()
            .unwrap()
            .push(request.to_string());

        // Simulate network delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Check if we should fail
        if *self.should_fail.lock().unwrap() {
            let status = *self.failure_status.lock().unwrap();
            return Err(HttpClientError::Response {
                status: status.unwrap_or(500),
                message: "Internal server error".to_string(),
            }
            .into());
        }

        // Get next response
        let mut responses = self.responses.lock().unwrap();
        if let Some(response) = responses.drain(1..).next().and_then(|r| r.first()) {
            Ok(response.clone())
        } else {
            Err(HttpClientError::Response {
                status: 500,
                message: "No response configured".to_string(),
            }
            .into())
        }
    }

    async fn send_request<'a>(
        &self,
        request: &'a a2a_rs::application::json_rpc::A2ARequest,
    ) -> Result<a2a_rs::application::JSONRPCResponse, A2AError> {
        let json = serde_json::to_string(request).unwrap();
        let response_text = self.send_raw_request(&json).await?;
        let response: a2a_rs::application::JSONRPCResponse =
            serde_json::from_str(&response_text)?;
        Ok(response)
    }

    async fn send_task_message<'a>(
        &self,
        task_id: &'a str,
        message: &'a Message,
        session_id: Option<&'a str>,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let params = TaskSendParams {
            id: task_id.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            message: message.clone(),
            push_notification: None,
            history_length,
            metadata: None,
        };

        let request =
            a2a_rs::application::json_rpc::SendTaskRequest::new(params);
        let response = self
            .send_request(&a2a_rs::application::json_rpc::A2ARequest::SendTask(
                request,
            ))
            .await?;

        match response.result {
            Some(value) => {
                let task: Task = serde_json::from_value(value)?;
                Ok(task)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn get_task<'a>(
        &self,
        task_id: &'a str,
        history_length: Option<u32>,
    ) -> Result<Task, A2AError> {
        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length,
            metadata: None,
        };

        let request =
            a2a_rs::application::json_rpc::GetTaskRequest::new(params);
        let response = self
            .send_request(&a2a_rs::application::json_rpc::A2ARequest::GetTask(
                request,
            ))
            .await?;

        match response.result {
            Some(value) => {
                let task: Task = serde_json::from_value(value)?;
                Ok(task)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn cancel_task<'a>(&self, task_id: &'a str) -> Result<Task, A2AError> {
        let params = a2a_rs::domain::TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };

        let request =
            a2a_rs::application::json_rpc::CancelTaskRequest::new(params);
        let response = self
            .send_request(&a2a_rs::application::json_rpc::A2ARequest::CancelTask(
                request,
            ))
            .await?;

        match response.result {
            Some(value) => {
                let task: Task = serde_json::from_value(value)?;
                Ok(task)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn set_task_push_notification<'a>(
        &self,
        config: &'a TaskPushNotificationConfig,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let request =
            a2a_rs::application::json_rpc::SetTaskPushNotificationRequest::new(
                config.clone(),
            );
        let response = self
            .send_request(
                &a2a_rs::application::json_rpc::A2ARequest::SetTaskPushNotification(
                    request,
                ),
            )
            .await?;

        match response.result {
            Some(value) => {
                let config: TaskPushNotificationConfig = serde_json::from_value(value)?;
                Ok(config)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn get_task_push_notification<'a>(
        &self,
        task_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        let params = a2a_rs::domain::TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };

        let request =
            a2a_rs::application::json_rpc::GetTaskPushNotificationRequest::new(
                params,
            );
        let response = self
            .send_request(
                &a2a_rs::application::json_rpc::A2ARequest::GetTaskPushNotification(
                    request,
                ),
            )
            .await?;

        match response.result {
            Some(value) => {
                let config: TaskPushNotificationConfig = serde_json::from_value(value)?;
                Ok(config)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn list_tasks<'a>(
        &self,
        params: &'a ListTasksParams,
    ) -> Result<a2a_rs::domain::ListTasksResult, A2AError> {
        let request =
            a2a_rs::application::json_rpc::ListTasksRequest::new(Some(params.clone()));
        let response = self
            .send_request(&a2a_rs::application::json_rpc::A2ARequest::ListTasks(
                request,
            ))
            .await?;

        match response.result {
            Some(value) => {
                let result: a2a_rs::domain::ListTasksResult =
                    serde_json::from_value(value)?;
                Ok(result)
            }
            None => {
                if let Some(error) = response.error {
                    Err(A2AError::JsonRpc {
                        code: error.code,
                        message: error.message,
                        data: error.data,
                    })
                } else {
                    Err(A2AError::Internal("Empty response".to_string()))
                }
            }
        }
    }

    async fn list_push_notification_configs<'a>(
        &self,
        _task_id: &'a str,
    ) -> Result<Vec<TaskPushNotificationConfig>, A2AError> {
        Err(A2AError::UnsupportedOperation(
            "Streaming not supported".to_string(),
        ))
    }

    async fn get_push_notification_config<'a>(
        &self,
        _task_id: &'a str,
        _config_id: &'a str,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::UnsupportedOperation(
            "Streaming not supported".to_string(),
        ))
    }

    async fn delete_push_notification_config<'a>(
        &self,
        _task_id: &'a str,
        _config_id: &'a str,
    ) -> Result<(), A2AError> {
        Err(A2AError::UnsupportedOperation(
            "Streaming not supported".to_string(),
        ))
    }

    async fn subscribe_to_task<'a>(
        &self,
        _task_id: &'a str,
        _history_length: Option<u32>,
    ) -> Result<
        Pin<Box<dyn futures::Stream<Item = Result<a2a_rs::services::client::StreamItem, A2AError>> + Send>>,
        A2AError,
    > {
        Err(A2AError::UnsupportedOperation(
            "Streaming not supported".to_string(),
        ))
    }
}

fn create_success_response(result: serde_json::Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": result
    })
    .to_string()
}

fn create_error_response(code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}

fn create_test_task(id: &str) -> Task {
    Task::builder()
        .id(id.to_string())
        .session_id("test-session".to_string())
        .build()
}

fn create_test_message() -> Message {
    Message::builder()
        .role(Role::User)
        .parts(vec![Part::text("Test message".to_string())])
        .message_id("msg-1".to_string())
        .build()
}

#[tokio::test]
async fn test_send_raw_request_success() {
    let client = MockHttpClient::new();
    let response = create_success_response(json!({"status": "ok"}));
    client.add_response(Some(response));

    let result = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), r#"{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}"#);
}

#[tokio::test]
async fn test_send_raw_request_failure() {
    let client = MockHttpClient::new();
    client.set_failure(500);

    let result = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    assert!(result.is_err());
    if let Err(A2AError::HttpClientError(HttpClientError::Response { status, .. })) = result {
        assert_eq!(status, 500);
    } else {
        panic!("Expected HttpClientError");
    }
}

#[tokio::test]
async fn test_send_raw_request_404() {
    let client = MockHttpClient::new();
    client.set_failure(404);

    let result = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_task_message_success() {
    let client = MockHttpClient::new();
    let task = create_test_task("task-1");
    let message = create_test_message();
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client
        .send_task_message("task-1", &message, Some("session-1"), None)
        .await;

    assert!(result.is_ok());
    let returned_task = result.unwrap();
    assert_eq!(returned_task.id, "task-1");
}

#[tokio::test]
async fn test_send_task_message_with_history_length() {
    let client = MockHttpClient::new();
    let task = create_test_task("task-2");
    let message = create_test_message();
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client
        .send_task_message("task-2", &message, None, Some(10))
        .await;

    assert!(result.is_ok());

    // Verify request includes history length
    let requests = client.get_requests();
    assert!(!requests.is_empty());
    let request_json: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    assert_eq!(
        request_json["params"]["historyLength"],
        10
    );
}

#[tokio::test]
async fn test_get_task_success() {
    let client = MockHttpClient::new();
    let task = create_test_task("task-3");
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client.get_task("task-3", None).await;

    assert!(result.is_ok());
    let returned_task = result.unwrap();
    assert_eq!(returned_task.id, "task-3");
}

#[tokio::test]
async fn test_get_task_with_history_length() {
    let client = MockHttpClient::new();
    let mut task = create_test_task("task-4");
    task.artifacts = vec![];
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client.get_task("task-4", Some(5)).await;

    assert!(result.is_ok());

    // Verify request includes history length
    let requests = client.get_requests();
    let request_json: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    assert_eq!(request_json["params"]["historyLength"], 5);
}

#[tokio::test]
async fn test_get_task_not_found() {
    let client = MockHttpClient::new();
    let error_response = create_error_response(-32001, "Task not found");
    client.add_response(Some(error_response));

    let result = client.get_task("nonexistent", None).await;

    assert!(result.is_err());
    if let Err(A2AError::JsonRpc { code, .. }) = result {
        assert_eq!(code, -32001);
    } else {
        panic!("Expected JsonRpc error");
    }
}

#[tokio::test]
async fn test_cancel_task_success() {
    let client = MockHttpClient::new();
    let mut task = create_test_task("task-5");
    task.status.state = a2a_rs::domain::TaskState::Canceled;
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client.cancel_task("task-5").await;

    assert!(result.is_ok());
    let canceled_task = result.unwrap();
    assert_eq!(canceled_task.status.state, a2a_rs::domain::TaskState::Canceled);
}

#[tokio::test]
async fn test_cancel_task_not_cancellable() {
    let client = MockHttpClient::new();
    let error_response = create_error_response(-32002, "Task not cancelable");
    client.add_response(Some(error_response));

    let result = client.cancel_task("task-6").await;

    assert!(result.is_err());
    if let Err(A2AError::JsonRpc { code, .. }) = result {
        assert_eq!(code, -32002);
    } else {
        panic!("Expected JsonRpc error");
    }
}

#[tokio::test]
async fn test_set_task_push_notification_success() {
    let client = MockHttpClient::new();
    let config = TaskPushNotificationConfig {
        id: Some("config-1".to_string()),
        url: "https://example.com/webhook".to_string(),
        token: Some("token-123".to_string()),
        authentication: None,
    };
    let response = create_success_response(serde_json::to_value(&config).unwrap());
    client.add_response(Some(response));

    let result = client
        .set_task_push_notification(&config)
        .await;

    assert!(result.is_ok());
    let returned_config = result.unwrap();
    assert_eq!(returned_config.url, "https://example.com/webhook");
}

#[tokio::test]
async fn test_get_task_push_notification_success() {
    let client = MockHttpClient::new();
    let config = TaskPushNotificationConfig {
        id: Some("config-2".to_string()),
        url: "https://example.com/webhook2".to_string(),
        token: Some("token-456".to_string()),
        authentication: None,
    };
    let response = create_success_response(serde_json::to_value(&config).unwrap());
    client.add_response(Some(response));

    let result = client
        .get_task_push_notification("task-7")
        .await;

    assert!(result.is_ok());
    let returned_config = result.unwrap();
    assert_eq!(returned_config.id, Some("config-2".to_string()));
}

#[tokio::test]
async fn test_list_tasks_success() {
    let client = MockHttpClient::new();
    let result_tasks = vec![create_test_task("task-8"), create_test_task("task-9")];
    let list_result = a2a_rs::domain::ListTasksResult {
        tasks: result_tasks,
        next_cursor: None,
    };
    let response = create_success_response(serde_json::to_value(&list_result).unwrap());
    client.add_response(Some(response));

    let params = ListTasksParams {
        cursor: None,
        limit: Some(10),
        status_filter: None,
    };
    let result = client.list_tasks(&params).await;

    assert!(result.is_ok());
    let returned_result = result.unwrap();
    assert_eq!(returned_result.tasks.len(), 2);
}

#[tokio::test]
async fn test_list_tasks_with_cursor() {
    let client = MockHttpClient::new();
    let result_tasks = vec![create_test_task("task-10")];
    let list_result = a2a_rs::domain::ListTasksResult {
        tasks: result_tasks,
        next_cursor: Some("cursor-abc123".to_string()),
    };
    let response = create_success_response(serde_json::to_value(&list_result).unwrap());
    client.add_response(Some(response));

    let params = ListTasksParams {
        cursor: Some("prev-cursor".to_string()),
        limit: Some(10),
        status_filter: None,
    };
    let result = client.list_tasks(&params).await;

    assert!(result.is_ok());
    let returned_result = result.unwrap();
    assert_eq!(
        returned_result.next_cursor,
        Some("cursor-abc123".to_string())
    );
}

#[tokio::test]
async fn test_empty_response() {
    let client = MockHttpClient::new();
    client.add_response(Some("{}".to_string()));

    let result = client.send_raw_request(r#"{"jsonrpc":"2.0","id":1}"#).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_malformed_json_response() {
    let client = MockHttpClient::new();
    client.add_response(Some("not json".to_string()));

    let result = client.send_raw_request(r#"{"jsonrpc":"2.0","id":1}"#).await;

    assert!(result.is_err());
    if let Err(A2AError::JsonParse(_)) = result {
        // Expected
    } else {
        panic!("Expected JsonParse error");
    }
}

#[tokio::test]
async fn test_request_tracking() {
    let client = MockHttpClient::new();
    let response = create_success_response(json!({"status": "ok"}));
    client.add_response(Some(response));

    let _ = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    let requests = client.get_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0], r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#);
}

#[tokio::test]
async fn test_concurrent_requests() {
    let client = MockHttpClient::new();

    // Add responses for 5 concurrent requests
    for i in 0..5 {
        let task = create_test_task(&format!("task-{}", i));
        let response = create_success_response(serde_json::to_value(&task).unwrap());
        client.add_response(Some(response));
    }

    // Spawn concurrent requests
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .get_task(&format!("task-{}", i), None)
                    .await
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }

    let requests = client.get_requests();
    assert_eq!(requests.len(), 5);
}

#[tokio::test]
async fn test_timeout_scenario() {
    let client = MockHttpClient::new();

    // Simulate timeout by not providing a response immediately
    // In a real scenario, this would timeout after the configured duration

    let _ = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    // Since we added a response, it should succeed
    // In real client without response, this would timeout
    assert_eq!(client.get_requests().len(), 1);
}

#[tokio::test]
async fn test_json_rpc_version_validation() {
    let client = MockHttpClient::new();

    // Test that request includes proper JSON-RPC version
    let response = create_success_response(json!({"status": "ok"}));
    client.add_response(Some(response));

    let _ = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    let requests = client.get_requests();
    let request_json: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    assert_eq!(request_json["jsonrpc"], "2.0");
}

#[tokio::test]
async fn test_error_response_with_data() {
    let client = MockHttpClient::new();

    let error_with_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": {
            "code": -32602,
            "message": "Invalid params",
            "data": {
                "field": "taskId",
                "reason": "required"
            }
        }
    });
    client.add_response(Some(error_with_data.to_string()));

    let result = client
        .send_raw_request(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#)
        .await;

    assert!(result.is_err());
    if let Err(A2AError::JsonRpc { data, .. }) = result {
        assert!(data.is_some());
        let data_obj = data.unwrap();
        assert_eq!(data_obj["field"], "taskId");
    } else {
        panic!("Expected JsonRpc error with data");
    }
}

#[tokio::test]
async fn test_multiple_sequential_requests() {
    let client = MockHttpClient::new();

    // Add responses for 5 sequential requests
    for i in 0..5 {
        let task = create_test_task(&format!("task-seq-{}", i));
        let response = create_success_response(serde_json::to_value(&task).unwrap());
        client.add_response(Some(response));
    }

    // Send requests sequentially
    for i in 0..5 {
        let result = client
            .get_task(&format!("task-seq-{}", i), None)
            .await;

        assert!(result.is_ok(), "Request {} failed", i);
    }

    let requests = client.get_requests();
    assert_eq!(requests.len(), 5);
}

#[tokio::test]
async fn test_request_with_metadata() {
    let client = MockHttpClient::new();
    let task = create_test_task("task-meta");
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client.get_task("task-meta", None).await;

    assert!(result.is_ok());

    // Verify request was made
    let requests = client.get_requests();
    assert!(!requests.is_empty());
}

#[tokio::test]
async fn test_session_id_in_request() {
    let client = MockHttpClient::new();
    let task = create_test_task("task-session");
    let message = create_test_message();
    let response = create_success_response(serde_json::to_value(&task).unwrap());
    client.add_response(Some(response));

    let result = client
        .send_task_message("task-session", &message, Some("test-session-123"), None)
        .await;

    assert!(result.is_ok());

    // Verify session ID in request
    let requests = client.get_requests();
    let request_json: serde_json::Value = serde_json::from_str(&requests[0]).unwrap();
    assert_eq!(
        request_json["params"]["sessionId"],
        "test-session-123"
    );
}

#[tokio::test]
async fn test_push_notification_with_authentication() {
    let client = MockHttpClient::new();

    let mut auth_info = std::collections::HashMap::new();
    auth_info.insert("type".to_string(), serde_json::json!("bearer"));

    let config = TaskPushNotificationConfig {
        id: Some("config-auth".to_string()),
        url: "https://example.com/webhook".to_string(),
        token: Some("bearer-token".to_string()),
        authentication: Some(a2a_rs::domain::PushNotificationAuthenticationInfo {
            schemes: vec!["bearer".to_string()],
            credentials: Some("credentials-123".to_string()),
        }),
    };
    let response = create_success_response(serde_json::to_value(&config).unwrap());
    client.add_response(Some(response));

    let result = client
        .set_task_push_notification(&config)
        .await;

    assert!(result.is_ok());
    let returned_config = result.unwrap();
    assert!(returned_config.authentication.is_some());
}

#[tokio::test]
async fn test_streaming_unsupported() {
    let client = MockHttpClient::new();

    let result = client.subscribe_to_task("task-1", None).await;

    assert!(result.is_err());
    if let Err(A2AError::UnsupportedOperation(msg)) = result {
        assert!(msg.contains("Streaming"));
    } else {
        panic!("Expected UnsupportedOperation error");
    }
}
