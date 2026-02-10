//! Example demonstrating MCP tasks primitive
//!
//! This example shows how to:
//! 1. Create long-running operations as MCP tasks
//! 2. Poll task status with tasks/get
//! 3. Retrieve results with tasks/result
//! 4. Bridge MCP tasks to A2A task model

use a2a_mcp::{JsonRpcRequest, JsonRpcResponse, McpTaskHandler, McpTaskManager, TaskWrapper};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== MCP Tasks Primitive Example ===\n");

    // Create task wrapper (adapter implementing McpTaskManager port)
    let task_wrapper = Arc::new(TaskWrapper::new());

    // Create JSON-RPC handler
    let handler = McpTaskHandler::new(task_wrapper.clone());

    // Example 1: Create a long-running task
    println!("1. Creating a long-running task...");
    let task = task_wrapper
        .create_task(|| async {
            println!("   Task started, simulating work...");
            sleep(Duration::from_secs(2)).await;
            println!("   Task completed!");
            Ok(json!({
                "status": "success",
                "data": "Processing complete",
                "processedItems": 42
            }))
        })
        .await?;

    println!("   Created task with ID: {}\n", task.id);

    // Example 2: Poll task status using JSON-RPC tasks/get
    println!("2. Polling task status with tasks/get...");
    let get_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(1.into())),
        method: "tasks/get".to_string(),
        params: Some(json!({ "taskId": task.id })),
    };

    let response = handler.handle_request(get_request.clone()).await;
    println!("   Initial status: {:?}\n", response.result);

    // Wait a bit for task to progress
    sleep(Duration::from_millis(500)).await;

    let response = handler.handle_request(get_request).await;
    println!("   Mid-execution status: {:?}\n", response.result);

    // Example 3: Wait for completion and get result
    println!("3. Waiting for task completion...");
    sleep(Duration::from_secs(2)).await;

    let result_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(2.into())),
        method: "tasks/result".to_string(),
        params: Some(json!({ "taskId": task.id })),
    };

    let response = handler.handle_request(result_request).await;
    println!(
        "   Task result: {}\n",
        serde_json::to_string_pretty(&response)?
    );

    // Example 4: Create multiple tasks and list them
    println!("4. Creating multiple tasks...");
    let task1 = task_wrapper
        .create_task(|| async {
            sleep(Duration::from_millis(100)).await;
            Ok(json!("Task 1 result"))
        })
        .await?;
    println!("   Created task: {}", task1.id);

    let task2 = task_wrapper
        .create_task(|| async {
            sleep(Duration::from_millis(200)).await;
            Ok(json!("Task 2 result"))
        })
        .await?;
    println!("   Created task: {}\n", task2.id);

    // List all tasks
    let list_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(3.into())),
        method: "tasks/list".to_string(),
        params: None,
    };

    let response = handler.handle_request(list_request).await;
    println!("5. Listing all tasks:");
    println!("   {}\n", serde_json::to_string_pretty(&response)?);

    // Example 5: Cancel a task
    println!("6. Testing task cancellation...");
    let long_task = task_wrapper
        .create_task(|| async {
            sleep(Duration::from_secs(10)).await;
            Ok(json!("This should not complete"))
        })
        .await?;
    println!("   Created long-running task: {}", long_task.id);

    sleep(Duration::from_millis(100)).await;

    let cancel_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(4.into())),
        method: "tasks/cancel".to_string(),
        params: Some(json!({ "taskId": long_task.id })),
    };

    let response = handler.handle_request(cancel_request).await;
    println!("   Cancel response: {:?}\n", response);

    // Verify cancellation
    let get_cancelled = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(5.into())),
        method: "tasks/get".to_string(),
        params: Some(json!({ "taskId": long_task.id })),
    };

    let response = handler.handle_request(get_cancelled).await;
    println!(
        "   Cancelled task status: {}\n",
        serde_json::to_string_pretty(&response)?
    );

    // Example 6: Task error handling
    println!("7. Testing task error handling...");
    let failing_task = task_wrapper
        .create_task(|| async {
            sleep(Duration::from_millis(100)).await;
            Err(a2a_mcp::Error::TaskProcessing(
                "Simulated task failure".to_string(),
            ))
        })
        .await?;

    sleep(Duration::from_millis(200)).await;

    let error_result = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(6.into())),
        method: "tasks/result".to_string(),
        params: Some(json!({ "taskId": failing_task.id })),
    };

    let response = handler.handle_request(error_result).await;
    println!(
        "   Failed task result: {}\n",
        serde_json::to_string_pretty(&response)?
    );

    // Example 7: Cleanup old tasks
    println!("8. Testing task cleanup...");
    sleep(Duration::from_millis(500)).await;

    let cleanup_count = task_wrapper.cleanup_old_tasks(0).await?;
    println!("   Cleaned up {} old tasks\n", cleanup_count);

    let list_after_cleanup = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(Value::Number(7.into())),
        method: "tasks/list".to_string(),
        params: None,
    };

    let response = handler.handle_request(list_after_cleanup).await;
    println!(
        "   Remaining tasks after cleanup: {}\n",
        serde_json::to_string_pretty(&response)?
    );

    println!("=== Example Complete ===");

    Ok(())
}
