//! File transfer client implementation with flow control and resume support

use crate::domain::TransportConnection;
use crate::error::{Error, Result};
use crate::protocol::{
    ChunkDataMessage, ChunkFlags, ErrorCode, Message, TransferEndMessage,
    TransferStartMessage,
};
use crate::protocol::types::{ProgressCallback, TransferConfig, TransferProgress, TransferStats};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Transfer client for sending files
pub struct TransferClient {
    config: TransferConfig,
    progress_callback: Option<ProgressCallback>,
}

impl TransferClient {
    /// Create a new transfer client with the given configuration
    pub fn new(config: TransferConfig) -> Self {
        Self {
            config,
            progress_callback: None,
        }
    }

    /// Set a progress callback for transfer updates
    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    /// Send a file over the connection
    pub async fn send_file(
        &self,
        conn: &mut dyn TransportConnection,
        file_path: impl AsRef<Path>,
    ) -> Result<TransferStats> {
        let file_path = file_path.as_ref();
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::InvalidInput("Invalid file name".to_string()))?
            .to_string();

        info!("Starting transfer of {}", file_name);

        // Open file
        let mut file = File::open(file_path)
            .await
            .map_err(|e| Error::Io(e))?;

        let file_size = file
            .metadata()
            .await
            .map_err(|e| Error::Io(e))?
            .len();

        // Calculate total chunks
        let total_chunks = ((file_size + self.config.chunk_size as u64 - 1)
            / self.config.chunk_size as u64) as u32;

        // Calculate file hash
        let file_hash = self.calculate_file_hash(file_path).await?;

        // Send TRANSFER_START
        let start_msg = TransferStartMessage::new(
            file_name.clone(),
            file_size,
            total_chunks,
            self.config.chunk_size,
            file_hash,
        );
        let transfer_id = start_msg.transfer_id;

        let msg_bytes = bincode::serialize(&Message::TransferStart(start_msg))
            .map_err(|e| Error::Serialization(e.to_string()))?;
        conn.send(&msg_bytes).await?;
        debug!("Sent TRANSFER_START");

        // Wait for ACK
        let mut recv_buf = vec![0u8; 65536];
        let n = conn.recv(&mut recv_buf).await?;
        let msg: Message = bincode::deserialize(&recv_buf[..n])
            .map_err(|e| Error::Serialization(e.to_string()))?;

        match msg {
            Message::TransferAck(ack) => {
                if ack.status != ErrorCode::Success {
                    return Err(Error::Protocol(format!(
                        "Server rejected transfer: {:?}",
                        ack.status
                    )));
                }
                debug!("Received TRANSFER_ACK");
            }
            Message::Error(err) => {
                return Err(Error::Protocol(err.message));
            }
            _ => {
                return Err(Error::Protocol(
                    "Expected TRANSFER_ACK".to_string(),
                ));
            }
        }

        // Transfer chunks
        let start_time = Instant::now();
        let stats = self
            .transfer_chunks(conn, &mut file, transfer_id, file_size, total_chunks, start_time)
            .await?;

        // Send TRANSFER_END
        let end_msg = TransferEndMessage {
            transfer_id,
            total_bytes_sent: stats.total_bytes,
            total_chunks: stats.chunks_sent,
            compression_ratio: stats.compression_ratio,
            transfer_duration_ms: stats.duration.as_millis() as u64,
            file_hash,
        };

        let msg_bytes = bincode::serialize(&Message::TransferEnd(end_msg))
            .map_err(|e| Error::Serialization(e.to_string()))?;
        conn.send(&msg_bytes).await?;
        debug!("Sent TRANSFER_END");

        // Wait for TRANSFER_COMPLETE
        let n = conn.recv(&mut recv_buf).await?;
        let msg: Message = bincode::deserialize(&recv_buf[..n])
            .map_err(|e| Error::Serialization(e.to_string()))?;

        match msg {
            Message::TransferComplete(complete) => {
                if complete.status != ErrorCode::Success {
                    return Err(Error::Protocol(format!(
                        "Transfer failed: {:?}",
                        complete.status
                    )));
                }
                info!("Transfer completed successfully");
            }
            Message::Error(err) => {
                return Err(Error::Protocol(err.message));
            }
            _ => {
                return Err(Error::Protocol(
                    "Expected TRANSFER_COMPLETE".to_string(),
                ));
            }
        }

        Ok(stats)
    }

    async fn transfer_chunks(
        &self,
        conn: &mut dyn TransportConnection,
        file: &mut File,
        transfer_id: Uuid,
        file_size: u64,
        total_chunks: u32,
        start_time: Instant,
    ) -> Result<TransferStats> {
        let mut bytes_transferred = 0u64;
        let mut compressed_bytes = 0u64;
        let mut chunks_sent = 0u32;
        let mut chunks_retried = 0u32;

        // Pending acknowledgments with retry count
        let pending_acks: Arc<Mutex<HashMap<u32, (Instant, u32)>>> = Arc::new(Mutex::new(HashMap::new()));
        // Cache of chunk data for retransmission
        let chunk_cache: Arc<Mutex<HashMap<u32, ChunkDataMessage>>> = Arc::new(Mutex::new(HashMap::new()));

        for chunk_index in 0..total_chunks {
            // Read chunk
            let mut buffer = vec![0u8; self.config.chunk_size as usize];
            let bytes_read = file
                .read(&mut buffer)
                .await
                .map_err(|e| Error::Io(e))?;

            buffer.truncate(bytes_read);

            // Calculate chunk hash
            let chunk_hash = self.calculate_chunk_hash(&buffer);

            // Create chunk message
            let chunk_offset = chunk_index as u64 * self.config.chunk_size as u64;
            let mut chunk_msg = ChunkDataMessage::new(
                transfer_id,
                chunk_index,
                chunk_offset,
                buffer,
                chunk_hash,
            );

            // Set flags
            let mut flags = ChunkFlags::new();
            if chunk_index == total_chunks - 1 {
                flags = flags.with_last_chunk();
            }
            chunk_msg.flags = flags.0;

            // Send chunk
            let msg_bytes = bincode::serialize(&Message::ChunkData(chunk_msg.clone()))
                .map_err(|e| Error::Serialization(e.to_string()))?;
            conn.send(&msg_bytes).await?;
            chunks_sent += 1;

            // Cache chunk for potential retransmission
            {
                let mut cache = chunk_cache.lock().await;
                cache.insert(chunk_index, chunk_msg.clone());
            }

            // Track pending ACK with retry count
            {
                let mut pending = pending_acks.lock().await;
                pending.insert(chunk_index, (Instant::now(), 0));
            }

            // Update progress
            bytes_transferred += bytes_read as u64;
            compressed_bytes += chunk_msg.compressed_size as u64;

            if let Some(callback) = &self.progress_callback {
                let elapsed = start_time.elapsed();
                let speed_bps = if elapsed.as_secs_f64() > 0.0 {
                    bytes_transferred as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                callback(TransferProgress {
                    transfer_id,
                    bytes_transferred,
                    total_bytes: file_size,
                    chunks_completed: chunk_index + 1,
                    total_chunks,
                    elapsed,
                    speed_bps,
                });
            }

            // Check for timeouts and retry failed chunks
            self.check_and_retry_timeouts(conn, &pending_acks, &chunk_cache, &mut chunks_retried).await?;

            // Wait for ACK if window is full
            {
                let pending = pending_acks.lock().await;
                if pending.len() >= self.config.send_window {
                    drop(pending);
                    self.wait_for_ack(conn, &pending_acks).await?;
                }
            }

            // Bandwidth throttling
            if self.config.bandwidth_limit > 0 {
                let elapsed = start_time.elapsed().as_secs_f64();
                let target_bytes = (elapsed * self.config.bandwidth_limit as f64) as u64;
                if bytes_transferred > target_bytes {
                    let sleep_time = ((bytes_transferred - target_bytes) as f64
                        / self.config.bandwidth_limit as f64)
                        * 1000.0;
                    tokio::time::sleep(Duration::from_millis(sleep_time as u64)).await;
                }
            }
        }

        // Wait for all remaining ACKs
        while !pending_acks.lock().await.is_empty() {
            self.wait_for_ack(conn, &pending_acks).await?;
        }

        let duration = start_time.elapsed();
        let average_speed_bps = if duration.as_secs_f64() > 0.0 {
            bytes_transferred as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let compression_ratio = if compressed_bytes > 0 {
            bytes_transferred as f32 / compressed_bytes as f32
        } else {
            1.0
        };

        Ok(TransferStats {
            total_bytes: bytes_transferred,
            compressed_bytes,
            chunks_sent,
            chunks_retried,
            duration,
            average_speed_bps,
            compression_ratio,
        })
    }

    async fn wait_for_ack(
        &self,
        conn: &mut dyn TransportConnection,
        pending_acks: &Arc<Mutex<HashMap<u32, (Instant, u32)>>>,
    ) -> Result<()> {
        let mut recv_buf = vec![0u8; 65536];
        let n = conn.recv(&mut recv_buf).await?;
        let msg: Message = bincode::deserialize(&recv_buf[..n])
            .map_err(|e| Error::Serialization(e.to_string()))?;

        match msg {
            Message::ChunkAck(ack) => {
                if ack.status == ErrorCode::Success {
                    let mut pending = pending_acks.lock().await;
                    pending.remove(&ack.chunk_index);
                    debug!("Received ACK for chunk {}", ack.chunk_index);
                } else {
                    warn!("Chunk {} failed: {:?}", ack.chunk_index, ack.status);
                    // Mark for retry instead of failing immediately
                    let mut pending = pending_acks.lock().await;
                    if let Some((_, retry_count)) = pending.get_mut(&ack.chunk_index) {
                        *retry_count += 1;
                    }
                }
            }
            Message::Error(err) => {
                return Err(Error::Protocol(err.message));
            }
            _ => {
                return Err(Error::Protocol("Expected CHUNK_ACK".to_string()));
            }
        }
        Ok(())
    }

    async fn check_and_retry_timeouts(
        &self,
        conn: &mut dyn TransportConnection,
        pending_acks: &Arc<Mutex<HashMap<u32, (Instant, u32)>>>,
        chunk_cache: &Arc<Mutex<HashMap<u32, ChunkDataMessage>>>,
        chunks_retried: &mut u32,
    ) -> Result<()> {
        let now = Instant::now();
        let mut to_retry = Vec::new();

        // Check for timeouts
        {
            let mut pending = pending_acks.lock().await;
            for (chunk_index, (sent_time, retry_count)) in pending.iter_mut() {
                if now.duration_since(*sent_time) > self.config.ack_timeout {
                    if *retry_count >= self.config.max_retries {
                        return Err(Error::Protocol(format!(
                            "Chunk {} exceeded max retries ({})",
                            chunk_index, self.config.max_retries
                        )));
                    }
                    to_retry.push(*chunk_index);
                    *retry_count += 1;
                    *sent_time = now;
                }
            }
        }

        // Retransmit timed-out chunks
        if !to_retry.is_empty() {
            let cache = chunk_cache.lock().await;
            for chunk_index in to_retry {
                if let Some(chunk_msg) = cache.get(&chunk_index) {
                    warn!("Retransmitting chunk {} due to timeout", chunk_index);
                    let msg_bytes = bincode::serialize(&Message::ChunkData(chunk_msg.clone()))
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    conn.send(&msg_bytes).await?;
                    *chunks_retried += 1;
                }
            }
        }

        Ok(())
    }

    async fn calculate_file_hash(&self, path: &Path) -> Result<[u8; 32]> {
        let mut file = File::open(path)
            .await
            .map_err(|e| Error::Io(e))?;

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .await
                .map_err(|e| Error::Io(e))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hasher.finalize().into())
    }

    fn calculate_chunk_hash(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_config_default() {
        let config = TransferConfig::default();
        assert_eq!(config.chunk_size, 4096);
        assert_eq!(config.send_window, 256);
        assert!(config.compression);
    }

    #[test]
    fn test_transfer_progress_percentage() {
        let progress = TransferProgress {
            transfer_id: Uuid::new_v4(),
            bytes_transferred: 50,
            total_bytes: 100,
            chunks_completed: 5,
            total_chunks: 10,
            elapsed: Duration::from_secs(1),
            speed_bps: 50.0,
        };
        assert_eq!(progress.percentage(), 50.0);
    }

    #[test]
    fn test_transfer_progress_eta() {
        let progress = TransferProgress {
            transfer_id: Uuid::new_v4(),
            bytes_transferred: 50,
            total_bytes: 100,
            chunks_completed: 5,
            total_chunks: 10,
            elapsed: Duration::from_secs(1),
            speed_bps: 50.0,
        };
        let eta = progress.eta().unwrap();
        assert_eq!(eta.as_secs(), 1);
    }
}

// Made with Bob
