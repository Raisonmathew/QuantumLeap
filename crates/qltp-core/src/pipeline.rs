//! Transfer pipeline implementation
//!
//! Orchestrates the complete file transfer process through all layers.

use crate::{
    chunking::{self, ContentDefinedChunker},
    error::Result,
    types::{ChunkInfo, TransferProgress, TransferResult, TransferStrategy},
    Error,
};
use qltp_compression::{self as compression, Algorithm, CompressionLevel};
use qltp_storage::{ContentStore, DeduplicationEngine};
use qltp_transport::application::TransportManager;
use qltp_transport::domain::{SessionConfig, SessionId};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

// Re-export for convenience
pub use qltp_transport::domain::TransportStats;

/// Progress callback function type
pub type ProgressCallback = Arc<dyn Fn(TransferProgress) + Send + Sync>;

/// Transfer mode - local or remote
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    /// Local transfer (storage only)
    Local,
    /// Remote transfer (via transport)
    Remote,
}

/// Transfer pipeline that orchestrates all layers
pub struct TransferPipeline {
    storage: Arc<Mutex<ContentStore>>,
    dedup_engine: Arc<Mutex<DeduplicationEngine>>,
    transport: Arc<TransportManager>,
    progress_callback: Option<ProgressCallback>,
    /// Active session for remote transfers
    active_session: Arc<Mutex<Option<SessionId>>>,
}

impl TransferPipeline {
    /// Create a new transfer pipeline with transport manager
    pub async fn with_transport(
        storage_dir: impl AsRef<Path>,
        transport: Arc<TransportManager>,
    ) -> Result<Self> {
        let storage = ContentStore::new(&storage_dir).await?;
        let dedup_engine = DeduplicationEngine::new(&storage_dir).await?;

        Ok(Self {
            storage: Arc::new(Mutex::new(storage)),
            dedup_engine: Arc::new(Mutex::new(dedup_engine)),
            transport,
            progress_callback: None,
            active_session: Arc::new(Mutex::new(None)),
        })
    }

    /// Set progress callback
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Create a new session for remote transfer
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        let session_id = self.transport.create_session(config).await
            .map_err(|e| Error::Other(format!("Failed to create session: {}", e)))?;
        let mut active = self.active_session.lock().await;
        *active = Some(session_id);
        info!("Created transport session: {}", session_id);
        Ok(session_id)
    }

    /// Start the active session
    pub async fn start_session(&self) -> Result<()> {
        let active = self.active_session.lock().await;
        if let Some(session_id) = *active {
            self.transport.start_session(session_id).await
                .map_err(|e| Error::Other(format!("Failed to start session: {}", e)))?;
            info!("Started transport session: {}", session_id);
            Ok(())
        } else {
            Err(Error::Other("No active session".to_string()))
        }
    }

    /// Stop the active session
    pub async fn stop_session(&self) -> Result<()> {
        let mut active = self.active_session.lock().await;
        if let Some(session_id) = active.take() {
            self.transport.stop_session(session_id).await
                .map_err(|e| Error::Other(format!("Failed to stop session: {}", e)))?;
            info!("Stopped transport session: {}", session_id);
            Ok(())
        } else {
            Ok(()) // No active session, nothing to stop
        }
    }

    /// Get the active session ID
    pub async fn active_session(&self) -> Option<SessionId> {
        *self.active_session.lock().await
    }

    /// Execute the complete transfer pipeline
    #[instrument(skip(self, source))]
    pub async fn execute(
        &self,
        source: impl AsRef<Path>,
        strategy: TransferStrategy,
    ) -> Result<TransferResult> {
        self.execute_with_mode(source, strategy, TransferMode::Local).await
    }

    /// Execute transfer with specified mode (local or remote)
    #[instrument(skip(self, source))]
    pub async fn execute_with_mode(
        &self,
        source: impl AsRef<Path>,
        strategy: TransferStrategy,
        mode: TransferMode,
    ) -> Result<TransferResult> {
        let source = source.as_ref();
        let start = Instant::now();

        info!("Starting transfer pipeline for {} (mode: {:?})", source.display(), mode);

        // Step 1: Chunk the file
        let chunks = self.chunk_file(source, &strategy).await?;
        debug!("Created {} chunks", chunks.len());

        // Step 2: Deduplicate chunks
        let dedup_result = if strategy.use_dedup {
            let mut engine = self.dedup_engine.lock().await;
            // Convert chunks to hex IDs for deduplication
            let chunk_ids: Vec<String> = chunks.iter().map(|c| c.id.to_hex()).collect();
            Some(engine.deduplicate(&chunk_ids).await?)
        } else {
            None
        };

        let chunks_to_process: Vec<&ChunkInfo> = if let Some(ref result) = dedup_result {
            debug!(
                "Deduplication: {} unique, {} duplicates (ratio: {:.2}x)",
                result.unique_indices.len(),
                result.duplicate_indices.len(),
                result.dedup_ratio
            );
            result.unique_indices.iter().map(|&idx| &chunks[idx]).collect()
        } else {
            chunks.iter().collect()
        };

        // Step 3: Compress, store, and optionally transfer chunks
        let mut total_original_size = 0u64;
        let mut total_compressed_size = 0u64;
        let mut processed_chunks = 0;
        let mut transfer_errors = 0;

        // Get active session for remote transfers
        let session_id = if mode == TransferMode::Remote {
            self.active_session.lock().await.ok_or_else(|| {
                Error::Other("No active session for remote transfer".to_string())
            })?
        } else {
            SessionId::default() // Dummy for local transfers
        };

        for &chunk in &chunks_to_process {
            // Read chunk data
            let chunk_data = match chunking::read_chunk(source, chunk).await {
                Ok(data) => data,
                Err(e) => {
                    warn!("Failed to read chunk {}: {}", chunk.id.to_hex(), e);
                    transfer_errors += 1;
                    continue;
                }
            };
            total_original_size += chunk_data.len() as u64;

            // Compress if enabled (with transport-aware decision)
            let should_compress = strategy.use_compression
                && compression::should_compress(&chunk_data, 1024, 1.5)
                && self.should_compress_for_transport(mode).await;

            let (final_data, compressed) = if should_compress {
                match compression::compress(
                    &chunk_data,
                    Algorithm::Lz4,
                    CompressionLevel::DEFAULT,
                ) {
                    Ok(compressed_data) => {
                        total_compressed_size += compressed_data.len() as u64;
                        (compressed_data, true)
                    }
                    Err(e) => {
                        warn!("Compression failed for chunk {}: {}, using uncompressed",
                              chunk.id.to_hex(), e);
                        total_compressed_size += chunk_data.len() as u64;
                        (chunk_data, false)
                    }
                }
            } else {
                total_compressed_size += chunk_data.len() as u64;
                (chunk_data, false)
            };

            // Store chunk locally
            {
                let mut storage = self.storage.lock().await;
                if let Err(e) = storage.store(&chunk.id.to_hex(), &final_data).await {
                    warn!("Failed to store chunk {}: {}", chunk.id.to_hex(), e);
                    transfer_errors += 1;
                    continue;
                }
            }

            // Transfer chunk if remote mode
            if mode == TransferMode::Remote {
                match self.transfer_chunk(session_id, &final_data).await {
                    Ok(bytes_sent) => {
                        debug!("Transferred chunk {} ({} bytes)", chunk.id.to_hex(), bytes_sent);
                    }
                    Err(e) => {
                        warn!("Failed to transfer chunk {}: {}", chunk.id.to_hex(), e);
                        transfer_errors += 1;
                        // Continue processing other chunks
                    }
                }
            }

            processed_chunks += 1;

            // Report progress with transport metrics
            if let Some(ref callback) = self.progress_callback {
                let elapsed = start.elapsed().as_secs_f64();
                let current_speed = if elapsed > 0.0 {
                    total_original_size as f64 / elapsed
                } else {
                    0.0
                };

                let total_bytes: u64 = chunks.iter().map(|c| c.size as u64).sum();
                let eta = if current_speed > 0.0 {
                    let remaining = total_bytes.saturating_sub(total_original_size);
                    Some(std::time::Duration::from_secs_f64(remaining as f64 / current_speed))
                } else {
                    None
                };

                let progress = TransferProgress {
                    bytes_transferred: total_original_size,
                    total_bytes,
                    current_speed,
                    eta,
                };
                callback(progress);
            }

            debug!(
                "Processed chunk {}/{} (compressed: {}, transferred: {})",
                processed_chunks,
                chunks_to_process.len(),
                compressed,
                mode == TransferMode::Remote
            );
        }

        // Log any errors
        if transfer_errors > 0 {
            warn!("Transfer completed with {} errors", transfer_errors);
        }

        let duration = start.elapsed();
        let compression_ratio = if total_compressed_size > 0 {
            total_original_size as f64 / total_compressed_size as f64
        } else {
            1.0
        };

        let result = TransferResult {
            bytes_transferred: total_original_size,
            duration,
            compression_ratio,
            strategy_used: strategy,
        };

        info!(
            "Transfer complete: {} bytes in {:?} ({:.2} MB/s, {:.2}x compression)",
            result.bytes_transferred,
            result.duration,
            result.speed_mbps(),
            result.compression_ratio
        );

        Ok(result)
    }

    /// Transfer a chunk over the transport layer
    async fn transfer_chunk(&self, session_id: SessionId, data: &[u8]) -> Result<usize> {
        // Send chunk data over transport with retry logic
        const MAX_RETRIES: usize = 3;
        let mut last_error = None;

        for attempt in 1..=MAX_RETRIES {
            match self.transport.send(session_id, data).await {
                Ok(bytes_sent) => {
                    if bytes_sent != data.len() {
                        warn!(
                            "Partial send: {} of {} bytes (attempt {}/{})",
                            bytes_sent, data.len(), attempt, MAX_RETRIES
                        );
                        // For now, treat partial sends as errors
                        last_error = Some(Error::Other(format!(
                            "Partial send: {} of {} bytes",
                            bytes_sent, data.len()
                        )));
                        continue;
                    }
                    return Ok(bytes_sent);
                }
                Err(e) => {
                    warn!("Transfer attempt {}/{} failed: {}", attempt, MAX_RETRIES, e);
                    last_error = Some(Error::Other(format!("Transport error: {}", e)));
                    
                    if attempt < MAX_RETRIES {
                        // Exponential backoff
                        let delay = std::time::Duration::from_millis(100 * (1 << (attempt - 1)));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| Error::Other("Transfer failed".to_string())))
    }

    /// Determine if compression should be used based on transport characteristics
    async fn should_compress_for_transport(&self, mode: TransferMode) -> bool {
        if mode == TransferMode::Local {
            return true; // Always compress for local storage
        }

        // For remote transfers, check transport capabilities
        match self.transport.get_capabilities().await {
            Ok(caps) => {
                // High-speed transports (>500 MB/s) may not benefit from compression
                // due to CPU overhead vs network speed tradeoff
                let throughput_mbps = caps.max_throughput_gbps() * 1000.0;
                
                if throughput_mbps > 500.0 {
                    debug!("High-speed transport ({:.0} MB/s), compression may not be beneficial", throughput_mbps);
                    // Still compress, but this could be made configurable
                    true
                } else {
                    true
                }
            }
            Err(_) => true, // Default to compression if we can't determine capabilities
        }
    }

    /// Chunk a file based on strategy
    async fn chunk_file(
        &self,
        path: &Path,
        strategy: &TransferStrategy,
    ) -> Result<Vec<ChunkInfo>> {
        if strategy.chunk_size != 4096 {
            // Use content-defined chunking for non-standard chunk sizes
            let chunker = ContentDefinedChunker::new(strategy.chunk_size);
            chunker.chunk_file(path).await
        } else {
            // Use fixed-size chunking
            chunking::chunk_file(path, strategy.chunk_size).await
        }
    }

    /// Get transport manager reference
    pub fn transport(&self) -> &Arc<TransportManager> {
        &self.transport
    }

    /// Get transport statistics for active session
    pub async fn get_transport_stats(&self) -> Option<qltp_transport::domain::TransportStats> {
        let session_id = self.active_session.lock().await;
        if let Some(sid) = *session_id {
            self.transport.get_session_stats(sid).await.ok()
        } else {
            None
        }
    }

    /// Retrieve a file from storage
    #[instrument(skip(self, output_path))]
    pub async fn retrieve_file(
        &self,
        chunks: &[ChunkInfo],
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let output_path = output_path.as_ref();
        let mut file = tokio::fs::File::create(output_path).await?;

        let storage = self.storage.lock().await;

        for chunk in chunks {
            let chunk_data = storage.retrieve(&chunk.id.to_hex()).await?;

            // Decompress if needed
            let final_data = if chunk.compressed {
                compression::decompress(&chunk_data, Algorithm::Lz4)?
            } else {
                chunk_data
            };

            // Write to file
            use tokio::io::AsyncWriteExt;
            file.write_all(&final_data).await?;
        }

        file.sync_all().await?;
        info!("Retrieved file to {}", output_path.display());

        Ok(())
    }

    /// Get storage statistics
    pub async fn storage_stats(&self) -> StorageStats {
        let storage = self.storage.lock().await;
        StorageStats {
            chunk_count: storage.chunk_count(),
            total_size: storage.total_size(),
        }
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub chunk_count: usize,
    pub total_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};
    use qltp_transport::application::TransportManagerConfig;

    async fn create_test_pipeline(temp_dir: &Path) -> TransferPipeline {
        let transport = Arc::new(TransportManager::new(TransportManagerConfig::default()));
        TransferPipeline::with_transport(temp_dir, transport).await.unwrap()
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        let temp_dir = TempDir::new().unwrap();
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"Hello, QLTP Pipeline! ".repeat(100);
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let pipeline = create_test_pipeline(temp_dir.path()).await;

        let strategy = TransferStrategy {
            use_compression: true,
            use_dedup: true,
            use_delta: false,
            use_neural: false,
            use_prefetch: false,
            chunk_size: 4096,
        };

        let result = pipeline.execute(temp_file.path(), strategy).await.unwrap();

        assert_eq!(result.bytes_transferred, test_data.len() as u64);
        assert!(result.compression_ratio > 1.0); // Should compress repetitive data
        assert!(result.speed_mbps() > 0.0);
    }

    #[tokio::test]
    async fn test_pipeline_with_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let pipeline = create_test_pipeline(temp_dir.path()).await;

        // Create two files with identical content
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();
        let test_data = b"Identical content for deduplication test. ".repeat(50);
        
        file1.write_all(&test_data).unwrap();
        file1.flush().unwrap();
        file2.write_all(&test_data).unwrap();
        file2.flush().unwrap();

        let strategy = TransferStrategy {
            use_compression: false,
            use_dedup: true,
            use_delta: false,
            use_neural: false,
            use_prefetch: false,
            chunk_size: 4096,
        };

        // Transfer first file
        let result1 = pipeline.execute(file1.path(), strategy).await.unwrap();
        let stats1 = pipeline.storage_stats().await;

        // Transfer second file (should deduplicate)
        let result2 = pipeline.execute(file2.path(), strategy).await.unwrap();
        let stats2 = pipeline.storage_stats().await;

        // Second transfer should not increase storage significantly
        assert_eq!(result1.bytes_transferred, result2.bytes_transferred);
        assert_eq!(stats1.chunk_count, stats2.chunk_count); // Same chunks
    }

    #[tokio::test]
    async fn test_progress_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let temp_dir = TempDir::new().unwrap();
        let mut pipeline = create_test_pipeline(temp_dir.path()).await;

        let progress_count = Arc::new(AtomicUsize::new(0));
        let progress_count_clone = progress_count.clone();

        pipeline.set_progress_callback(Arc::new(move |_progress| {
            progress_count_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = vec![0u8; 50000]; // 50KB
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let strategy = TransferStrategy::default();
        pipeline.execute(temp_file.path(), strategy).await.unwrap();

        // Should have called progress callback multiple times
        assert!(progress_count.load(Ordering::SeqCst) > 0);
    }

    #[tokio::test]
    async fn test_session_management() {
        let temp_dir = TempDir::new().unwrap();
        let pipeline = create_test_pipeline(temp_dir.path()).await;

        // Create session - will fail without backend, which is expected
        let result = pipeline.create_session(SessionConfig::default()).await;
        assert!(result.is_err(), "Session creation should fail without backend");
        
        // Verify no active session
        assert_eq!(pipeline.active_session().await, None);
        
        // Stop session should succeed even with no active session
        pipeline.stop_session().await.unwrap();
    }

    #[tokio::test]
    async fn test_transfer_modes() {
        let temp_dir = TempDir::new().unwrap();
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"Test data for mode testing".repeat(10);
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let pipeline = create_test_pipeline(temp_dir.path()).await;
        let strategy = TransferStrategy::default();

        // Test local mode
        let result = pipeline
            .execute_with_mode(temp_file.path(), strategy, TransferMode::Local)
            .await
            .unwrap();
        assert_eq!(result.bytes_transferred, test_data.len() as u64);
    }
}

// Made with Bob
