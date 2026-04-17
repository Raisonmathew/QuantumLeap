//! QLTP Transport Layer
//!
//! High-performance transport abstraction layer supporting multiple backends:
//! - TCP: Standard reliable transport (120 MB/s)
//! - QUIC: Modern UDP-based protocol with 0-RTT (1 GB/s)
//! - io_uring: Linux kernel bypass for maximum performance (8 GB/s)
//! - DPDK: Data Plane Development Kit for specialized hardware (10 GB/s)
//!
//! # Architecture
//!
//! This crate follows Domain-Driven Design (DDD) and Hexagonal Architecture:
//!
//! - **Domain Layer**: Core business logic (entities, value objects, aggregates)
//! - **Ports Layer**: Interfaces for external adapters
//! - **Application Layer**: Use cases and orchestration
//! - **Adapters Layer**: Concrete implementations of transport backends
//!
//! # Example
//!
//! ```rust,no_run
//! use qltp_transport::{TransportManager, TransportManagerConfig, SessionConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create transport manager
//!     let config = TransportManagerConfig::default();
//!     let manager = TransportManager::new(config);
//!
//!     // Initialize with a backend (TCP example)
//!     // let backend = TcpBackend::new();
//!     // manager.initialize(Box::new(backend)).await?;
//!
//!     // Create a session
//!     let session_config = SessionConfig::default();
//!     let session_id = manager.create_session(session_config).await?;
//!
//!     // Start the session
//!     manager.start_session(session_id).await?;
//!
//!     // Send data
//!     let data = b"Hello, QLTP!";
//!     manager.send(session_id, data).await?;
//!
//!     // Receive data
//!     let mut buffer = vec![0u8; 1024];
//!     let bytes_received = manager.receive(session_id, &mut buffer).await?;
//!
//!     // Get statistics
//!     let stats = manager.get_session_stats(session_id).await?;
//!     println!("Throughput: {:.2} MB/s", stats.throughput_mbps());
//!
//!     // Stop the session
//!     manager.stop_session(session_id).await?;
//!
//!     // Shutdown
//!     manager.shutdown().await?;
//!
//!     Ok(())
//! }
//! ```

// Re-export domain types
pub mod domain;
pub use domain::{
    BackendCapabilities, Platform, SessionConfig, SessionId, SessionState, TransportConnection,
    TransportSession, TransportStats, TransportType,
};

// Re-export ports (interfaces)
pub mod ports;
pub use ports::TransportBackend;

// Re-export application services
pub mod application;
pub use application::{TransferClient, TransferServer, TransportManager, TransportManagerConfig};

// Re-export infrastructure components
pub mod infrastructure;
pub use infrastructure::{BufferPool, BufferHandle, PoolStats, StorageIo, StorageIoConfig, IoStrategy};

// Re-export error types
pub mod error;
pub use error::{Error, Result};

// Protocol layer (moved from qltp-network)
pub mod protocol;
pub use protocol::{
    Capabilities, ChunkAckMessage, ChunkDataMessage, ChunkFlags, CompressionType, ErrorCode,
    ErrorMessage, HashAlgorithm, HelloMessage, Message, MessageHeader, MessageType,
    ProgressCallback, QltpCodec, ResumeAckMessage, ResumeRequestMessage, TransferAckMessage,
    TransferCompleteMessage, TransferConfig, TransferEndMessage, TransferProgress,
    TransferStartMessage, TransferStats, WelcomeMessage, DEFAULT_CHUNK_SIZE, MAX_PAYLOAD_SIZE,
    PROTOCOL_MAGIC, PROTOCOL_VERSION,
};

// Adapters layer
pub mod adapters;
pub use adapters::TcpBackend;

#[cfg(feature = "io_uring")]
pub use adapters::IoUringBackend;

// Future adapters
// #[cfg(feature = "quic")]
// pub use adapters::QuicBackend;

// #[cfg(feature = "dpdk")]
// pub use adapters::DpdkBackend;

// Features layer (optional capabilities)
pub mod features;
pub use features::{
    ParallelClient, ParallelConfig, ParallelServer, ParallelStats, ResumeManager,
    TlsClientConfig, TlsServerConfig, TransferState,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_display() {
        assert_eq!(TransportType::Tcp.to_string(), "TCP");
        assert_eq!(TransportType::Quic.to_string(), "QUIC");
        assert_eq!(TransportType::IoUring.to_string(), "io_uring");
        assert_eq!(TransportType::Dpdk.to_string(), "DPDK");
    }

    #[test]
    fn test_session_state_transitions() {
        let state = SessionState::Initializing;
        assert!(state.can_transition_to(SessionState::Active));
        assert!(!state.can_transition_to(SessionState::Completed));
    }

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        assert!(!platform.os.is_empty());
        assert!(platform.cpu_cores > 0);
    }

    #[test]
    fn test_backend_capabilities() {
        let caps = BackendCapabilities::for_transport(TransportType::IoUring);
        assert_eq!(caps.max_throughput_gbps(), 20.0); // 20 GB/s with enhanced async I/O
        assert!(caps.supports_zero_copy);
        assert!(caps.uses_kernel_bypass);
        assert!(!caps.requires_special_hardware);
    }

    #[tokio::test]
    async fn test_transport_manager_creation() {
        let config = TransportManagerConfig::default();
        let manager = TransportManager::new(config);
        
        let platform = manager.get_platform().await;
        assert!(!platform.os.is_empty());
    }

    #[test]
    fn test_session_id_uniqueness() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_transport_stats() {
        let mut stats = TransportStats::new();
        stats.record_send(1000);
        stats.record_receive(2000);
        
        assert_eq!(stats.bytes_sent, 1000);
        assert_eq!(stats.bytes_received, 2000);
        assert_eq!(stats.bytes_transferred, 3000);
    }
}

// Made with Bob
