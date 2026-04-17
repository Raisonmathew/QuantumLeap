//! Optional features for enhanced transfer capabilities
//!
//! This module provides additional features that can be enabled:
//! - Resume: Save and restore transfer state for interrupted transfers
//! - Parallel: Concurrent chunk transmission over multiple streams
//! - TLS: Encryption support for secure transfers
//! - Adaptive Tuning: Dynamic optimization of buffers, compression, and streams
//! - Parallel Streams: QUIC multi-stream multiplexing for 6-8x performance
//! - FEC: Forward Error Correction for near-zero packet loss

pub mod resume;
pub mod parallel;
pub mod tls;
pub mod adaptive_tuning;
pub mod parallel_streams;
pub mod fec;

pub use resume::{ResumeManager, TransferState};
pub use parallel::{ParallelClient, ParallelConfig, ParallelServer, ParallelStats};
pub use tls::{TlsClientConfig, TlsServerConfig};
pub use adaptive_tuning::{
    AdaptiveTuning, AdaptiveTuningConfig, CompressionStrategy,
    ParallelStreamConfig, PerformanceMetrics, TransferConfig
};
pub use parallel_streams::{
    ParallelStreamManager, StreamChunk, StreamId, StreamState, StreamStats,
};
pub use fec::{
    FecCodec, FecConfig, FecBlock, FecManager, FecStats,
};

// Made with Bob