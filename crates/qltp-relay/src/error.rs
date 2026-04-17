//! Error types for QLTP Relay Service

use std::fmt;

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for relay service
#[derive(Debug, Clone)]
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
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

// Made with Bob
