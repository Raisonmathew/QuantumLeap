//! File chunking functionality

use crate::{error::Result, hash::compute_hash, types::{ChunkId, ChunkInfo}};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::{debug, instrument};

/// Chunk a file into fixed-size pieces
#[instrument(skip(path))]
pub async fn chunk_file(path: impl AsRef<Path>, chunk_size: usize) -> Result<Vec<ChunkInfo>> {
    let path = path.as_ref();
    let mut file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    debug!(
        "Chunking file: {} ({} bytes, chunk_size: {})",
        path.display(),
        file_size,
        chunk_size
    );

    let mut chunks = Vec::new();
    let mut offset = 0u64;
    let mut buffer = vec![0u8; chunk_size];

    while offset < file_size {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }

        let chunk_data = &buffer[..bytes_read];
        let hash = compute_hash(chunk_data);
        let id = ChunkId::new(hash);

        chunks.push(ChunkInfo {
            id,
            size: bytes_read,
            offset,
            compressed: false,
            compression_ratio: 1.0,
        });

        offset += bytes_read as u64;
    }

    debug!("Created {} chunks", chunks.len());
    Ok(chunks)
}

/// Read a specific chunk from a file
#[instrument(skip(path))]
pub async fn read_chunk(path: impl AsRef<Path>, chunk_info: &ChunkInfo) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let mut file = tokio::fs::File::open(path).await?;
    
    file.seek(std::io::SeekFrom::Start(chunk_info.offset)).await?;
    
    let mut buffer = vec![0u8; chunk_info.size];
    file.read_exact(&mut buffer).await?;
    
    Ok(buffer)
}

/// Content-defined chunking using rolling hash (Rabin fingerprinting)
/// This creates variable-size chunks based on content, which improves deduplication
pub struct ContentDefinedChunker {
    min_chunk_size: usize,
    avg_chunk_size: usize,
    max_chunk_size: usize,
    mask: u64,
}

impl ContentDefinedChunker {
    /// Create a new content-defined chunker
    pub fn new(avg_chunk_size: usize) -> Self {
        Self {
            min_chunk_size: avg_chunk_size / 4,
            avg_chunk_size,
            max_chunk_size: avg_chunk_size * 4,
            mask: (1 << (avg_chunk_size.trailing_zeros())) - 1,
        }
    }

    /// Chunk a file using content-defined chunking
    #[instrument(skip(self, path))]
    pub async fn chunk_file(&self, path: impl AsRef<Path>) -> Result<Vec<ChunkInfo>> {
        let path = path.as_ref();
        let mut file = tokio::fs::File::open(path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        debug!(
            "Content-defined chunking: {} ({} bytes, avg_chunk: {})",
            path.display(),
            file_size,
            self.avg_chunk_size
        );

        let mut chunks = Vec::new();
        let _offset = 0u64;
        let mut buffer = Vec::new();
        let mut rolling_hash = 0u64;
        let mut chunk_start = 0u64;

        // Read entire file (for simplicity; in production, use streaming)
        file.read_to_end(&mut buffer).await?;

        for (i, &byte) in buffer.iter().enumerate() {
            // Update rolling hash
            rolling_hash = rolling_hash.wrapping_mul(31).wrapping_add(byte as u64);

            let chunk_size = i - chunk_start as usize;

            // Check if we should create a chunk
            let should_chunk = (rolling_hash & self.mask) == 0
                || chunk_size >= self.max_chunk_size;

            if should_chunk && chunk_size >= self.min_chunk_size {
                let chunk_data = &buffer[chunk_start as usize..i];
                let hash = compute_hash(chunk_data);
                let id = ChunkId::new(hash);

                chunks.push(ChunkInfo {
                    id,
                    size: chunk_data.len(),
                    offset: chunk_start,
                    compressed: false,
                    compression_ratio: 1.0,
                });

                chunk_start = i as u64;
            }
        }

        // Handle remaining data
        if chunk_start < buffer.len() as u64 {
            let chunk_data = &buffer[chunk_start as usize..];
            let hash = compute_hash(chunk_data);
            let id = ChunkId::new(hash);

            chunks.push(ChunkInfo {
                id,
                size: chunk_data.len(),
                offset: chunk_start,
                compressed: false,
                compression_ratio: 1.0,
            });
        }

        debug!(
            "Created {} content-defined chunks (avg size: {} bytes)",
            chunks.len(),
            if chunks.is_empty() {
                0
            } else {
                buffer.len() / chunks.len()
            }
        );

        Ok(chunks)
    }
}

impl Default for ContentDefinedChunker {
    fn default() -> Self {
        Self::new(4096)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_chunk_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = vec![0u8; 10000]; // 10KB
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let chunks = chunk_file(temp_file.path(), 4096).await.unwrap();
        
        // Should create 3 chunks: 4096 + 4096 + 1808
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].size, 4096);
        assert_eq!(chunks[1].size, 4096);
        assert_eq!(chunks[2].size, 1808);
    }

    #[tokio::test]
    async fn test_read_chunk() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = b"Hello, World!";
        temp_file.write_all(data).unwrap();
        temp_file.flush().unwrap();

        let chunks = chunk_file(temp_file.path(), 5).await.unwrap();
        assert_eq!(chunks.len(), 3);

        let chunk_data = read_chunk(temp_file.path(), &chunks[0]).await.unwrap();
        assert_eq!(&chunk_data, b"Hello");

        let chunk_data = read_chunk(temp_file.path(), &chunks[1]).await.unwrap();
        assert_eq!(&chunk_data, b", Wor");

        let chunk_data = read_chunk(temp_file.path(), &chunks[2]).await.unwrap();
        assert_eq!(&chunk_data, b"ld!");
    }

    #[tokio::test]
    async fn test_content_defined_chunking() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let data = vec![0u8; 10000]; // 10KB of zeros
        temp_file.write_all(&data).unwrap();
        temp_file.flush().unwrap();

        let chunker = ContentDefinedChunker::new(4096);
        let chunks = chunker.chunk_file(temp_file.path()).await.unwrap();

        // Content-defined chunking should create variable-size chunks
        assert!(!chunks.is_empty());
        
        // Verify total size matches
        let total_size: usize = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total_size, 10000);
    }
}

// Made with Bob
