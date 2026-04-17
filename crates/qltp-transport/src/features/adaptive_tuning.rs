//! Adaptive Buffer & Window Tuning
//!
//! Phase 6.4: Dynamic optimization of buffers, windows, and compression
//!
//! Features:
//! - Adaptive compression based on data characteristics
//! - Dynamic buffer sizing based on throughput
//! - Parallel stream management
//! - Automatic window scaling
//! - Real-time performance monitoring

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use crate::error::Result;

/// Compression strategy based on data characteristics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionStrategy {
    /// No compression (for pre-compressed data)
    None,
    /// Fast compression (LZ4-like, low CPU)
    Fast,
    /// Balanced compression (Zstandard-like, medium CPU)
    Balanced,
    /// Maximum compression (GZIP-like, high CPU)
    Maximum,
}

/// Parallel stream configuration
#[derive(Debug, Clone)]
pub struct ParallelStreamConfig {
    /// Number of parallel streams
    pub stream_count: usize,
    /// Chunk size per stream
    pub chunk_size: usize,
    /// Enable dynamic stream adjustment
    pub dynamic_adjustment: bool,
}

impl Default for ParallelStreamConfig {
    fn default() -> Self {
        Self {
            stream_count: 4,
            chunk_size: 1024 * 1024, // 1 MB
            dynamic_adjustment: true,
        }
    }
}

/// Adaptive tuning configuration
#[derive(Debug, Clone)]
pub struct AdaptiveTuningConfig {
    /// Enable adaptive compression
    pub enable_compression: bool,
    /// Enable parallel streams
    pub enable_parallel_streams: bool,
    /// Enable dynamic buffer sizing
    pub enable_dynamic_buffers: bool,
    /// Minimum buffer size
    pub min_buffer_size: usize,
    /// Maximum buffer size
    pub max_buffer_size: usize,
    /// Parallel stream configuration
    pub parallel_config: ParallelStreamConfig,
}

impl Default for AdaptiveTuningConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            enable_parallel_streams: true,
            enable_dynamic_buffers: true,
            min_buffer_size: 64 * 1024,      // 64 KB
            max_buffer_size: 16 * 1024 * 1024, // 16 MB
            parallel_config: ParallelStreamConfig::default(),
        }
    }
}

/// Performance metrics for adaptive tuning
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// Current throughput in bytes per second
    pub throughput_bps: u64,
    /// Average latency in microseconds
    pub latency_us: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f64,
    /// Compression ratio (original / compressed)
    pub compression_ratio: f64,
    /// Number of active streams
    pub active_streams: usize,
    /// Current buffer size
    pub buffer_size: usize,
}

/// Adaptive tuning engine
pub struct AdaptiveTuning {
    config: AdaptiveTuningConfig,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    last_update: Arc<RwLock<Instant>>,
}

impl AdaptiveTuning {
    /// Create a new adaptive tuning engine
    pub fn new(config: AdaptiveTuningConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            last_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(AdaptiveTuningConfig::default())
    }

    /// Select optimal compression strategy based on data
    pub fn select_compression(&self, data: &[u8]) -> CompressionStrategy {
        if !self.config.enable_compression {
            return CompressionStrategy::None;
        }

        // Sample first 1KB to detect data type
        let sample_size = data.len().min(1024);
        let sample = &data[..sample_size];

        // Calculate entropy (simplified)
        let entropy = Self::calculate_entropy(sample);

        // High entropy (>7.5) suggests already compressed data
        if entropy > 7.5 {
            CompressionStrategy::None
        } else if entropy > 6.0 {
            // Medium entropy - use fast compression
            CompressionStrategy::Fast
        } else if entropy > 4.0 {
            // Low entropy - use balanced compression
            CompressionStrategy::Balanced
        } else {
            // Very low entropy - use maximum compression
            CompressionStrategy::Maximum
        }
    }

    /// Calculate Shannon entropy of data
    fn calculate_entropy(data: &[u8]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mut counts = [0u32; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    /// Calculate optimal buffer size based on throughput
    pub async fn calculate_buffer_size(&self, throughput_bps: u64, latency_ms: u64) -> usize {
        if !self.config.enable_dynamic_buffers {
            return self.config.min_buffer_size;
        }

        // BDP (Bandwidth-Delay Product) formula
        // Buffer Size = Throughput (bytes/sec) × Latency (sec)
        let bdp = (throughput_bps as f64 * latency_ms as f64 / 1000.0) as usize;

        // Clamp to configured limits
        bdp.max(self.config.min_buffer_size)
            .min(self.config.max_buffer_size)
    }

    /// Calculate optimal number of parallel streams
    pub async fn calculate_stream_count(&self, file_size: u64, throughput_bps: u64) -> usize {
        if !self.config.enable_parallel_streams {
            return 1;
        }

        let config = &self.config.parallel_config;

        if !config.dynamic_adjustment {
            return config.stream_count;
        }

        // For small files, use fewer streams
        if file_size < 10 * 1024 * 1024 {
            return 1;
        }

        // Calculate based on throughput
        // More streams for higher throughput
        let base_streams = if throughput_bps > 1_000_000_000 {
            // >1 GB/s: 8 streams
            8
        } else if throughput_bps > 100_000_000 {
            // >100 MB/s: 4 streams
            4
        } else {
            // <100 MB/s: 2 streams
            2
        };

        // Adjust based on file size
        let size_factor = (file_size / (100 * 1024 * 1024)) as usize; // Per 100 MB
        let streams = base_streams + size_factor.min(8);

        // Cap at configured maximum
        streams.min(config.stream_count * 2)
    }

    /// Update performance metrics
    pub async fn update_metrics(&self, metrics: PerformanceMetrics) -> Result<()> {
        let mut current_metrics = self.metrics.write().await;
        *current_metrics = metrics;

        let mut last_update = self.last_update.write().await;
        *last_update = Instant::now();

        Ok(())
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().await.clone()
    }

    /// Estimate compression ratio for data
    pub fn estimate_compression_ratio(&self, data: &[u8], strategy: CompressionStrategy) -> f64 {
        match strategy {
            CompressionStrategy::None => 1.0,
            CompressionStrategy::Fast => {
                // LZ4-like: 1.5-2.5x for text, 1.0-1.2x for binary
                let entropy = Self::calculate_entropy(&data[..data.len().min(1024)]);
                if entropy < 5.0 {
                    2.0 // Good compression for text
                } else {
                    1.2 // Minimal compression for binary
                }
            }
            CompressionStrategy::Balanced => {
                // Zstandard-like: 2-4x for text, 1.2-1.5x for binary
                let entropy = Self::calculate_entropy(&data[..data.len().min(1024)]);
                if entropy < 5.0 {
                    3.0
                } else {
                    1.4
                }
            }
            CompressionStrategy::Maximum => {
                // GZIP-like: 3-6x for text, 1.3-2x for binary
                let entropy = Self::calculate_entropy(&data[..data.len().min(1024)]);
                if entropy < 5.0 {
                    4.5
                } else {
                    1.6
                }
            }
        }
    }

    /// Calculate optimal chunk size for parallel streams
    pub fn calculate_chunk_size(&self, file_size: u64, stream_count: usize) -> usize {
        if stream_count <= 1 {
            return file_size as usize;
        }

        // Divide file into equal chunks
        let base_chunk = (file_size / stream_count as u64) as usize;

        // Round up to nearest MB for better alignment
        let chunk_mb = (base_chunk + 1024 * 1024 - 1) / (1024 * 1024);
        let aligned_chunk = chunk_mb * 1024 * 1024;

        // Ensure minimum chunk size of 1 MB
        aligned_chunk.max(1024 * 1024)
    }

    /// Recommend optimal configuration for a transfer
    pub async fn recommend_config(
        &self,
        file_size: u64,
        throughput_bps: u64,
        latency_ms: u64,
        data_sample: &[u8],
    ) -> TransferConfig {
        let compression = self.select_compression(data_sample);
        let buffer_size = self.calculate_buffer_size(throughput_bps, latency_ms).await;
        let stream_count = self.calculate_stream_count(file_size, throughput_bps).await;
        let chunk_size = self.calculate_chunk_size(file_size, stream_count);
        let compression_ratio = self.estimate_compression_ratio(data_sample, compression);

        TransferConfig {
            compression,
            buffer_size,
            stream_count,
            chunk_size,
            estimated_compression_ratio: compression_ratio,
        }
    }
}

/// Recommended transfer configuration
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Compression strategy
    pub compression: CompressionStrategy,
    /// Buffer size in bytes
    pub buffer_size: usize,
    /// Number of parallel streams
    pub stream_count: usize,
    /// Chunk size per stream
    pub chunk_size: usize,
    /// Estimated compression ratio
    pub estimated_compression_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_calculation() {
        // All zeros - minimum entropy
        let zeros = vec![0u8; 1024];
        let entropy = AdaptiveTuning::calculate_entropy(&zeros);
        assert!(entropy < 0.1);

        // Random data - high entropy
        let random: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        let entropy = AdaptiveTuning::calculate_entropy(&random);
        assert!(entropy > 5.0);

        // Text-like data - medium entropy
        let text = b"Hello World! This is a test message with some repetition.";
        let entropy = AdaptiveTuning::calculate_entropy(text);
        assert!(entropy > 3.0 && entropy < 6.0);
    }

    #[test]
    fn test_compression_selection() {
        let tuning = AdaptiveTuning::default();

        // High entropy (random) -> No compression
        let random: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        assert_eq!(tuning.select_compression(&random), CompressionStrategy::None);

        // Low entropy (text) -> Maximum compression
        let text = vec![b'A'; 1024];
        let strategy = tuning.select_compression(&text);
        assert!(strategy == CompressionStrategy::Maximum || strategy == CompressionStrategy::Balanced);
    }

    #[tokio::test]
    async fn test_buffer_size_calculation() {
        let tuning = AdaptiveTuning::default();

        // 1 Gbps, 50ms latency
        let buffer_size = tuning.calculate_buffer_size(125_000_000, 50).await;
        assert!(buffer_size >= 64 * 1024); // At least min size
        assert!(buffer_size <= 16 * 1024 * 1024); // At most max size
    }

    #[tokio::test]
    async fn test_stream_count_calculation() {
        let tuning = AdaptiveTuning::default();

        // Small file
        let streams = tuning.calculate_stream_count(1024 * 1024, 100_000_000).await;
        assert_eq!(streams, 1);

        // Large file, high throughput
        let streams = tuning.calculate_stream_count(1024 * 1024 * 1024, 1_000_000_000).await;
        assert!(streams > 1);
    }

    #[test]
    fn test_chunk_size_calculation() {
        let tuning = AdaptiveTuning::default();

        // 100 MB file, 4 streams
        let chunk_size = tuning.calculate_chunk_size(100 * 1024 * 1024, 4);
        assert!(chunk_size >= 1024 * 1024); // At least 1 MB
        assert!(chunk_size % (1024 * 1024) == 0); // Aligned to MB
    }

    #[test]
    fn test_compression_ratio_estimation() {
        let tuning = AdaptiveTuning::default();

        // Text data with fast compression
        let text = b"Hello World! ".repeat(100);
        let ratio = tuning.estimate_compression_ratio(&text, CompressionStrategy::Fast);
        assert!(ratio > 1.0);

        // No compression
        let ratio = tuning.estimate_compression_ratio(&text, CompressionStrategy::None);
        assert_eq!(ratio, 1.0);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let tuning = AdaptiveTuning::default();

        let metrics = PerformanceMetrics {
            throughput_bps: 1_000_000_000,
            latency_us: 1000,
            cpu_usage: 50.0,
            compression_ratio: 2.5,
            active_streams: 4,
            buffer_size: 1024 * 1024,
        };

        tuning.update_metrics(metrics.clone()).await.unwrap();

        let retrieved = tuning.get_metrics().await;
        assert_eq!(retrieved.throughput_bps, metrics.throughput_bps);
        assert_eq!(retrieved.active_streams, metrics.active_streams);
    }

    #[tokio::test]
    async fn test_recommend_config() {
        let tuning = AdaptiveTuning::default();

        let text_data = b"Hello World! ".repeat(100);
        let config = tuning.recommend_config(
            100 * 1024 * 1024,  // 100 MB file
            1_000_000_000,       // 1 Gbps
            50,                  // 50ms latency
            &text_data,
        ).await;

        assert!(config.buffer_size >= 64 * 1024);
        assert!(config.stream_count >= 1);
        assert!(config.chunk_size >= 1024 * 1024);
        assert!(config.estimated_compression_ratio >= 1.0);
    }

    #[test]
    fn test_config_defaults() {
        let config = AdaptiveTuningConfig::default();
        assert!(config.enable_compression);
        assert!(config.enable_parallel_streams);
        assert!(config.enable_dynamic_buffers);
        assert_eq!(config.min_buffer_size, 64 * 1024);
        assert_eq!(config.max_buffer_size, 16 * 1024 * 1024);
    }
}

// Made with Bob
