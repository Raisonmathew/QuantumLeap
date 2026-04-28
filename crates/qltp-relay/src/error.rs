//! Error types for QLTP Relay Service

use std::fmt;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for relay service
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Error {
    /// Resource not found
    NotFound(String),
    /// Invalid state transition
    InvalidState(String),
    /// Invalid input
    InvalidInput(String),
    /// Connection error
    ConnectionError(String),
    /// Timeout error
    Timeout(String),
    /// Optimistic-concurrency conflict: a CAS write failed because the stored
    /// version did not match the expected version. Callers should reload the
    /// entity, re-apply their mutation, and retry. Bounded retry is provided
    /// by the `update_with_retry` helpers in the application/adapter layer.
    Conflict(String),
    /// Internal error
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Error::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Error::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            Error::Timeout(msg) => write!(f, "Timeout: {}", msg),
            Error::Conflict(msg) => write!(f, "Concurrency conflict: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

// Made with Bob
