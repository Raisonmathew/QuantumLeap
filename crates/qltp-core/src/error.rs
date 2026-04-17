//! Error types for QLTP core

use std::io;
use thiserror::Error;

/// Result type alias for QLTP operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for QLTP operations
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Compression error
    #[error("Compression error: {0}")]
    Compression(String),

    /// Decompression error
    #[error("Decompression error: {0}")]
    Decompression(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Hash mismatch
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Chunk error
    #[error("Chunk error: {0}")]
    Chunk(String),

    /// Transfer error
    #[error("Transfer error: {0}")]
    Transfer(String),

    /// Timeout error
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl Error {
    /// Create a new compression error
    pub fn compression(msg: impl Into<String>) -> Self {
        Self::Compression(msg.into())
    }

    /// Create a new decompression error
    pub fn decompression(msg: impl Into<String>) -> Self {
        Self::Decompression(msg.into())
    }

    /// Create a new network error
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    /// Create a new chunk error
    pub fn chunk(msg: impl Into<String>) -> Self {
        Self::Chunk(msg.into())
    }

    /// Create a new transfer error
    pub fn transfer(msg: impl Into<String>) -> Self {
        Self::Transfer(msg.into())
    }

    /// Create a new generic error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::compression("test error");
        assert_eq!(err.to_string(), "Compression error: test error");

        let err = Error::HashMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Hash mismatch: expected abc123, got def456"
        );
    }

    #[test]
    fn test_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }
}

// Made with Bob
