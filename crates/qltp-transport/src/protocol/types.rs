//! Common types for file transfer

use super::messages::CompressionType;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Transfer configuration
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Chunk size in bytes
    pub chunk_size: u32,
    /// Send window size (max unacknowledged chunks)
    pub send_window: usize,
    /// Receive window size (max buffered chunks)
    pub receive_window: usize,
    /// Enable compression
    pub compression: bool,
    /// Compression type
    pub compression_type: CompressionType,
    /// Enable deduplication
    pub deduplication: bool,
    /// Bandwidth limit (bytes per second, 0 = unlimited)
    pub bandwidth_limit: u64,
    /// Retry attempts for failed chunks
    pub max_retries: u32,
    /// Timeout for chunk acknowledgment
    pub ack_timeout: Duration,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size: 4096,
            send_window: 256,
            receive_window: 512,
            compression: true,
            compression_type: CompressionType::Lz4,
            deduplication: true,
            bandwidth_limit: 0,
            max_retries: 5,
            ack_timeout: Duration::from_secs(5),
        }
    }
}

/// Transfer progress information
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transfer_id: Uuid,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub chunks_completed: u32,
    pub total_chunks: u32,
    pub elapsed: Duration,
    pub speed_bps: f64,
}

impl TransferProgress {
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0
        }
    }

    pub fn eta(&self) -> Option<Duration> {
        if self.speed_bps <= 0.0 || self.bytes_transferred >= self.total_bytes {
            return None;
        }

        let remaining_bytes = self.total_bytes - self.bytes_transferred;
        let seconds = remaining_bytes as f64 / self.speed_bps;
        Some(Duration::from_secs_f64(seconds))
    }
}

/// Transfer statistics
#[derive(Debug, Clone)]
pub struct TransferStats {
    pub total_bytes: u64,
    pub compressed_bytes: u64,
    pub chunks_sent: u32,
    pub chunks_retried: u32,
    pub duration: Duration,
    pub average_speed_bps: f64,
    pub compression_ratio: f32,
}

/// Progress callback type
pub type ProgressCallback = Arc<dyn Fn(TransferProgress) + Send + Sync>;

// Made with Bob