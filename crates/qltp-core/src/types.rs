//! Core types for QLTP

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Unique identifier for a chunk (SHA-256 hash)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkId(pub [u8; 32]);

impl ChunkId {
    /// Create a new ChunkId from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the bytes of the ChunkId
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Self(array))
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Information about a chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    /// Unique identifier (hash)
    pub id: ChunkId,
    /// Size in bytes
    pub size: usize,
    /// Offset in the original file
    pub offset: u64,
    /// Whether this chunk is compressed
    pub compressed: bool,
    /// Compression ratio (if compressed)
    pub compression_ratio: f64,
}

/// Engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Chunk size in bytes (default: 4KB)
    pub chunk_size: usize,
    /// Minimum file size for compression (default: 1KB)
    pub min_compression_size: u64,
    /// Minimum file size for deduplication (default: 1MB)
    pub min_dedup_size: u64,
    /// Maximum concurrent transfers
    pub max_concurrent_transfers: usize,
    /// Network timeout
    pub network_timeout: Duration,
    /// Enable compression by default
    pub enable_compression: bool,
    /// Enable deduplication by default
    pub enable_dedup: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            chunk_size: 4096,                           // 4KB
            min_compression_size: 1024,                 // 1KB
            min_dedup_size: 1024 * 1024,                // 1MB
            max_concurrent_transfers: 8,
            network_timeout: Duration::from_secs(30),
            enable_compression: true,
            enable_dedup: true,
        }
    }
}

/// Options for a file transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOptions {
    /// Enable compression
    pub compression: bool,
    /// Enable deduplication
    pub deduplication: bool,
    /// Enable delta encoding
    pub delta_encoding: bool,
    /// Enable encryption
    pub encryption: bool,
    /// Priority (0-10, higher is more important)
    pub priority: u8,
    /// Resume from previous transfer
    pub resume: bool,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            compression: true,
            deduplication: true,
            delta_encoding: false,
            encryption: false,
            priority: 5,
            resume: false,
        }
    }
}

/// Strategy for transferring a file
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TransferStrategy {
    /// Use compression
    pub use_compression: bool,
    /// Use deduplication
    pub use_dedup: bool,
    /// Use delta encoding
    pub use_delta: bool,
    /// Use neural compression
    pub use_neural: bool,
    /// Use predictive prefetching
    pub use_prefetch: bool,
    /// Chunk size to use
    pub chunk_size: usize,
}

impl Default for TransferStrategy {
    fn default() -> Self {
        Self {
            use_compression: true,
            use_dedup: true,
            use_delta: false,
            use_neural: false,
            use_prefetch: false,
            chunk_size: 4096,
        }
    }
}

/// Result of a file transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    /// Total bytes transferred (original size)
    pub bytes_transferred: u64,
    /// Time taken
    pub duration: Duration,
    /// Compression ratio achieved
    pub compression_ratio: f64,
    /// Strategy used
    pub strategy_used: TransferStrategy,
}

impl TransferResult {
    /// Calculate effective speed in bytes per second.
    ///
    /// Returns `0.0` when `duration` is zero (or sub-nanosecond) instead
    /// of producing `+inf` / `NaN`. A zero-duration transfer is reported
    /// as zero throughput rather than infinity so downstream metrics,
    /// dashboards, and JSON serializers don't get poisoned values.
    pub fn speed_bps(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs <= 0.0 || !secs.is_finite() {
            0.0
        } else {
            self.bytes_transferred as f64 / secs
        }
    }

    /// Calculate effective speed in megabytes per second
    pub fn speed_mbps(&self) -> f64 {
        self.speed_bps() / (1024.0 * 1024.0)
    }

    /// Calculate effective speed in gigabytes per second
    pub fn speed_gbps(&self) -> f64 {
        self.speed_bps() / (1024.0 * 1024.0 * 1024.0)
    }

    /// Calculate effective speed considering compression
    pub fn effective_speed_gbps(&self) -> f64 {
        self.speed_gbps() * self.compression_ratio
    }
}

/// Progress information for a transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// Current speed in bytes per second
    pub current_speed: f64,
    /// Estimated time remaining
    pub eta: Option<Duration>,
}

impl TransferProgress {
    /// Calculate progress percentage (0-100)
    pub fn percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.bytes_transferred as f64 / self.total_bytes as f64) * 100.0
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        self.bytes_transferred >= self.total_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_id_hex() {
        let id = ChunkId::new([1u8; 32]);
        let hex = id.to_hex();
        assert_eq!(hex.len(), 64);
        
        let parsed = ChunkId::from_hex(&hex).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_transfer_result_speed() {
        let result = TransferResult {
            bytes_transferred: 1024 * 1024 * 1024, // 1GB
            duration: Duration::from_secs(1),
            compression_ratio: 2.0,
            strategy_used: TransferStrategy::default(),
        };

        assert_eq!(result.speed_gbps(), 1.0);
        assert_eq!(result.effective_speed_gbps(), 2.0);
    }

    #[test]
    fn test_transfer_progress() {
        let progress = TransferProgress {
            bytes_transferred: 50,
            total_bytes: 100,
            current_speed: 10.0,
            eta: Some(Duration::from_secs(5)),
        };

        assert_eq!(progress.percent(), 50.0);
        assert!(!progress.is_complete());

        let complete = TransferProgress {
            bytes_transferred: 100,
            total_bytes: 100,
            current_speed: 0.0,
            eta: None,
        };

        assert_eq!(complete.percent(), 100.0);
        assert!(complete.is_complete());
    }
}

// Made with Bob
