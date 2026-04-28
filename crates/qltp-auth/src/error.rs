//! Error types for authentication

use thiserror::Error;
use std::time::Duration;

/// Authentication errors
#[derive(Error, Debug)]
#[non_exhaustive]
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

    /// Too many recent attempts — caller is being rate-limited.
    #[error("Rate limit exceeded; retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },

    /// Internal error (lock poisoning, etc.)
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for authentication operations
pub type Result<T> = std::result::Result<T, AuthError>;

// Made with Bob
