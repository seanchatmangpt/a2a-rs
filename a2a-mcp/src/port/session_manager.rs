//! Session management port definitions
//!
//! Defines the interface for managing MCP sessions, independent of implementation.

use async_trait::async_trait;

use crate::{domain::Session, error::Result};

/// Trait for managing MCP session lifecycle and state
#[async_trait]
pub trait SessionManager: Send + Sync {
    /// Create a new session with the given ID
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for the session
    ///
    /// # Returns
    /// * `Ok(Session)` - The newly created session
    /// * `Err(Error)` - If a session with this ID already exists
    async fn create_session(&self, session_id: String) -> Result<Session>;

    /// Get a session by ID
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Ok(Some(Session))` - The session if found
    /// * `Ok(None)` - If no session exists with this ID
    /// * `Err(Error)` - If an error occurs during retrieval
    async fn get_session(&self, session_id: &str) -> Result<Option<Session>>;

    /// Get or create a session by ID
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Ok((Session, bool))` - The session and a boolean indicating if it was newly created
    /// * `Err(Error)` - If an error occurs
    async fn get_or_create_session(&self, session_id: String) -> Result<(Session, bool)>;

    /// Update an existing session
    ///
    /// # Arguments
    /// * `session` - The session to update
    ///
    /// # Returns
    /// * `Ok(())` - If the session was successfully updated
    /// * `Err(Error)` - If the session doesn't exist or an error occurs
    async fn update_session(&self, session: Session) -> Result<()>;

    /// Touch a session to update its last accessed time
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Ok(())` - If the session was successfully touched
    /// * `Err(Error)` - If the session doesn't exist or an error occurs
    async fn touch_session(&self, session_id: &str) -> Result<()>;

    /// Delete a session by ID
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Ok(bool)` - True if a session was deleted, false if no session existed
    /// * `Err(Error)` - If an error occurs during deletion
    async fn delete_session(&self, session_id: &str) -> Result<bool>;

    /// Delete all expired sessions
    ///
    /// # Returns
    /// * `Ok(usize)` - The number of sessions deleted
    /// * `Err(Error)` - If an error occurs during cleanup
    async fn cleanup_expired_sessions(&self) -> Result<usize>;

    /// List all active session IDs
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - List of all active session IDs
    /// * `Err(Error)` - If an error occurs during listing
    async fn list_sessions(&self) -> Result<Vec<String>>;

    /// Get the count of active sessions
    ///
    /// # Returns
    /// * `Ok(usize)` - Number of active sessions
    /// * `Err(Error)` - If an error occurs
    async fn count_sessions(&self) -> Result<usize>;
}
