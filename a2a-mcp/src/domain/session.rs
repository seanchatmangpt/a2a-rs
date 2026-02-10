//! Session domain types for MCP session management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents an MCP session with associated state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Unique session identifier (MCP-Session-Id header value)
    pub id: String,

    /// Timestamp when the session was created
    pub created_at: DateTime<Utc>,

    /// Timestamp when the session was last accessed
    pub last_accessed: DateTime<Utc>,

    /// Optional session metadata (e.g., client info, configuration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,

    /// Session-specific state data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,

    /// Time-to-live in seconds (None means no expiration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

impl Session {
    /// Create a new session with the given ID
    pub fn new(id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            created_at: now,
            last_accessed: now,
            metadata: None,
            state: None,
            ttl_seconds: None,
        }
    }

    /// Update the last accessed timestamp to now
    pub fn touch(&mut self) {
        self.last_accessed = Utc::now();
    }

    /// Check if the session has expired based on TTL
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let age = Utc::now()
                .signed_duration_since(self.last_accessed)
                .num_seconds();
            age > ttl as i64
        } else {
            false
        }
    }

    /// Get the age of the session in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds()
    }

    /// Get seconds since last access
    pub fn idle_seconds(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.last_accessed)
            .num_seconds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session() {
        let session = Session::new("test-session-123".to_string());
        assert_eq!(session.id, "test-session-123");
        assert!(session.metadata.is_none());
        assert!(session.state.is_none());
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_expiration() {
        let mut session = Session::new("test-session".to_string());
        session.ttl_seconds = Some(0);
        assert!(session.is_expired());

        session.ttl_seconds = Some(3600);
        assert!(!session.is_expired());
    }

    #[test]
    fn test_touch_updates_last_accessed() {
        let mut session = Session::new("test-session".to_string());
        let initial_access = session.last_accessed;

        std::thread::sleep(std::time::Duration::from_millis(10));
        session.touch();

        assert!(session.last_accessed > initial_access);
    }
}
