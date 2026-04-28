//! Unified error types for the transport layer
//!
//! This module provides a comprehensive error hierarchy that covers:
//! - Domain-level errors (sessions, backends, state transitions)
//! - Adapter-level errors (backend-specific failures)
//! - Protocol-level errors (message parsing, serialization)
//! - Network-level errors (connections, I/O, timeouts)
//! - Transfer-level errors (file transfers, resume, parallel)
//! - Security errors (TLS, authentication)

use thiserror::Error;

/// Unified transport layer errors
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    // ============================================================================
    // Domain Errors - Business logic and state management
    // ============================================================================
    
    /// Generic domain error
    #[error("Domain error: {0}")]
    Domain(String),

    /// Backend is not available on this platform
    #[error("Backend not available: {0:?}")]
    BackendNotAvailable(crate::domain::TransportType),

    /// Invalid state transition attempted
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: crate::domain::SessionState,
        to: crate::domain::SessionState,
    },

    /// Session is not in active state
    #[error("Session not active")]
    SessionNotActive,

    /// Session with given ID not found
    #[error("Session not found: {0}")]
    SessionNotFound(crate::domain::SessionId),

    // ============================================================================
    // Adapter Errors - Backend-specific failures
    // ============================================================================
    
    /// Generic adapter error
    #[error("Adapter error: {0}")]
    Adapter(String),

    /// Backend type doesn't match adapter
    #[error("Invalid backend type for adapter")]
    InvalidBackendType,

    /// Connection establishment failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Send operation failed
    #[error("Send failed: {0}")]
    SendFailed(String),

    /// Receive operation failed
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    /// Out of memory (io_uring buffers, etc.)
    #[error("Out of memory")]
    OutOfMemory,

    /// Queue is full (submission queue, etc.)
    #[error("Queue full")]
    QueueFull,

    /// No completion available
    #[error("No completion")]
    NoCompletion,

    // ============================================================================
    // I/O and Network Errors
    // ============================================================================
    
    /// Standard I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid network address
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// No active connection
    #[error("No connection")]
    NoConnection,

    /// Network connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Operation timed out
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Resource exhausted (bandwidth, connections, etc.)
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    // ============================================================================
    // Protocol Errors - Message parsing and validation
    // ============================================================================
    
    /// Protocol-level error
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Message serialization/deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid message format or content
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Protocol version mismatch
    #[error("Unsupported version: expected {expected}, got {actual}")]
    UnsupportedVersion { expected: u8, actual: u8 },

    /// Checksum validation failed
    #[error("Checksum mismatch")]
    ChecksumMismatch,

    /// Invalid input provided
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // ============================================================================
    // Transfer Errors - File transfer operations
    // ============================================================================
    
    /// File transfer error
    #[error("Transfer error: {0}")]
    Transfer(String),

    // ============================================================================
    // Security Errors
    // ============================================================================
    
    /// TLS/SSL error
    #[error("TLS error: {0}")]
    Tls(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Authentication(String),

    // ============================================================================
    // Configuration Errors
    // ============================================================================
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Feature not supported
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Invalid kernel version for io_uring
    #[error("Invalid kernel version")]
    InvalidKernelVersion,

    /// DPDK initialization failed
    #[error("DPDK initialization failed: {0}")]
    DpdkInit(String),

    // ============================================================================
    // External Error Conversions
    // ============================================================================
    
    /// UUID parsing error
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    // ============================================================================
    // Generic Errors
    // ============================================================================
    
    /// Generic transport error
    #[error("Transport error: {0}")]
    Other(String),
}

/// Result type for transport operations
pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// Error Conversions
// ============================================================================

impl From<bincode::Error> for Error {
    fn from(err: bincode::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_display() {
        let err = Error::Protocol("test error".to_string());
        assert_eq!(err.to_string(), "Protocol error: test error");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let transport_err: Error = io_err.into();
        assert!(matches!(transport_err, Error::Io(_)));
    }

    #[test]
    fn test_bincode_error_conversion() {
        // Create a serialization error by trying to serialize something that will fail
        let data: Vec<u8> = vec![0xFF; 10];
        let result: Result<String> = bincode::deserialize(&data)
            .map_err(|e| e.into());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::Serialization(_)));
    }

    #[test]
    fn test_uuid_error_conversion() {
        let uuid_err = uuid::Uuid::parse_str("invalid-uuid");
        assert!(uuid_err.is_err());
        let transport_err: Error = uuid_err.unwrap_err().into();
        assert!(matches!(transport_err, Error::Uuid(_)));
    }

    #[test]
    fn test_version_mismatch_error() {
        let err = Error::UnsupportedVersion {
            expected: 1,
            actual: 2,
        };
        assert_eq!(err.to_string(), "Unsupported version: expected 1, got 2");
    }

    #[test]
    fn test_state_transition_error() {
        use crate::domain::SessionState;
        let err = Error::InvalidStateTransition {
            from: SessionState::Initializing,
            to: SessionState::Completed,
        };
        assert!(err.to_string().contains("Invalid state transition"));
    }
}

// Made with Bob
