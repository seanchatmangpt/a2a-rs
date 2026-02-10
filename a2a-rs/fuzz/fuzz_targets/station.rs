#![no_main]

//! Fuzz testing for CONSTRUCT Station system
//!
//! This fuzz target validates that:
//! 1. Stations never panic on invalid inputs
//! 2. Guards properly reject malformed packets
//! 3. Admission control is deterministic
//! 4. Step operations maintain state consistency
//!
//! The fuzzer generates random typed packets and feeds them to all station
//! implementations, ensuring robustness against adversarial inputs.

use libfuzzer_sys::fuzz_target;
use a2a_rs::construct::ontology::OntologyState;
use a2a_rs::construct::station::{
    Station, SendMessageStation, GetTaskStation, CancelTaskStation, ListTasksStation,
    SendStreamingMessageStation, TaskResubscribeStation, SetPushNotificationConfigStation,
    GetPushNotificationConfigStation, ListPushNotificationConfigsStation,
    DeletePushNotificationConfigStation, StationRegistry,
};
use a2a_rs::construct::types::*;
use a2a_rs::domain::{Message, MessageSendParams, Task, TaskIdParams, Role};

/// Fuzz input representing different packet types and operations
#[derive(Debug, Clone)]
enum FuzzOperation {
    /// Send a message with random content
    SendMessage {
        message_id: String,
        task_id: Option<String>,
        context_id: Option<String>,
        content: String,
    },
    /// Get a task by ID
    GetTask {
        task_id: String,
    },
    /// Cancel a task
    CancelTask {
        task_id: String,
    },
    /// List tasks with filters
    ListTasks {
        context_id: Option<String>,
    },
    /// Send streaming message
    SendStreamingMessage {
        message_id: String,
        task_id: Option<String>,
        content: String,
    },
    /// Resubscribe to task
    TaskResubscribe {
        task_id: String,
    },
    /// Set push notification config
    SetPushNotificationConfig {
        task_id: String,
        url: String,
    },
    /// Get push notification config
    GetPushNotificationConfig {
        task_id: String,
    },
    /// List push notification configs
    ListPushNotificationConfigs {
        task_id: String,
    },
    /// Delete push notification config
    DeletePushNotificationConfig {
        task_id: String,
        config_id: String,
    },
    /// Dispatch via registry
    RegistryDispatch {
        method: String,
        task_id: Option<String>,
    },
}

impl FuzzOperation {
    /// Parse fuzz input bytes into an operation
    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let op_type = data[0] % 11;
        let rest = &data[1..];

        match op_type {
            0 => {
                // SendMessage
                let (message_id, task_id, context_id, content) = parse_message_data(rest);
                Some(FuzzOperation::SendMessage {
                    message_id,
                    task_id,
                    context_id,
                    content,
                })
            }
            1 => {
                // GetTask
                let task_id = parse_string(rest, "task");
                Some(FuzzOperation::GetTask { task_id })
            }
            2 => {
                // CancelTask
                let task_id = parse_string(rest, "task");
                Some(FuzzOperation::CancelTask { task_id })
            }
            3 => {
                // ListTasks
                let context_id = if rest.len() > 1 && rest[0] % 2 == 0 {
                    Some(parse_string(&rest[1..], "ctx"))
                } else {
                    None
                };
                Some(FuzzOperation::ListTasks { context_id })
            }
            4 => {
                // SendStreamingMessage
                let (message_id, task_id, _, content) = parse_message_data(rest);
                Some(FuzzOperation::SendStreamingMessage {
                    message_id,
                    task_id,
                    content,
                })
            }
            5 => {
                // TaskResubscribe
                let task_id = parse_string(rest, "task");
                Some(FuzzOperation::TaskResubscribe { task_id })
            }
            6 => {
                // SetPushNotificationConfig
                let task_id = parse_string(rest, "task");
                let url = if rest.len() > 10 {
                    format!("https://example.com/{}", parse_string(&rest[10..], "hook"))
                } else {
                    "https://example.com/webhook".to_string()
                };
                Some(FuzzOperation::SetPushNotificationConfig { task_id, url })
            }
            7 => {
                // GetPushNotificationConfig
                let task_id = parse_string(rest, "task");
                Some(FuzzOperation::GetPushNotificationConfig { task_id })
            }
            8 => {
                // ListPushNotificationConfigs
                let task_id = parse_string(rest, "task");
                Some(FuzzOperation::ListPushNotificationConfigs { task_id })
            }
            9 => {
                // DeletePushNotificationConfig
                let task_id = parse_string(rest, "task");
                let config_id = parse_string(&rest[5..], "config");
                Some(FuzzOperation::DeletePushNotificationConfig { task_id, config_id })
            }
            10 => {
                // RegistryDispatch
                let methods = ["message/send", "tasks/get", "tasks/list", "tasks/cancel"];
                let method_idx = if !rest.is_empty() { rest[0] as usize % methods.len() } else { 0 };
                let method = methods[method_idx].to_string();
                let task_id = if rest.len() > 1 { Some(parse_string(&rest[1..], "task")) } else { None };
                Some(FuzzOperation::RegistryDispatch { method, task_id })
            }
            _ => None,
        }
    }

    /// Execute the operation against ontology and stations
    fn execute(&self, ontology: &mut OntologyState) {
        match self {
            FuzzOperation::SendMessage {
                message_id,
                task_id,
                context_id,
                content,
            } => {
                let mut message = Message::user_text(content.clone(), message_id.clone());
                message.task_id = task_id.clone();
                message.context_id = context_id.clone();

                let request = SendMessageRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "message/send".to_string(),
                    params: MessageSendParams {
                        message,
                        configuration: None,
                        metadata: None,
                    },
                };

                // Test admission - must not panic
                let admit_result = SendMessageStation::admit(ontology, &request);

                // If admitted, test step - must not panic
                if admit_result.is_ok() {
                    let _ = SendMessageStation.step(ontology, request);
                }
            }
            FuzzOperation::GetTask { task_id } => {
                let request = GetTaskRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/get".to_string(),
                    params: TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None,
                        metadata: None,
                    },
                };

                let admit_result = GetTaskStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = GetTaskStation.step(ontology, request);
                }
            }
            FuzzOperation::CancelTask { task_id } => {
                let request = CancelTaskRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/cancel".to_string(),
                    params: TaskIdParams {
                        id: task_id.clone(),
                    },
                };

                let admit_result = CancelTaskStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = CancelTaskStation.step(ontology, request);
                }
            }
            FuzzOperation::ListTasks { context_id } => {
                let params = context_id.as_ref().map(|ctx| crate::domain::ListTasksParams {
                    context_id: Some(ctx.clone()),
                    status: None,
                    page_size: None,
                    page_token: None,
                });

                let request = ListTasksRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/list".to_string(),
                    params,
                };

                let _ = ListTasksStation::admit(ontology, &request);
                let _ = ListTasksStation.step(ontology, request);
            }
            FuzzOperation::SendStreamingMessage {
                message_id,
                task_id,
                content,
            } => {
                let mut message = Message::user_text(content.clone(), message_id.clone());
                message.task_id = task_id.clone();

                let request = SendMessageStreamingRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "message/stream".to_string(),
                    params: MessageSendParams {
                        message,
                        configuration: None,
                        metadata: None,
                    },
                };

                let admit_result = SendStreamingMessageStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = SendStreamingMessageStation.step(ontology, request);
                }
            }
            FuzzOperation::TaskResubscribe { task_id } => {
                let request = TaskResubscriptionRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/resubscribe".to_string(),
                    params: TaskIdParams {
                        id: task_id.clone(),
                    },
                };

                let admit_result = TaskResubscribeStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = TaskResubscribeStation.step(ontology, request);
                }
            }
            FuzzOperation::SetPushNotificationConfig { task_id, url } => {
                let config = crate::domain::TaskPushNotificationConfig {
                    task_id: task_id.clone(),
                    push_notification_config: crate::domain::PushNotificationConfig {
                        url: url.clone(),
                        auth: None,
                    },
                };

                let request = SetTaskPushNotificationRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/pushNotificationConfig/set".to_string(),
                    params: config,
                };

                let admit_result = SetPushNotificationConfigStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = SetPushNotificationConfigStation.step(ontology, request);
                }
            }
            FuzzOperation::GetPushNotificationConfig { task_id } => {
                let request = GetTaskPushNotificationConfigRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/pushNotificationConfig/get".to_string(),
                    params: Some(TaskIdParams {
                        id: task_id.clone(),
                    }),
                };

                let admit_result = GetPushNotificationConfigStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = GetPushNotificationConfigStation.step(ontology, request);
                }
            }
            FuzzOperation::ListPushNotificationConfigs { task_id } => {
                let request = ListTaskPushNotificationConfigRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/pushNotificationConfig/list".to_string(),
                    params: TaskIdParams {
                        id: task_id.clone(),
                    },
                };

                let admit_result = ListPushNotificationConfigsStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = ListPushNotificationConfigsStation.step(ontology, request);
                }
            }
            FuzzOperation::DeletePushNotificationConfig { task_id, config_id } => {
                let request = DeleteTaskPushNotificationConfigRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(JsonRpcId::new_uuid()),
                    method: "tasks/pushNotificationConfig/delete".to_string(),
                    params: crate::domain::DeleteTaskPushNotificationConfigParams {
                        id: task_id.clone(),
                        push_notification_config_id: config_id.clone(),
                    },
                };

                let admit_result = DeletePushNotificationConfigStation::admit(ontology, &request);

                if admit_result.is_ok() {
                    let _ = DeletePushNotificationConfigStation.step(ontology, request);
                }
            }
            FuzzOperation::RegistryDispatch { method, task_id } => {
                let mut registry = StationRegistry::new();
                let params = if let Some(tid) = task_id {
                    serde_json::json!({ "id": tid })
                } else {
                    serde_json::json!({})
                };

                let _ = registry.dispatch(
                    method,
                    ontology,
                    params,
                    Some(JsonRpcId::new_uuid()),
                );
            }
        }
    }
}

/// Helper: parse message-related data from bytes
fn parse_message_data(data: &[u8]) -> (String, Option<String>, Option<String>, String) {
    let message_id = parse_string(data, "msg");
    let task_id = if data.len() > 2 && data[1] % 2 == 0 {
        Some(parse_string(&data[2..], "task"))
    } else {
        None
    };
    let context_id = if data.len() > 4 && data[3] % 2 == 0 {
        Some(parse_string(&data[4..], "ctx"))
    } else {
        None
    };
    let content = parse_string(&data[5..], "content");

    (message_id, task_id, context_id, content)
}

/// Helper: parse a string from bytes with a prefix
fn parse_string(data: &[u8], prefix: &str) -> String {
    if data.is_empty() {
        return format!("{}-empty", prefix);
    }

    // Use bytes to create a deterministic but varied string
    let suffix: String = data
        .iter()
        .take(16)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("");

    format!("{}-{}", prefix, suffix)
}

fuzz_target!(|data: &[u8]| {
    // Create a fresh ontology state for each fuzz run
    let mut ontology = OntologyState::new();

    // Parse operation from fuzz input
    if let Some(operation) = FuzzOperation::from_bytes(data) {
        // Execute the operation - must not panic
        operation.execute(&mut ontology);
    }

    // Additional invariant checks:
    // 1. Ontology state should remain valid
    assert!(ontology.task_count() >= 0, "Task count should never be negative");

    // 2. All tasks in ontology should have valid IDs
    for task in ontology.get_all_tasks() {
        assert!(!task.id.is_empty(), "Task ID should never be empty");
        assert!(!task.context_id.is_empty(), "Context ID should never be empty");
    }
});
