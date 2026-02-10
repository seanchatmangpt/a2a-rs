//! Example demonstrating SSE resumable streaming with the SseManager
//!
//! This example shows:
//! - Creating an SSE stream manager
//! - Publishing events to a stream
//! - Subscribing to a stream
//! - Resuming from a specific event ID (Last-Event-ID support)
//! - Automatic redelivery of missed events

use a2a_mcp::adapter::{SseManager, SseManagerConfig};
use chrono::Duration;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Create SSE manager with custom configuration
    let config = SseManagerConfig {
        max_events: 100,               // Keep last 100 events
        event_ttl: Duration::hours(1), // Events expire after 1 hour
        channel_capacity: 50,          // Broadcast channel capacity
    };
    let manager = SseManager::new(config);

    // Initialize a stream for a task
    let task_id = "task-123";
    manager.init_stream(task_id)?;
    println!("Initialized SSE stream for task: {}", task_id);

    // Publish some events to the stream
    let event_ids = vec![
        manager.publish(
            task_id,
            "task.created",
            serde_json::json!({"status": "created", "message": "Task initialized"}),
        )?,
        manager.publish(
            task_id,
            "task.status",
            serde_json::json!({"status": "working", "message": "Processing started"}),
        )?,
        manager.publish(
            task_id,
            "task.progress",
            serde_json::json!({"status": "working", "progress": 50}),
        )?,
    ];

    println!("Published {} events", event_ids.len());
    for id in &event_ids {
        println!("  - Event ID: {}", id);
    }

    // Example 1: Subscribe from the beginning (no Last-Event-ID)
    println!("\n=== Example 1: Subscribe from beginning ===");
    let mut stream = manager.subscribe(task_id, None)?;

    // Consume first 3 events
    for _ in 0..3 {
        if let Some(event) = stream.next().await {
            println!(
                "Received event: id={}, type={}, data={}",
                event.id, event.event, event.data
            );
        }
    }

    // Publish more events after initial subscription
    let new_event_id = manager.publish(
        task_id,
        "task.completed",
        serde_json::json!({"status": "completed", "message": "Task finished"}),
    )?;
    println!("\nPublished new event: {}", new_event_id);

    // Example 2: Resume from a specific event ID
    println!("\n=== Example 2: Resume from event ID {} ===", event_ids[1]);
    let mut resume_stream = manager.subscribe(task_id, Some(&event_ids[1]))?;

    // Should receive events after event_ids[1] (i.e., events 2, 3, and the new one)
    println!("Receiving missed events since {}:", event_ids[1]);
    for _ in 0..3 {
        if let Some(event) = resume_stream.next().await {
            println!(
                "Received event: id={}, type={}, data={}",
                event.id, event.event, event.data
            );
        }
    }

    // Example 3: Check redelivery window
    println!("\n=== Example 3: Inspect redelivery window ===");
    let all_events = manager.get_events(task_id)?;
    println!("Total events in redelivery window: {}", all_events.len());
    for event in &all_events {
        println!(
            "  - id={}, type={}, timestamp={}",
            event.id, event.event, event.timestamp
        );
    }

    // Example 4: Multiple subscribers
    println!("\n=== Example 4: Multiple concurrent subscribers ===");
    let mut subscriber1 = manager.subscribe(task_id, None)?;
    let mut subscriber2 = manager.subscribe(task_id, None)?;

    // Publish an event
    manager.publish(
        task_id,
        "broadcast.test",
        serde_json::json!({"message": "This goes to all subscribers"}),
    )?;

    // Both subscribers should receive it
    tokio::select! {
        Some(event) = subscriber1.next() => {
            if event.event == "broadcast.test" {
                println!("Subscriber 1 received broadcast: {}", event.data);
            }
        }
        Some(event) = subscriber2.next() => {
            if event.event == "broadcast.test" {
                println!("Subscriber 2 received broadcast: {}", event.data);
            }
        }
    }

    // Cleanup
    manager.close_stream(task_id)?;
    println!("\n=== Stream closed ===");
    println!("Active streams: {}", manager.active_stream_count());

    Ok(())
}
