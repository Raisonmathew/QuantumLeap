//! Error types for authentication

use thiserror::Error;

/// Authentication errors
#[derive(Error, Debug)]
pub enum AuthError {
    /// Invalid credentials provided
    #[error("Invalid credentials")]
    InvalidCredentials,

    /// Invalid or unknown token
    #[error("Invalid token")]
    InvalidToken,

    /// Token has expired
    #[error("Token expired")]
    TokenExpired,

    /// Internal error (lock poisoning, etc.)
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for authentication operations
pub type Result<T> = std::result::Result<T, AuthError>;

// Made with Bob
