//! Error types for the query optimizer.

/// Result type alias for optimizer operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during query optimization.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// Failed to parse the SPARQL query.
    #[error("Parse error at position {position}: {message}")]
    ParseError {
        /// The position in the input where parsing failed.
        position: usize,
        /// Description of what went wrong.
        message: String,
    },

    /// Query contains semantic errors.
    #[error("Semantic error: {0}")]
    SemanticError(String),

    /// Cost model computation failed.
    #[error("Cost calculation error: {0}")]
    CostError(String),

    /// Query rewriting failed.
    #[error("Rewrite error: {0}")]
    RewriteError(String),

    /// Internal optimizer error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Query contains unsupported features.
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Invalid optimization configuration.
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

impl Error {
    /// Create a parse error at the given position.
    pub fn parse_error(position: usize, message: impl Into<String>) -> Self {
        Self::ParseError {
            position,
            message: message.into(),
        }
    }

    /// Create a semantic error.
    pub fn semantic(message: impl Into<String>) -> Self {
        Self::SemanticError(message.into())
    }

    /// Create a cost calculation error.
    pub fn cost(message: impl Into<String>) -> Self {
        Self::CostError(message.into())
    }

    /// Create a rewrite error.
    pub fn rewrite(message: impl Into<String>) -> Self {
        Self::RewriteError(message.into())
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::InternalError(message.into())
    }

    /// Create an unsupported feature error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::UnsupportedFeature(message.into())
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::ConfigError(message.into())
    }
}
