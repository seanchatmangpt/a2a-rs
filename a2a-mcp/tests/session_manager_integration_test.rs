//! Integration tests for session management
//!
//! Tests the full session management stack: domain types, port traits, and adapter implementation.

use a2a_mcp::{InMemorySessionManager, Session, SessionManager};
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_session_lifecycle() {
    let manager = InMemorySessionManager::new();

    // Create session
    let session = manager
        .create_session("test-session".to_string())
        .await
        .unwrap();
    assert_eq!(session.id, "test-session");

    // Retrieve session
    let retrieved = manager.get_session("test-session").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "test-session");

    // Delete session
    let deleted = manager.delete_session("test-session").await.unwrap();
    assert!(deleted);

    // Verify deletion
    let missing = manager.get_session("test-session").await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_get_or_create_idempotent() {
    let manager = InMemorySessionManager::new();

    // First call creates
    let (session1, created1) = manager
        .get_or_create_session("test-session".to_string())
        .await
        .unwrap();
    assert!(created1);
    assert_eq!(session1.id, "test-session");

    // Second call retrieves existing
    let (session2, created2) = manager
        .get_or_create_session("test-session".to_string())
        .await
        .unwrap();
    assert!(!created2);
    assert_eq!(session2.id, "test-session");
    assert_eq!(session1.created_at, session2.created_at);
}

#[tokio::test]
async fn test_session_state_management() {
    let manager = InMemorySessionManager::new();

    let (mut session, _) = manager
        .get_or_create_session("state-session".to_string())
        .await
        .unwrap();

    // Add state
    session.state = Some(serde_json::json!({
        "key1": "value1",
        "key2": 42
    }));

    manager.update_session(session.clone()).await.unwrap();

    // Retrieve and verify state
    let retrieved = manager.get_session("state-session").await.unwrap().unwrap();
    assert!(retrieved.state.is_some());

    let state = retrieved.state.unwrap();
    assert_eq!(state["key1"], "value1");
    assert_eq!(state["key2"], 42);
}

#[tokio::test]
async fn test_concurrent_session_access() {
    let manager = Arc::new(InMemorySessionManager::new());

    // Create initial session
    manager
        .create_session("concurrent-test".to_string())
        .await
        .unwrap();

    let mut handles = vec![];

    // Spawn 10 tasks that concurrently access the session
    for i in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            // Touch the session
            manager_clone
                .touch_session("concurrent-test")
                .await
                .unwrap();

            // Read the session
            let session = manager_clone
                .get_session("concurrent-test")
                .await
                .unwrap()
                .unwrap();

            session.id
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert_eq!(result, "concurrent-test");
    }

    // Verify session still exists
    let final_session = manager.get_session("concurrent-test").await.unwrap();
    assert!(final_session.is_some());
}

#[tokio::test]
async fn test_session_expiration() {
    let manager = InMemorySessionManager::new();

    // Create expired session
    let mut expired = manager
        .create_session("expired-session".to_string())
        .await
        .unwrap();
    expired.ttl_seconds = Some(0);
    manager.update_session(expired).await.unwrap();

    // Create active session
    manager
        .create_session("active-session".to_string())
        .await
        .unwrap();

    assert_eq!(manager.count_sessions().await.unwrap(), 2);

    // Clean up expired sessions
    let cleaned = manager.cleanup_expired_sessions().await.unwrap();
    assert_eq!(cleaned, 1);

    // Verify active session remains
    assert_eq!(manager.count_sessions().await.unwrap(), 1);
    let active = manager.get_session("active-session").await.unwrap();
    assert!(active.is_some());

    let expired = manager.get_session("expired-session").await.unwrap();
    assert!(expired.is_none());
}

#[tokio::test]
async fn test_list_and_count_sessions() {
    let manager = InMemorySessionManager::new();

    // Start with empty manager
    assert_eq!(manager.count_sessions().await.unwrap(), 0);
    assert_eq!(manager.list_sessions().await.unwrap().len(), 0);

    // Create multiple sessions
    for i in 1..=5 {
        manager
            .create_session(format!("session-{}", i))
            .await
            .unwrap();
    }

    // Verify count
    assert_eq!(manager.count_sessions().await.unwrap(), 5);

    // Verify list
    let sessions = manager.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 5);
    assert!(sessions.contains(&"session-1".to_string()));
    assert!(sessions.contains(&"session-5".to_string()));
}

#[tokio::test]
async fn test_touch_updates_last_accessed() {
    let manager = InMemorySessionManager::new();

    let session = manager
        .create_session("touch-test".to_string())
        .await
        .unwrap();
    let initial_access = session.last_accessed;

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Touch the session
    manager.touch_session("touch-test").await.unwrap();

    // Verify last_accessed was updated
    let updated = manager.get_session("touch-test").await.unwrap().unwrap();
    assert!(updated.last_accessed > initial_access);
}
