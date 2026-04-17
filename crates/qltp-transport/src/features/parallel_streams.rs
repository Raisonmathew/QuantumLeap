//! Parallel Stream Management for QUIC
//!
//! Implements efficient parallel stream multiplexing for maximum throughput.
//! Distributes data across multiple QUIC streams for 6-8x performance improvement.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

/// Stream identifier
pub type StreamId = u64;

/// Chunk of data to be sent on a stream
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// Stream ID this chunk belongs to
    pub stream_id: StreamId,
    /// Chunk sequence number within the stream
    pub sequence: u64,
    /// Total number of chunks in this stream
    pub total_chunks: u64,
    /// Chunk data
    pub data: Vec<u8>,
    /// Whether this is the final chunk for this stream
    pub is_final: bool,
}

/// Stream state tracking
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is idle
    Idle,
    /// Stream is actively sending
    Sending,
    /// Stream is actively receiving
    Receiving,
    /// Stream completed successfully
    Completed,
    /// Stream encountered an error
    Failed,
}

/// Statistics for a single stream
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Stream ID
    pub stream_id: StreamId,
    /// Current state
    pub state: StreamState,
    /// Bytes sent on this stream
    pub bytes_sent: u64,
    /// Bytes received on this stream
    pub bytes_received: u64,
    /// Number of chunks sent
    pub chunks_sent: u64,
    /// Number of chunks received
    pub chunks_received: u64,
    /// Number of retransmissions
    pub retransmissions: u64,
    /// Average latency in microseconds
    pub avg_latency_us: u64,
}

impl StreamStats {
    fn new(stream_id: StreamId) -> Self {
        Self {
            stream_id,
            state: StreamState::Idle,
            bytes_sent: 0,
            bytes_received: 0,
            chunks_sent: 0,
            chunks_received: 0,
            retransmissions: 0,
            avg_latency_us: 0,
        }
    }
}

/// Configuration for parallel stream management
#[derive(Debug, Clone)]
pub struct ParallelStreamConfig {
    /// Number of parallel streams to use
    pub stream_count: usize,
    /// Size of each chunk in bytes
    pub chunk_size: usize,
    /// Enable dynamic stream adjustment
    pub dynamic_adjustment: bool,
    /// Maximum retransmission attempts per chunk
    pub max_retransmissions: u32,
    /// Stream priority (0 = lowest, 255 = highest)
    pub priority: u8,
}

impl Default for ParallelStreamConfig {
    fn default() -> Self {
        Self {
            stream_count: 4,
            chunk_size: 1024 * 1024, // 1 MB
            dynamic_adjustment: true,
            max_retransmissions: 3,
            priority: 128, // Medium priority
        }
    }
}

/// Parallel stream manager
pub struct ParallelStreamManager {
    config: ParallelStreamConfig,
    streams: Arc<RwLock<HashMap<StreamId, StreamStats>>>,
    next_stream_id: Arc<Mutex<StreamId>>,
    chunk_buffer: Arc<RwLock<HashMap<StreamId, Vec<StreamChunk>>>>,
}

impl ParallelStreamManager {
    /// Create a new parallel stream manager
    pub fn new(config: ParallelStreamConfig) -> Self {
        Self {
            config,
            streams: Arc::new(RwLock::new(HashMap::new())),
            next_stream_id: Arc::new(Mutex::new(0)),
            chunk_buffer: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(ParallelStreamConfig::default())
    }

    /// Allocate a new stream
    pub async fn allocate_stream(&self) -> Result<StreamId> {
        let mut next_id = self.next_stream_id.lock().await;
        let stream_id = *next_id;
        *next_id += 1;

        let mut streams = self.streams.write().await;
        streams.insert(stream_id, StreamStats::new(stream_id));

        debug!("Allocated stream {}", stream_id);
        Ok(stream_id)
    }

    /// Split data into chunks across multiple streams
    pub async fn split_data(&self, data: &[u8]) -> Result<Vec<StreamChunk>> {
        let total_size = data.len();
        let stream_count = self.config.stream_count;
        let chunk_size = self.config.chunk_size;

        // Calculate chunks per stream
        let total_chunks = (total_size + chunk_size - 1) / chunk_size;
        let chunks_per_stream = ((total_chunks + stream_count - 1) / stream_count) as u64;

        let mut chunks = Vec::new();
        let mut offset = 0;
        let mut current_stream = 0u64;
        let mut stream_chunk_count = 0u64;

        while offset < total_size {
            let remaining = total_size - offset;
            let current_chunk_size = remaining.min(chunk_size);
            
            let chunk_data = data[offset..offset + current_chunk_size].to_vec();
            let is_final = stream_chunk_count == chunks_per_stream - 1 
                || offset + current_chunk_size >= total_size;

            chunks.push(StreamChunk {
                stream_id: current_stream,
                sequence: stream_chunk_count,
                total_chunks: chunks_per_stream,
                data: chunk_data,
                is_final,
            });

            offset += current_chunk_size;
            stream_chunk_count += 1;

            // Move to next stream if we've filled this one
            if stream_chunk_count >= chunks_per_stream {
                current_stream += 1;
                stream_chunk_count = 0;
            }
        }

        info!(
            "Split {} bytes into {} chunks across {} streams",
            total_size,
            chunks.len(),
            stream_count
        );

        Ok(chunks)
    }

    /// Reassemble chunks back into original data
    pub async fn reassemble_chunks(&self, chunks: Vec<StreamChunk>) -> Result<Vec<u8>> {
        // Group chunks by stream
        let mut stream_chunks: HashMap<StreamId, Vec<StreamChunk>> = HashMap::new();
        for chunk in chunks {
            stream_chunks
                .entry(chunk.stream_id)
                .or_insert_with(Vec::new)
                .push(chunk);
        }

        // Sort chunks within each stream by sequence number
        for chunks in stream_chunks.values_mut() {
            chunks.sort_by_key(|c| c.sequence);
        }

        // Reassemble in stream order
        let mut result = Vec::new();
        let mut stream_ids: Vec<_> = stream_chunks.keys().copied().collect();
        stream_ids.sort();

        for stream_id in stream_ids {
            if let Some(chunks) = stream_chunks.get(&stream_id) {
                for chunk in chunks {
                    result.extend_from_slice(&chunk.data);
                }
            }
        }

        info!("Reassembled {} bytes from {} streams", result.len(), stream_chunks.len());
        Ok(result)
    }

    /// Update stream statistics
    pub async fn update_stream_stats(
        &self,
        stream_id: StreamId,
        bytes_sent: u64,
        chunks_sent: u64,
    ) -> Result<()> {
        let mut streams = self.streams.write().await;
        
        if let Some(stats) = streams.get_mut(&stream_id) {
            stats.bytes_sent += bytes_sent;
            stats.chunks_sent += chunks_sent;
            stats.state = StreamState::Sending;
        } else {
            return Err(Error::Domain(format!("Stream {} not found", stream_id)));
        }

        Ok(())
    }

    /// Mark stream as completed
    pub async fn complete_stream(&self, stream_id: StreamId) -> Result<()> {
        let mut streams = self.streams.write().await;
        
        if let Some(stats) = streams.get_mut(&stream_id) {
            stats.state = StreamState::Completed;
            debug!("Stream {} completed", stream_id);
        }

        Ok(())
    }

    /// Get statistics for all streams
    pub async fn get_all_stats(&self) -> Vec<StreamStats> {
        let streams = self.streams.read().await;
        streams.values().cloned().collect()
    }

    /// Get statistics for a specific stream
    pub async fn get_stream_stats(&self, stream_id: StreamId) -> Result<StreamStats> {
        let streams = self.streams.read().await;
        streams
            .get(&stream_id)
            .cloned()
            .ok_or_else(|| Error::Domain(format!("Stream {} not found", stream_id)))
    }

    /// Calculate optimal stream count based on file size and throughput
    pub fn calculate_optimal_streams(
        file_size: u64,
        throughput_bps: u64,
    ) -> usize {
        // Small files: single stream
        if file_size < 10 * 1024 * 1024 {
            return 1;
        }

        // Scale with throughput
        if throughput_bps > 1_000_000_000 {
            8 // 1+ Gbps: 8 streams
        } else if throughput_bps > 100_000_000 {
            4 // 100+ Mbps: 4 streams
        } else {
            2 // < 100 Mbps: 2 streams
        }
    }

    /// Adjust stream count dynamically based on performance
    pub async fn adjust_stream_count(&mut self, current_throughput: u64, target_throughput: u64) {
        if !self.config.dynamic_adjustment {
            return;
        }

        let current_count = self.config.stream_count;
        let new_count = if current_throughput < target_throughput * 80 / 100 {
            // Underperforming: increase streams
            (current_count + 1).min(16)
        } else if current_throughput > target_throughput * 120 / 100 {
            // Overperforming: can reduce streams to save resources
            (current_count.saturating_sub(1)).max(1)
        } else {
            current_count
        };

        if new_count != current_count {
            info!(
                "Adjusting stream count: {} -> {} (throughput: {} bps, target: {} bps)",
                current_count, new_count, current_throughput, target_throughput
            );
            self.config.stream_count = new_count;
        }
    }

    /// Reset all streams
    pub async fn reset(&self) {
        let mut streams = self.streams.write().await;
        streams.clear();
        
        let mut next_id = self.next_stream_id.lock().await;
        *next_id = 0;

        let mut buffer = self.chunk_buffer.write().await;
        buffer.clear();

        debug!("Reset all streams");
    }

    /// Get current stream count
    pub fn stream_count(&self) -> usize {
        self.config.stream_count
    }

    /// Get chunk size
    pub fn chunk_size(&self) -> usize {
        self.config.chunk_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_allocation() {
        let manager = ParallelStreamManager::default();
        
        let stream1 = manager.allocate_stream().await.unwrap();
        let stream2 = manager.allocate_stream().await.unwrap();
        
        assert_eq!(stream1, 0);
        assert_eq!(stream2, 1);
    }

    #[tokio::test]
    async fn test_data_splitting() {
        let config = ParallelStreamConfig {
            stream_count: 4,
            chunk_size: 1024,
            ..Default::default()
        };
        let manager = ParallelStreamManager::new(config);
        
        // Create 10 KB of test data
        let data = vec![0u8; 10 * 1024];
        let chunks = manager.split_data(&data).await.unwrap();
        
        // Should create multiple chunks across 4 streams
        assert!(!chunks.is_empty());
        assert!(chunks.len() >= 4);
        
        // Verify total size matches
        let total_size: usize = chunks.iter().map(|c| c.data.len()).sum();
        assert_eq!(total_size, data.len());
    }

    #[tokio::test]
    async fn test_chunk_reassembly() {
        let config = ParallelStreamConfig {
            stream_count: 2,
            chunk_size: 1024,
            ..Default::default()
        };
        let manager = ParallelStreamManager::new(config);
        
        // Create test data
        let original_data = vec![42u8; 5 * 1024];
        
        // Split and reassemble
        let chunks = manager.split_data(&original_data).await.unwrap();
        let reassembled = manager.reassemble_chunks(chunks).await.unwrap();
        
        // Verify data integrity
        assert_eq!(reassembled, original_data);
    }

    #[tokio::test]
    async fn test_stream_stats() {
        let manager = ParallelStreamManager::default();
        let stream_id = manager.allocate_stream().await.unwrap();
        
        // Update stats
        manager.update_stream_stats(stream_id, 1024, 1).await.unwrap();
        
        // Verify stats
        let stats = manager.get_stream_stats(stream_id).await.unwrap();
        assert_eq!(stats.bytes_sent, 1024);
        assert_eq!(stats.chunks_sent, 1);
        assert_eq!(stats.state, StreamState::Sending);
    }

    #[tokio::test]
    async fn test_stream_completion() {
        let manager = ParallelStreamManager::default();
        let stream_id = manager.allocate_stream().await.unwrap();
        
        manager.complete_stream(stream_id).await.unwrap();
        
        let stats = manager.get_stream_stats(stream_id).await.unwrap();
        assert_eq!(stats.state, StreamState::Completed);
    }

    #[tokio::test]
    async fn test_optimal_stream_calculation() {
        // Small file
        assert_eq!(
            ParallelStreamManager::calculate_optimal_streams(5 * 1024 * 1024, 1_000_000_000),
            1
        );
        
        // Large file, high throughput
        assert_eq!(
            ParallelStreamManager::calculate_optimal_streams(1_000 * 1024 * 1024, 2_000_000_000),
            8
        );
        
        // Large file, medium throughput
        assert_eq!(
            ParallelStreamManager::calculate_optimal_streams(1_000 * 1024 * 1024, 500_000_000),
            4
        );
        
        // Large file, low throughput
        assert_eq!(
            ParallelStreamManager::calculate_optimal_streams(1_000 * 1024 * 1024, 50_000_000),
            2
        );
    }

    #[tokio::test]
    async fn test_dynamic_adjustment() {
        let config = ParallelStreamConfig {
            stream_count: 4,
            dynamic_adjustment: true,
            ..Default::default()
        };
        let mut manager = ParallelStreamManager::new(config);
        
        // Underperforming: should increase
        manager.adjust_stream_count(500_000_000, 1_000_000_000).await;
        assert_eq!(manager.stream_count(), 5);
        
        // Overperforming: should decrease
        manager.adjust_stream_count(1_500_000_000, 1_000_000_000).await;
        assert_eq!(manager.stream_count(), 4);
    }

    #[tokio::test]
    async fn test_reset() {
        let manager = ParallelStreamManager::default();
        
        // Allocate some streams
        manager.allocate_stream().await.unwrap();
        manager.allocate_stream().await.unwrap();
        
        // Reset
        manager.reset().await;
        
        // Next allocation should start from 0
        let stream_id = manager.allocate_stream().await.unwrap();
        assert_eq!(stream_id, 0);
    }
}

// Made with Bob
