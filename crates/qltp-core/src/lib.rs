//! QLTP Core - High-performance file transfer engine
//!
//! This crate provides the core functionality for the QLTP (Quantum Leap Transfer Protocol)
//! file transfer system, implementing a 5-layer optimization cascade:
//!
//! 1. Context-aware pre-positioning
//! 2. Predictive delta encoding
//! 3. Content-addressable deduplication
//! 4. Neural compression (future)
//! 5. Speculative pre-fetching (future)
//!
//! # Example
//!
//! ```no_run
//! use qltp_core::{Engine, TransferOptions};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let engine = Engine::new().await?;
//!
//!     let options = TransferOptions::default();
//!     let result = engine.transfer_file("large_file.bin", "remote:/path/", options).await?;
//!
//!     println!("Transferred {} bytes in {:?}", result.bytes_transferred, result.duration);
//!     println!("Effective speed: {:.2} GB/s", result.effective_speed_gbps());
//!
//!     Ok(())
//! }
//! ```

pub mod adaptive;
pub mod chunking;
pub mod compression;
pub mod error;
pub mod hash;
pub mod pipeline;
pub mod prefetch;
pub mod types;

pub use adaptive::{AdaptiveCompressor, AdaptiveConfig, CompressionAlgorithm, ContentType};
pub use compression::{compress_lz4, compress_zstd, decompress_lz4, decompress_zstd};
pub use error::{Error, Result};
pub use pipeline::{ProgressCallback, StorageStats, TransferMode, TransferPipeline, TransportStats};
pub use prefetch::{AccessPattern, Prefetcher, PrefetchConfig, PrefetchStats, Prediction};
pub use types::{
    ChunkId, ChunkInfo, EngineConfig, TransferOptions, TransferProgress, TransferResult,
    TransferStrategy,
};

// Re-export transport types for convenience
pub use qltp_transport::application::{TransportManager, TransportManagerConfig, SelectionCriteria};
pub use qltp_transport::domain::{SessionConfig, SessionId, TransportType};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

/// Main QLTP engine for file transfers
pub struct Engine {
    config: EngineConfig,
    pipeline: Arc<TransferPipeline>,
    transport: Arc<TransportManager>,
    storage_dir: PathBuf,
}

impl Engine {
    /// Create a new QLTP engine with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(EngineConfig::default()).await
    }

    /// Create a new QLTP engine with custom configuration
    pub async fn with_config(config: EngineConfig) -> Result<Self> {
        Self::with_transport_config(config, TransportManagerConfig::default()).await
    }

    /// Create a new QLTP engine with custom transport configuration
    pub async fn with_transport_config(
        config: EngineConfig,
        transport_config: TransportManagerConfig,
    ) -> Result<Self> {
        info!("Initializing QLTP engine with config: {:?}", config);
        
        // Create storage directory
        let storage_dir = std::env::temp_dir().join("qltp-storage");
        tokio::fs::create_dir_all(&storage_dir).await?;
        
        // Create and initialize transport manager
        let transport = Arc::new(TransportManager::new(transport_config));
        
        // Create transfer pipeline with transport
        let pipeline = TransferPipeline::with_transport(&storage_dir, transport.clone()).await?;
        
        // Auto-select and initialize optimal backend
        info!("Auto-selecting optimal transport backend...");
        match transport.auto_initialize(None).await {
            Ok(selection) => {
                info!(
                    "Selected transport backend: {} - {}",
                    selection.transport_type, selection.reason
                );
            }
            Err(e) => {
                warn!("Failed to auto-initialize transport backend: {}", e);
                warn!("Transport operations will fail until backend is manually initialized");
            }
        }
        
        Ok(Self {
            config,
            pipeline: Arc::new(pipeline),
            transport,
            storage_dir,
        })
    }

    /// Transfer a file from source to destination
    #[instrument(skip(self, source))]
    pub async fn transfer_file(
        &self,
        source: impl AsRef<Path>,
        destination: &str,
        options: TransferOptions,
    ) -> Result<TransferResult> {
        let source = source.as_ref();
        let _start = Instant::now();

        info!(
            "Starting transfer: {} -> {}",
            source.display(),
            destination
        );

        // Analyze file to determine optimal strategy
        let metadata = self.analyze_file(source).await?;
        debug!("File metadata: {:?}", metadata);

        // Select transfer strategy based on file characteristics
        let strategy = self.select_strategy(&metadata, &options);
        debug!("Selected strategy: {:?}", strategy);

        // Execute the transfer pipeline
        let result = self.pipeline.execute(source, strategy).await?;

        info!(
            "Transfer complete: {} bytes in {:?} ({:.2} GB/s effective, {:.2}x compression)",
            result.bytes_transferred,
            result.duration,
            result.effective_speed_gbps(),
            result.compression_ratio
        );

        Ok(result)
    }

    /// Set progress callback for transfers
    pub fn set_progress_callback(&mut self, _callback: ProgressCallback) {
        // Note: This requires making pipeline mutable, which we'll handle differently
        // For now, this is a placeholder
        debug!("Progress callback set (not yet implemented in immutable pipeline)");
    }

    /// Get storage statistics
    pub async fn storage_stats(&self) -> StorageStats {
        self.pipeline.storage_stats().await
    }
    
    /// Get the storage directory path
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Get the transport manager
    pub fn transport(&self) -> &Arc<TransportManager> {
        &self.transport
    }

    /// Get the pipeline for advanced operations
    pub fn pipeline(&self) -> &Arc<TransferPipeline> {
        &self.pipeline
    }

    /// Get current transport backend type
    pub async fn current_backend(&self) -> Option<qltp_transport::domain::TransportType> {
        self.transport.current_backend_type().await
    }

    /// List available transport backends on this platform
    pub fn list_available_backends(&self) -> Vec<(qltp_transport::domain::TransportType, qltp_transport::domain::BackendCapabilities)> {
        self.transport.list_available_backends()
    }

    /// Get transport performance metrics
    pub async fn transport_metrics(&self) -> Option<qltp_transport::application::BackendMetrics> {
        self.transport.get_backend_metrics().await
    }

    /// Perform health check on current transport backend
    pub async fn transport_health(&self) -> Result<qltp_transport::application::HealthCheckResult> {
        self.transport.health_check_backend().await
            .map_err(|e| Error::Other(format!("Transport health check failed: {}", e)))
    }

    /// Create a new session for remote transfer
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        self.pipeline.create_session(config).await
    }

    /// Start the active session
    pub async fn start_session(&self) -> Result<()> {
        self.pipeline.start_session().await
    }

    /// Stop the active session
    pub async fn stop_session(&self) -> Result<()> {
        self.pipeline.stop_session().await
    }

    /// Get the active session ID
    pub async fn active_session(&self) -> Option<SessionId> {
        self.pipeline.active_session().await
    }

    /// Get transport statistics for active session
    pub async fn transport_stats(&self) -> Option<TransportStats> {
        self.pipeline.get_transport_stats().await
    }

    /// Transfer a file with specified mode (local or remote)
    pub async fn transfer_file_with_mode(
        &self,
        source: impl AsRef<Path>,
        _destination: &str,
        options: TransferOptions,
        mode: TransferMode,
    ) -> Result<TransferResult> {
        let source = source.as_ref();
        let metadata = self.analyze_file(source).await?;
        let strategy = self.select_strategy(&metadata, &options);
        
        self.pipeline.execute_with_mode(source, strategy, mode).await
    }

    /// Analyze a file to determine its characteristics
    async fn analyze_file(&self, path: &Path) -> Result<FileMetadata> {
        let metadata = tokio::fs::metadata(path).await?;

        Ok(FileMetadata {
            size: metadata.len(),
            file_type: detect_file_type(path),
        })
    }

    /// Select optimal transfer strategy based on file metadata
    fn select_strategy(&self, metadata: &FileMetadata, options: &TransferOptions) -> TransferStrategy {
        // Don't compress already compressed files or media
        let should_compress = match metadata.file_type {
            FileType::Compressed | FileType::Media => false,
            _ => true,
        };
        
        let use_compression = options.compression
            && metadata.size > self.config.min_compression_size
            && should_compress;
        let use_dedup = options.deduplication && metadata.size > self.config.min_dedup_size;

        TransferStrategy {
            use_compression,
            use_dedup,
            use_delta: options.delta_encoding,
            // Neural compression and prefetch are not implemented in this
            // build. Surfaced as `false` so downstream code can be honest
            // about what is actually being applied.
            use_neural: false,
            use_prefetch: false,
            chunk_size: self.config.chunk_size,
        }
    }
}

/// File metadata for strategy selection
#[derive(Debug, Clone)]
struct FileMetadata {
    size: u64,
    file_type: FileType,
}

/// Detected file type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileType {
    Text,
    Binary,
    Compressed,
    Media,
    Unknown,
}

/// Detect file type from path
fn detect_file_type(path: &Path) -> FileType {
    match path.extension().and_then(|s| s.to_str()) {
        Some("txt") | Some("log") | Some("json") | Some("xml") | Some("csv") => FileType::Text,
        Some("zip") | Some("gz") | Some("bz2") | Some("xz") | Some("7z") => FileType::Compressed,
        Some("mp4") | Some("mkv") | Some("avi") | Some("mp3") | Some("flac") => FileType::Media,
        Some("jpg") | Some("png") | Some("gif") | Some("bmp") => FileType::Media,
        Some("bin") | Some("exe") | Some("dll") | Some("so") => FileType::Binary,
        _ => FileType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = Engine::new().await;
        assert!(engine.is_ok());
    }

    #[test]
    fn test_file_type_detection() {
        assert_eq!(detect_file_type(Path::new("test.txt")), FileType::Text);
        assert_eq!(detect_file_type(Path::new("test.zip")), FileType::Compressed);
        assert_eq!(detect_file_type(Path::new("test.mp4")), FileType::Media);
        assert_eq!(detect_file_type(Path::new("test.bin")), FileType::Binary);
        assert_eq!(detect_file_type(Path::new("test.unknown")), FileType::Unknown);
    }

    #[tokio::test]
    async fn test_analyze_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test data").unwrap();
        temp_file.flush().unwrap();

        let engine = Engine::new().await.unwrap();
        let metadata = engine.analyze_file(temp_file.path()).await.unwrap();

        assert_eq!(metadata.size, 9);
    }

    #[tokio::test]
    async fn test_transfer_with_compression() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"Repetitive data for compression test. ".repeat(100);
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let engine = Engine::new().await.unwrap();
        let options = TransferOptions {
            compression: true,
            deduplication: true,
            ..Default::default()
        };

        let result = engine
            .transfer_file(temp_file.path(), "remote:/test", options)
            .await
            .unwrap();

        assert_eq!(result.bytes_transferred, test_data.len() as u64);
        assert!(result.compression_ratio > 1.0); // Should compress repetitive data
    }
}

// Made with Bob
