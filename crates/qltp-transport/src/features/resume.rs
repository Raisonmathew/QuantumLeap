//! Transfer resume support
//!
//! This module provides functionality to save and restore transfer state,
//! allowing interrupted transfers to resume from where they left off.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Transfer state for resume capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferState {
    /// Unique transfer ID
    pub transfer_id: Uuid,
    /// Original file path
    pub file_path: PathBuf,
    /// File size in bytes
    pub file_size: u64,
    /// Total number of chunks
    pub total_chunks: u32,
    /// Chunk size in bytes
    pub chunk_size: u32,
    /// Last successfully transferred chunk index
    pub last_chunk_index: u32,
    /// Byte offset of last chunk
    pub last_chunk_offset: u64,
    /// Hash of partial file (up to last_chunk_offset)
    pub partial_file_hash: [u8; 32],
    /// Full file hash (for verification)
    pub full_file_hash: [u8; 32],
    /// Timestamp of last update
    pub last_update: u64,
}

impl TransferState {
    /// Create a new transfer state
    pub fn new(
        transfer_id: Uuid,
        file_path: PathBuf,
        file_size: u64,
        total_chunks: u32,
        chunk_size: u32,
        full_file_hash: [u8; 32],
    ) -> Self {
        Self {
            transfer_id,
            file_path,
            file_size,
            total_chunks,
            chunk_size,
            last_chunk_index: 0,
            last_chunk_offset: 0,
            partial_file_hash: [0u8; 32],
            full_file_hash,
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Update state after successful chunk transfer
    pub fn update_progress(&mut self, chunk_index: u32, chunk_offset: u64) {
        self.last_chunk_index = chunk_index;
        self.last_chunk_offset = chunk_offset;
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Calculate hash of partial file
    pub async fn calculate_partial_hash(&mut self) -> Result<()> {
        let mut file = File::open(&self.file_path)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to open file: {}", e)))?;

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192];
        let mut bytes_read_total = 0u64;

        while bytes_read_total < self.last_chunk_offset {
            let to_read = std::cmp::min(
                buffer.len(),
                (self.last_chunk_offset - bytes_read_total) as usize,
            );
            let bytes_read = file
                .read(&mut buffer[..to_read])
                .await
                .map_err(|e| Error::Transfer(format!("Failed to read file: {}", e)))?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
            bytes_read_total += bytes_read as u64;
        }

        self.partial_file_hash = hasher.finalize().into();
        Ok(())
    }

    /// Save state to disk
    pub async fn save(&self, state_dir: impl AsRef<Path>) -> Result<()> {
        let state_dir = state_dir.as_ref();
        tokio::fs::create_dir_all(state_dir)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to create state dir: {}", e)))?;

        let state_file = state_dir.join(format!("{}.state", self.transfer_id));
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Transfer(format!("Failed to serialize state: {}", e)))?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&state_file)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to create state file: {}", e)))?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| Error::Transfer(format!("Failed to write state: {}", e)))?;

        file.sync_all()
            .await
            .map_err(|e| Error::Transfer(format!("Failed to sync state: {}", e)))?;

        Ok(())
    }

    /// Load state from disk
    pub async fn load(state_dir: impl AsRef<Path>, transfer_id: Uuid) -> Result<Self> {
        let state_file = state_dir.as_ref().join(format!("{}.state", transfer_id));

        let mut file = File::open(&state_file)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to open state file: {}", e)))?;

        let mut json = String::new();
        file.read_to_string(&mut json)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to read state: {}", e)))?;

        let state: TransferState = serde_json::from_str(&json)
            .map_err(|e| Error::Transfer(format!("Failed to deserialize state: {}", e)))?;

        Ok(state)
    }

    /// Delete state file
    pub async fn delete(state_dir: impl AsRef<Path>, transfer_id: Uuid) -> Result<()> {
        let state_file = state_dir.as_ref().join(format!("{}.state", transfer_id));

        if state_file.exists() {
            tokio::fs::remove_file(&state_file)
                .await
                .map_err(|e| Error::Transfer(format!("Failed to delete state: {}", e)))?;
        }

        Ok(())
    }

    /// Check if transfer is complete
    pub fn is_complete(&self) -> bool {
        self.last_chunk_index >= self.total_chunks - 1
    }

    /// Get resume progress percentage
    pub fn progress_percentage(&self) -> f64 {
        if self.total_chunks == 0 {
            0.0
        } else {
            (self.last_chunk_index as f64 / self.total_chunks as f64) * 100.0
        }
    }
}

/// Resume manager for handling transfer state
pub struct ResumeManager {
    state_dir: PathBuf,
}

impl ResumeManager {
    /// Create a new resume manager
    pub fn new(state_dir: impl AsRef<Path>) -> Self {
        Self {
            state_dir: state_dir.as_ref().to_path_buf(),
        }
    }

    /// Save transfer state
    pub async fn save_state(&self, state: &TransferState) -> Result<()> {
        state.save(&self.state_dir).await
    }

    /// Load transfer state
    pub async fn load_state(&self, transfer_id: Uuid) -> Result<TransferState> {
        TransferState::load(&self.state_dir, transfer_id).await
    }

    /// Delete transfer state
    pub async fn delete_state(&self, transfer_id: Uuid) -> Result<()> {
        TransferState::delete(&self.state_dir, transfer_id).await
    }

    /// List all saved transfer states
    pub async fn list_states(&self) -> Result<Vec<TransferState>> {
        let mut states = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.state_dir)
            .await
            .map_err(|e| Error::Transfer(format!("Failed to read state dir: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::Transfer(format!("Failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("state") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(transfer_id) = Uuid::parse_str(file_stem) {
                        if let Ok(state) = TransferState::load(&self.state_dir, transfer_id).await {
                            states.push(state);
                        }
                    }
                }
            }
        }

        Ok(states)
    }

    /// Clean up old transfer states (older than specified days)
    pub async fn cleanup_old_states(&self, days: u64) -> Result<usize> {
        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (days * 24 * 60 * 60);

        let states = self.list_states().await?;
        let mut deleted = 0;

        for state in states {
            if state.last_update < cutoff_time {
                self.delete_state(state.transfer_id).await?;
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_transfer_state_save_load() {
        let temp_dir = tempdir().unwrap();
        let transfer_id = Uuid::new_v4();

        let mut state = TransferState::new(
            transfer_id,
            PathBuf::from("/tmp/test.bin"),
            1024000,
            250,
            4096,
            [0u8; 32],
        );

        state.update_progress(50, 204800);

        // Save state
        state.save(temp_dir.path()).await.unwrap();

        // Load state
        let loaded_state = TransferState::load(temp_dir.path(), transfer_id)
            .await
            .unwrap();

        assert_eq!(loaded_state.transfer_id, transfer_id);
        assert_eq!(loaded_state.last_chunk_index, 50);
        assert_eq!(loaded_state.last_chunk_offset, 204800);
    }

    #[tokio::test]
    async fn test_resume_manager() {
        let temp_dir = tempdir().unwrap();
        let manager = ResumeManager::new(temp_dir.path());

        let transfer_id = Uuid::new_v4();
        let state = TransferState::new(
            transfer_id,
            PathBuf::from("/tmp/test.bin"),
            1024000,
            250,
            4096,
            [0u8; 32],
        );

        // Save
        manager.save_state(&state).await.unwrap();

        // Load
        let loaded = manager.load_state(transfer_id).await.unwrap();
        assert_eq!(loaded.transfer_id, transfer_id);

        // List
        let states = manager.list_states().await.unwrap();
        assert_eq!(states.len(), 1);

        // Delete
        manager.delete_state(transfer_id).await.unwrap();
        let states = manager.list_states().await.unwrap();
        assert_eq!(states.len(), 0);
    }
}

// Made with Bob
