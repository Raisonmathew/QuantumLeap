//! Content-addressable storage for QLTP
//!
//! Provides deduplication and efficient storage of file chunks.

use anyhow::anyhow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, instrument};

/// Chunk identifier (SHA-256 hash as hex string)
pub type ChunkId = String;

/// Error type for storage operations
pub type Error = anyhow::Error;

/// Result type for storage operations
pub type Result<T> = std::result::Result<T, Error>;

/// Content-addressable storage
pub struct ContentStore {
    /// Base directory for storage
    base_dir: PathBuf,
    /// In-memory index of chunks
    index: HashMap<ChunkId, ChunkMetadata>,
}

/// Metadata for a stored chunk
#[derive(Debug, Clone)]
struct ChunkMetadata {
    /// Size in bytes
    size: usize,
    /// Reference count (for garbage collection)
    ref_count: usize,
    /// Path to the chunk file
    path: PathBuf,
}

impl ContentStore {
    /// Create a new content store
    #[instrument(skip(base_dir))]
    pub async fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        
        // Create base directory if it doesn't exist
        fs::create_dir_all(&base_dir).await?;
        
        info!("Initialized content store at {}", base_dir.display());
        
        let mut store = Self {
            base_dir,
            index: HashMap::new(),
        };
        
        // Load existing chunks
        store.load_index().await?;
        
        Ok(store)
    }

    /// Store a chunk
    #[instrument(skip(self, data))]
    pub async fn store(&mut self, chunk_id: &ChunkId, data: &[u8]) -> Result<()> {
        // Check if chunk already exists
        if let Some(metadata) = self.index.get_mut(chunk_id) {
            debug!("Chunk {} already exists, incrementing ref count", chunk_id);
            metadata.ref_count += 1;
            return Ok(());
        }

        // Create chunk file path
        let chunk_path = self.chunk_path(chunk_id);
        
        // Create parent directory
        if let Some(parent) = chunk_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write chunk data
        let mut file = fs::File::create(&chunk_path).await?;
        file.write_all(data).await?;
        file.sync_all().await?;

        debug!("Stored chunk {} ({} bytes)", chunk_id, data.len());

        // Update index
        self.index.insert(
            chunk_id.clone(),
            ChunkMetadata {
                size: data.len(),
                ref_count: 1,
                path: chunk_path,
            },
        );

        Ok(())
    }

    /// Retrieve a chunk
    #[instrument(skip(self))]
    pub async fn retrieve(&self, chunk_id: &ChunkId) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;

        let metadata = self
            .index
            .get(chunk_id)
            .ok_or_else(|| anyhow!("Chunk not found: {}", chunk_id))?;

        // SECURITY/RELIABILITY (CWE-770): bound the on-disk read to the
        // size we recorded at insert time. If a chunk file has grown
        // since (corruption, tampering, race), `read_to_end` would
        // happily allocate the new size; instead we cap the reader and
        // require the byte count to match the recorded metadata.
        let file = fs::File::open(&metadata.path).await?;
        let mut limited = file.take(metadata.size as u64);
        let mut data = Vec::with_capacity(metadata.size);
        limited.read_to_end(&mut data).await?;
        if data.len() != metadata.size {
            return Err(anyhow!(
                "Chunk {} size mismatch: expected {}, read {}",
                chunk_id,
                metadata.size,
                data.len()
            ));
        }

        debug!("Retrieved chunk {} ({} bytes)", chunk_id, data.len());

        Ok(data)
    }

    /// Check if a chunk exists
    pub fn contains(&self, chunk_id: &str) -> bool {
        self.index.contains_key(chunk_id)
    }

    /// Get chunk size
    pub fn chunk_size(&self, chunk_id: &ChunkId) -> Option<usize> {
        self.index.get(chunk_id).map(|m| m.size)
    }

    /// Get total number of chunks
    pub fn chunk_count(&self) -> usize {
        self.index.len()
    }

    /// Get total storage size
    pub fn total_size(&self) -> u64 {
        self.index.values().map(|m| m.size as u64).sum()
    }

    /// Delete a chunk (decrements ref count, removes if zero)
    #[instrument(skip(self))]
    pub async fn delete(&mut self, chunk_id: &ChunkId) -> Result<bool> {
        let metadata = self
            .index
            .get_mut(chunk_id)
            .ok_or_else(|| anyhow!("Chunk not found: {}", chunk_id))?;

        metadata.ref_count -= 1;

        if metadata.ref_count == 0 {
            // Remove chunk file
            let path = metadata.path.clone();
            fs::remove_file(&path).await?;
            
            // Remove from index
            self.index.remove(chunk_id);
            
            debug!("Deleted chunk {}", chunk_id);
            Ok(true)
        } else {
            debug!(
                "Decremented ref count for chunk {} (now {})",
                chunk_id, metadata.ref_count
            );
            Ok(false)
        }
    }

    /// Load index from disk
    async fn load_index(&mut self) -> Result<()> {
        // Walk through storage directory and rebuild index
        let mut count = 0;
        
        // Read subdirectories (first 2 chars of hash)
        let mut entries = fs::read_dir(&self.base_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_dir() {
                // Read files in subdirectory
                let mut subdir_entries = fs::read_dir(&path).await?;
                
                while let Some(subentry) = subdir_entries.next_entry().await? {
                    let file_path = subentry.path();
                    
                    if file_path.is_file() {
                        if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
                            // ChunkId is just a String (hex), so use filename directly
                            let chunk_id = filename.to_string();
                            let metadata = fs::metadata(&file_path).await?;
                            
                            self.index.insert(
                                chunk_id,
                                ChunkMetadata {
                                    size: metadata.len() as usize,
                                    ref_count: 1,
                                    path: file_path.clone(),
                                },
                            );
                            
                            count += 1;
                        }
                    }
                }
            }
        }

        if count > 0 {
            info!("Loaded {} chunks from storage", count);
        }

        Ok(())
    }

    /// Get path for a chunk
    fn chunk_path(&self, chunk_id: &ChunkId) -> PathBuf {
        // Use first 2 chars of hash as subdirectory for better filesystem performance
        // ChunkId is already a hex string
        let subdir = &chunk_id[..2];
        self.base_dir.join(subdir).join(chunk_id)
    }

    /// Clear all chunks (for testing)
    #[cfg(test)]
    pub async fn clear(&mut self) -> Result<()> {
        for metadata in self.index.values() {
            let _ = fs::remove_file(&metadata.path).await;
        }
        self.index.clear();
        Ok(())
    }
}

/// Deduplication engine
pub struct DeduplicationEngine {
    store: ContentStore,
}

impl DeduplicationEngine {
    /// Create a new deduplication engine
    pub async fn new(storage_dir: impl AsRef<Path>) -> Result<Self> {
        let store = ContentStore::new(storage_dir).await?;
        Ok(Self { store })
    }

    /// Deduplicate chunks
    #[instrument(skip(self, chunks))]
    pub async fn deduplicate<T>(&mut self, chunks: &[T]) -> Result<DeduplicationResult>
    where
        T: AsRef<str> + std::fmt::Debug + Clone,
    {
        let mut unique_indices = Vec::new();
        let mut duplicate_indices = Vec::new();
        let mut total_count = 0;

        for (idx, chunk) in chunks.iter().enumerate() {
            total_count += 1;
            let chunk_id = chunk.as_ref();

            if self.store.contains(chunk_id) {
                duplicate_indices.push(idx);
            } else {
                unique_indices.push(idx);
            }
        }

        let dedup_ratio = if !unique_indices.is_empty() {
            total_count as f64 / unique_indices.len() as f64
        } else {
            1.0
        };

        debug!(
            "Deduplication: {} total chunks, {} unique, {} duplicates (ratio: {:.2}x)",
            total_count,
            unique_indices.len(),
            duplicate_indices.len(),
            dedup_ratio
        );

        Ok(DeduplicationResult {
            unique_indices,
            duplicate_indices,
            total_count,
            dedup_ratio,
        })
    }

    /// Get the underlying content store
    pub fn store(&self) -> &ContentStore {
        &self.store
    }

    /// Get mutable access to the content store
    pub fn store_mut(&mut self) -> &mut ContentStore {
        &mut self.store
    }
}

/// Result of deduplication
#[derive(Debug, Clone)]
pub struct DeduplicationResult {
    /// Indices of unique chunks that need to be transferred
    pub unique_indices: Vec<usize>,
    /// Indices of duplicate chunks that already exist
    pub duplicate_indices: Vec<usize>,
    /// Total number of chunks
    pub total_count: usize,
    /// Deduplication ratio
    pub dedup_ratio: f64,
}

impl DeduplicationResult {
    /// Calculate percentage of chunks that are unique
    pub fn unique_percentage(&self) -> f64 {
        if self.total_count == 0 {
            return 100.0;
        }
        (self.unique_indices.len() as f64 / self.total_count as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    fn compute_hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    #[tokio::test]
    async fn test_content_store() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = ContentStore::new(temp_dir.path()).await.unwrap();

        // Store a chunk
        let data = b"Hello, World!";
        let hash = compute_hash(data);
        let chunk_id = hex::encode(hash);

        store.store(&chunk_id, data).await.unwrap();
        assert!(store.contains(&chunk_id));
        assert_eq!(store.chunk_count(), 1);

        // Retrieve the chunk
        let retrieved = store.retrieve(&chunk_id).await.unwrap();
        assert_eq!(data, retrieved.as_slice());

        // Store same chunk again (should increment ref count)
        store.store(&chunk_id, data).await.unwrap();
        assert_eq!(store.chunk_count(), 1);

        // Delete once (should decrement ref count)
        let deleted = store.delete(&chunk_id).await.unwrap();
        assert!(!deleted);
        assert!(store.contains(&chunk_id));

        // Delete again (should remove)
        let deleted = store.delete(&chunk_id).await.unwrap();
        assert!(deleted);
        assert!(!store.contains(&chunk_id));
    }

    #[tokio::test]
    async fn test_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let mut engine = DeduplicationEngine::new(temp_dir.path()).await.unwrap();

        // Create some chunks
        let data1 = b"Chunk 1";
        let data2 = b"Chunk 2";
        let data3 = b"Chunk 1"; // Duplicate of data1

        let hash1 = hex::encode(compute_hash(data1));
        let hash2 = hex::encode(compute_hash(data2));
        let hash3 = hex::encode(compute_hash(data3));

        let chunk_ids = vec![hash1.clone(), hash2.clone(), hash3.clone()];

        // First deduplication (all unique since nothing stored yet)
        let result = engine.deduplicate(&chunk_ids).await.unwrap();
        assert_eq!(result.unique_indices.len(), 3); // All are unique initially
        assert_eq!(result.duplicate_indices.len(), 0);

        // Store first two chunks
        engine.store_mut().store(&hash1, data1).await.unwrap();
        engine.store_mut().store(&hash2, data2).await.unwrap();

        // Second deduplication (hash1 and hash2 are duplicates, hash3 is also duplicate of hash1)
        let result2 = engine.deduplicate(&chunk_ids).await.unwrap();
        assert_eq!(result2.unique_indices.len(), 0); // All are now duplicates
        assert_eq!(result2.duplicate_indices.len(), 3); // hash1, hash2, and hash3 are all duplicates
        assert_eq!(result2.dedup_ratio, 1.0); // When all are duplicates, ratio is 1.0 (no unique chunks to divide by)
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create store and add chunk
        {
            let mut store = ContentStore::new(temp_dir.path()).await.unwrap();
            let data = b"Persistent data";
            let hash = compute_hash(data);
            let chunk_id = hex::encode(hash);
            
            store.store(&chunk_id, data).await.unwrap();
            assert_eq!(store.chunk_count(), 1);
        }

        // Create new store (should load existing chunks)
        {
            let store = ContentStore::new(temp_dir.path()).await.unwrap();
            assert_eq!(store.chunk_count(), 1);
        }
    }
}

// Made with Bob
