//! Error types for a2a-mcp integration

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Errors that can occur in a2a-mcp integration
#[derive(Error, Debug)]
pub enum Error {
    /// Error related to A2A protocol
    #[error("A2A error: {0}")]
    A2a(String),

    /// Error related to RMCP protocol
    #[error("RMCP error: {0}")]
    Rmcp(String),

    /// Error in protocol translation
    #[error("Protocol translation error: {0}")]
    Translation(String),

    /// Task not found
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Error in task processing
    #[error("Task processing error: {0}")]
    TaskProcessing(String),

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Invalid tool method format
    #[error("Invalid tool method format: {0}")]
    InvalidToolMethod(String),

    /// Server error
    #[error("Server error: {0}")]
    Server(String),

    /// RMCP tool call error
    #[error("RMCP tool call error: {0}")]
    RmcpToolCall(String),

    /// Origin validation failed - DNS rebinding defense
    #[error("Origin forbidden: {0}")]
    OriginForbidden(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already exists
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(String),

    /// Session error
    #[error("Session error: {0}")]
    Session(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for a2a-mcp operations
pub type Result<T> = std::result::Result<T, Error>;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Error::TaskNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            Error::AgentNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            Error::SessionNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            Error::InvalidToolMethod(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Error::OriginForbidden(_) => (StatusCode::FORBIDDEN, self.to_string()),
            Error::TaskProcessing(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Server(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Session(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            Error::Json(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        (status, message).into_response()
    }
}

/// Convenience function to convert a string error to an Error
pub fn err<E: ToString>(e: E) -> Error {
    Error::Translation(e.to_string())
}
