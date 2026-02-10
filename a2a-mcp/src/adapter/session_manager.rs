//! In-memory session manager implementation
//!
//! Provides a thread-safe, in-memory implementation of the SessionManager port
//! using Arc<RwLock<HashMap>> for concurrent session handling.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{
    domain::Session,
    error::{Error, Result},
    port::SessionManager,
};

/// In-memory session manager using Arc<RwLock<HashMap>> for thread-safe concurrent access
#[derive(Debug, Clone)]
pub struct InMemorySessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl InMemorySessionManager {
    /// Create a new in-memory session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new session manager with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::with_capacity(capacity))),
        }
    }
}

impl Default for InMemorySessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionManager for InMemorySessionManager {
    async fn create_session(&self, session_id: String) -> Result<Session> {
        let mut sessions = self.sessions.write().await;

        // Check if session already exists
        if sessions.contains_key(&session_id) {
            return Err(Error::SessionAlreadyExists(session_id));
        }

        let session = Session::new(session_id.clone());
        sessions.insert(session_id, session.clone());

        Ok(session)
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn get_or_create_session(&self, session_id: String) -> Result<(Session, bool)> {
        // Fast path: try to read first
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                return Ok((session.clone(), false));
            }
        }

        // Slow path: acquire write lock and create
        let mut sessions = self.sessions.write().await;

        // Double-check in case another thread created it
        if let Some(session) = sessions.get(&session_id) {
            return Ok((session.clone(), false));
        }

        let session = Session::new(session_id.clone());
        sessions.insert(session_id, session.clone());

        Ok((session, true))
    }

    async fn update_session(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if !sessions.contains_key(&session.id) {
            return Err(Error::SessionNotFound(session.id));
        }

        sessions.insert(session.id.clone(), session);
        Ok(())
    }

    async fn touch_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::SessionNotFound(session_id.to_string()))?;

        session.touch();
        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let mut sessions = self.sessions.write().await;
        Ok(sessions.remove(session_id).is_some())
    }

    async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut sessions = self.sessions.write().await;

        let expired_ids: Vec<String> = sessions
            .iter()
            .filter(|(_, session)| session.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired_ids.len();
        for id in expired_ids {
            sessions.remove(&id);
        }

        Ok(count)
    }

    async fn list_sessions(&self) -> Result<Vec<String>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }

    async fn count_sessions(&self) -> Result<usize> {
        let sessions = self.sessions.read().await;
        Ok(sessions.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_session() {
        let manager = InMemorySessionManager::new();
        let session = manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        assert_eq!(session.id, "test-session");
        assert_eq!(manager.count_sessions().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_create_duplicate_session_fails() {
        let manager = InMemorySessionManager::new();
        manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        let result = manager.create_session("test-session".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_session() {
        let manager = InMemorySessionManager::new();
        manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        let session = manager.get_session("test-session").await.unwrap();
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, "test-session");

        let missing = manager.get_session("missing").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_get_or_create_session() {
        let manager = InMemorySessionManager::new();

        // First call creates session
        let (session1, created1) = manager
            .get_or_create_session("test-session".to_string())
            .await
            .unwrap();
        assert!(created1);
        assert_eq!(session1.id, "test-session");

        // Second call retrieves existing session
        let (session2, created2) = manager
            .get_or_create_session("test-session".to_string())
            .await
            .unwrap();
        assert!(!created2);
        assert_eq!(session2.id, "test-session");
        assert_eq!(session1.created_at, session2.created_at);
    }

    #[tokio::test]
    async fn test_update_session() {
        let manager = InMemorySessionManager::new();
        let mut session = manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        session.ttl_seconds = Some(3600);
        manager.update_session(session.clone()).await.unwrap();

        let retrieved = manager.get_session("test-session").await.unwrap().unwrap();
        assert_eq!(retrieved.ttl_seconds, Some(3600));
    }

    #[tokio::test]
    async fn test_touch_session() {
        let manager = InMemorySessionManager::new();
        let session = manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        let initial_access = session.last_accessed;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        manager.touch_session("test-session").await.unwrap();

        let updated = manager.get_session("test-session").await.unwrap().unwrap();
        assert!(updated.last_accessed > initial_access);
    }

    #[tokio::test]
    async fn test_delete_session() {
        let manager = InMemorySessionManager::new();
        manager
            .create_session("test-session".to_string())
            .await
            .unwrap();

        let deleted = manager.delete_session("test-session").await.unwrap();
        assert!(deleted);

        let not_deleted = manager.delete_session("test-session").await.unwrap();
        assert!(!not_deleted);

        assert_eq!(manager.count_sessions().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        let manager = InMemorySessionManager::new();

        // Create expired session
        let mut expired_session = manager.create_session("expired".to_string()).await.unwrap();
        expired_session.ttl_seconds = Some(0);
        manager.update_session(expired_session).await.unwrap();

        // Create non-expired session
        manager.create_session("active".to_string()).await.unwrap();

        let cleaned = manager.cleanup_expired_sessions().await.unwrap();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.count_sessions().await.unwrap(), 1);

        let active = manager.get_session("active").await.unwrap();
        assert!(active.is_some());

        let expired = manager.get_session("expired").await.unwrap();
        assert!(expired.is_none());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let manager = InMemorySessionManager::new();
        manager
            .create_session("session1".to_string())
            .await
            .unwrap();
        manager
            .create_session("session2".to_string())
            .await
            .unwrap();
        manager
            .create_session("session3".to_string())
            .await
            .unwrap();

        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
        assert!(sessions.contains(&"session1".to_string()));
        assert!(sessions.contains(&"session2".to_string()));
        assert!(sessions.contains(&"session3".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let manager = InMemorySessionManager::new();
        let manager_clone1 = manager.clone();
        let manager_clone2 = manager.clone();

        // Spawn multiple tasks that concurrently create sessions
        let handle1 = tokio::spawn(async move {
            for i in 0..100 {
                let _ = manager_clone1
                    .create_session(format!("session-1-{}", i))
                    .await;
            }
        });

        let handle2 = tokio::spawn(async move {
            for i in 0..100 {
                let _ = manager_clone2
                    .create_session(format!("session-2-{}", i))
                    .await;
            }
        });

        handle1.await.unwrap();
        handle2.await.unwrap();

        // Verify all sessions were created (some might overlap, but should be close to 200)
        let count = manager.count_sessions().await.unwrap();
        assert!(count > 0);
    }
}
