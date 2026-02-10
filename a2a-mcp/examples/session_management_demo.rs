//! Demo of MCP session management with MCP-Session-Id header handling
//!
//! This example demonstrates:
//! 1. Creating and managing sessions
//! 2. Binding MCP-Session-Id header to sessions
//! 3. Thread-safe concurrent session handling
//! 4. Session state management across requests

use a2a_mcp::{InMemorySessionManager, Session, SessionManager};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MCP Session Management Demo ===\n");

    // Create a session manager
    let session_manager = Arc::new(InMemorySessionManager::new());
    println!("✓ Created InMemorySessionManager\n");

    // Simulate receiving a request with MCP-Session-Id header
    let session_id = Uuid::new_v4().to_string();
    println!("Incoming request with MCP-Session-Id: {}", session_id);

    // Get or create session from header
    let (mut session, created) = session_manager
        .get_or_create_session(session_id.clone())
        .await?;

    if created {
        println!("✓ Created new session");
    } else {
        println!("✓ Retrieved existing session");
    }

    println!("  Session ID: {}", session.id);
    println!("  Created at: {}", session.created_at);
    println!("  Last accessed: {}", session.last_accessed);
    println!();

    // Store some state in the session
    session.metadata = Some(serde_json::json!({
        "client": "mcp-client-1.0",
        "user_agent": "Example/1.0"
    }));

    session.state = Some(serde_json::json!({
        "conversation_context": ["Hello", "How are you?"],
        "tool_state": {
            "last_tool_called": "list_files",
            "working_directory": "/home/user"
        }
    }));

    session.ttl_seconds = Some(3600); // 1 hour TTL

    // Update the session
    session_manager.update_session(session.clone()).await?;
    println!("✓ Updated session with metadata and state\n");

    // Simulate a second request with the same session ID
    println!("Second request with same MCP-Session-Id: {}", session_id);
    let (session2, _) = session_manager
        .get_or_create_session(session_id.clone())
        .await?;

    println!("✓ Retrieved session");
    println!("  Metadata: {}", session2.metadata.unwrap());
    println!("  State: {}", session2.state.unwrap());
    println!();

    // Touch the session to update last accessed time
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    session_manager.touch_session(&session_id).await?;
    println!("✓ Touched session (updated last accessed time)\n");

    let session3 = session_manager.get_session(&session_id).await?.unwrap();
    println!("  Idle time: {} ms", session3.idle_seconds() * 1000);
    println!();

    // Demonstrate concurrent session access
    println!("Demonstrating concurrent session access...");
    let manager_clone = session_manager.clone();
    let session_id_clone = session_id.clone();

    let handle1 = tokio::spawn(async move {
        for _ in 0..5 {
            manager_clone.touch_session(&session_id_clone).await.ok();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });

    let manager_clone2 = session_manager.clone();
    let session_id_clone2 = session_id.clone();

    let handle2 = tokio::spawn(async move {
        for _ in 0..5 {
            let _ = manager_clone2.get_session(&session_id_clone2).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });

    handle1.await?;
    handle2.await?;
    println!("✓ Concurrent access completed successfully\n");

    // List all sessions
    let sessions = session_manager.list_sessions().await?;
    println!("Active sessions: {}", sessions.len());
    for id in &sessions {
        println!("  - {}", id);
    }
    println!();

    // Create a few more sessions to demonstrate cleanup
    for i in 1..=3 {
        let mut expired_session = session_manager
            .create_session(format!("expired-{}", i))
            .await?;
        expired_session.ttl_seconds = Some(0); // Expired immediately
        session_manager.update_session(expired_session).await?;
    }

    println!("✓ Created 3 expired sessions");
    println!(
        "Total sessions before cleanup: {}",
        session_manager.count_sessions().await?
    );

    // Clean up expired sessions
    let cleaned = session_manager.cleanup_expired_sessions().await?;
    println!("✓ Cleaned up {} expired sessions", cleaned);
    println!(
        "Total sessions after cleanup: {}",
        session_manager.count_sessions().await?
    );
    println!();

    // Delete the main session
    let deleted = session_manager.delete_session(&session_id).await?;
    if deleted {
        println!("✓ Deleted session: {}", session_id);
    }

    println!(
        "\nFinal session count: {}",
        session_manager.count_sessions().await?
    );

    println!("\n=== Demo Complete ===");

    Ok(())
}
