//! File transfer server implementation for receiving files

use crate::domain::TransportConnection;
use crate::error::{Error, Result};
use crate::protocol::{
    ChunkAckMessage, ErrorCode, Message, TransferAckMessage, TransferCompleteMessage,
};
use crate::protocol::types::{TransferConfig, TransferStats};
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};
use uuid::Uuid;

/// Transfer server for receiving files
pub struct TransferServer {
    config: TransferConfig,
    output_dir: std::path::PathBuf,
}

impl TransferServer {
    /// Create a new transfer server with the given configuration and output directory
    pub fn new(config: TransferConfig, output_dir: impl AsRef<Path>) -> Self {
        Self {
            config,
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }

    /// Receive a file over the connection
    pub async fn receive_file(&self, conn: &mut dyn TransportConnection) -> Result<TransferStats> {
        info!("Waiting for transfer...");

        // Wait for TRANSFER_START
        let mut recv_buf = vec![0u8; 65536];
        let n = conn.recv(&mut recv_buf).await?;
        let msg: Message = bincode::deserialize(&recv_buf[..n])
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let (transfer_id, file_name, _file_size, total_chunks) = match msg {
            Message::TransferStart(start) => {
                info!("Receiving file: {} ({} bytes)", start.file_name, start.file_size);
                (
                    start.transfer_id,
                    start.file_name,
                    start.file_size,
                    start.total_chunks,
                )
            }
            Message::Error(err) => {
                return Err(Error::Protocol(err.message));
            }
            _ => {
                return Err(Error::Protocol(
                    "Expected TRANSFER_START".to_string(),
                ));
            }
        };

        // Send ACK
        let ack = TransferAckMessage {
            transfer_id,
            status: ErrorCode::Success,
            resume_offset: 0,
            available_space: u64::MAX,
            preferred_chunk_size: self.config.chunk_size,
        };
        let msg_bytes = bincode::serialize(&Message::TransferAck(ack))
            .map_err(|e| Error::Serialization(e.to_string()))?;
        conn.send(&msg_bytes).await?;
        debug!("Sent TRANSFER_ACK");

        // Receive chunks
        let output_path = self.output_dir.join(&file_name);
        let start_time = Instant::now();
        let stats = self
            .receive_chunks(conn, &output_path, transfer_id, total_chunks, start_time)
            .await?;

        // Wait for TRANSFER_END
        let n = conn.recv(&mut recv_buf).await?;
        let msg: Message = bincode::deserialize(&recv_buf[..n])
            .map_err(|e| Error::Serialization(e.to_string()))?;

        match msg {
            Message::TransferEnd(_end) => {
                debug!("Received TRANSFER_END");
            }
            Message::Error(err) => {
                return Err(Error::Protocol(err.message));
            }
            _ => {
                return Err(Error::Protocol("Expected TRANSFER_END".to_string()));
            }
        }

        // Send TRANSFER_COMPLETE
        let complete = TransferCompleteMessage {
            transfer_id,
            status: ErrorCode::Success,
            total_bytes_received: stats.total_bytes,
            verified_hash: [0u8; 32], // TODO: Calculate actual hash
            storage_path: output_path.to_string_lossy().to_string(),
        };
        let msg_bytes = bincode::serialize(&Message::TransferComplete(complete))
            .map_err(|e| Error::Serialization(e.to_string()))?;
        conn.send(&msg_bytes).await?;

        info!("Transfer completed successfully");
        Ok(stats)
    }

    async fn receive_chunks(
        &self,
        conn: &mut dyn TransportConnection,
        output_path: &Path,
        transfer_id: Uuid,
        total_chunks: u32,
        start_time: Instant,
    ) -> Result<TransferStats> {
        let mut file = File::create(output_path)
            .await
            .map_err(|e| Error::Io(e))?;

        let mut bytes_received = 0u64;
        let mut chunks_received = 0u32;
        let mut recv_buf = vec![0u8; 65536];

        for _ in 0..total_chunks {
            let n = conn.recv(&mut recv_buf).await?;
            let msg: Message = bincode::deserialize(&recv_buf[..n])
                .map_err(|e| Error::Serialization(e.to_string()))?;

            match msg {
                Message::ChunkData(chunk) => {
                    // Verify transfer ID
                    if chunk.transfer_id != transfer_id {
                        return Err(Error::Protocol("Transfer ID mismatch".to_string()));
                    }

                    // Write chunk data
                    file.write_all(&chunk.data)
                        .await
                        .map_err(|e| Error::Io(e))?;

                    bytes_received += chunk.data.len() as u64;
                    chunks_received += 1;

                    // Send ACK
                    let ack = ChunkAckMessage::new(
                        transfer_id,
                        chunk.chunk_index,
                        chunk.data.len() as u32,
                    );
                    let msg_bytes = bincode::serialize(&Message::ChunkAck(ack))
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    conn.send(&msg_bytes).await?;

                    debug!("Received and acknowledged chunk {}", chunk.chunk_index);
                }
                Message::Error(err) => {
                    return Err(Error::Protocol(err.message));
                }
                _ => {
                    return Err(Error::Protocol("Expected CHUNK_DATA".to_string()));
                }
            }
        }

        file.flush()
            .await
            .map_err(|e| Error::Io(e))?;

        let duration = start_time.elapsed();
        let average_speed_bps = if duration.as_secs_f64() > 0.0 {
            bytes_received as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        Ok(TransferStats {
            total_bytes: bytes_received,
            compressed_bytes: bytes_received,
            chunks_sent: chunks_received,
            chunks_retried: 0,
            duration,
            average_speed_bps,
            compression_ratio: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_server_creation() {
        let config = TransferConfig::default();
        let server = TransferServer::new(config, "/tmp");
        assert_eq!(server.output_dir, std::path::PathBuf::from("/tmp"));
    }
}

// Made with Bob
