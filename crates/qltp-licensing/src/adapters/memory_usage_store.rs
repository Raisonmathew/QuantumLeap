//! In-memory usage store adapter
//!
//! Provides an in-memory implementation of UsageRepository for testing
//! and development purposes.

use crate::domain::usage::UsageRecord;
use crate::error::Result;
use crate::ports::UsageRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory usage store
#[derive(Clone)]
pub struct MemoryUsageStore {
    records: Arc<RwLock<HashMap<String, UsageRecord>>>,
}

impl MemoryUsageStore {
    /// Create a new in-memory usage store
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of records stored
    pub fn len(&self) -> usize {
        self.records.read().unwrap().len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.records.read().unwrap().is_empty()
    }

    /// Clear all records
    pub fn clear(&self) {
        self.records.write().unwrap().clear();
    }
}

impl Default for MemoryUsageStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UsageRepository for MemoryUsageStore {
    async fn save(&self, record: &UsageRecord) -> Result<()> {
        let mut records = self.records.write().unwrap();
        records.insert(record.id().to_string(), record.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<UsageRecord>> {
        let records = self.records.read().unwrap();
        Ok(records.get(id).cloned())
    }

    async fn find_by_license_id(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>> {
        let records = self.records.read().unwrap();
        
        let filtered: Vec<UsageRecord> = records
            .values()
            .filter(|r| {
                r.license_id().to_string() == license_id
                    && r.timestamp() >= start
                    && r.timestamp() <= end
            })
            .cloned()
            .collect();
        
        Ok(filtered)
    }

    async fn get_total_bytes(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<u64> {
        let records = self.find_by_license_id(license_id, start, end).await?;
        Ok(records.iter().map(|r| r.bytes()).sum())
    }

    async fn find_by_user(
        &self,
        _user_id: &str,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>> {
        // UsageRecord doesn't store user_id directly
        // This would need to be implemented by joining with license data
        // For now, return empty vec
        Ok(Vec::new())
    }

    async fn delete_before(&self, timestamp: DateTime<Utc>) -> Result<usize> {
        let mut records = self.records.write().unwrap();
        let initial_len = records.len();
        
        records.retain(|_, record| record.timestamp() >= timestamp);
        
        Ok(initial_len - records.len())
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<UsageRecord>> {
        let records = self.records.read().unwrap();
        
        let mut sorted: Vec<UsageRecord> = records.values().cloned().collect();
        sorted.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));
        sorted.truncate(limit);
        
        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::license::LicenseId;
    use crate::domain::usage::TransferType;
    use chrono::Duration;

    #[tokio::test]
    async fn test_save_and_find() {
        let store = MemoryUsageStore::new();
        let license_id = LicenseId::new();
        let record = UsageRecord::new(
            license_id,
            1024 * 1024, // 1 MB
            TransferType::Upload,
        );
        
        store.save(&record).await.unwrap();
        
        let found = store.find_by_id(record.id().to_string().as_str()).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), record.id());
    }

    #[tokio::test]
    async fn test_find_by_license_id() {
        let store = MemoryUsageStore::new();
        let license_id = LicenseId::new();
        
        let record1 = UsageRecord::new(
            license_id.clone(),
            1024,
            TransferType::Upload,
        );
        
        let record2 = UsageRecord::new(
            license_id.clone(),
            2048,
            TransferType::Download,
        );
        
        store.save(&record1).await.unwrap();
        store.save(&record2).await.unwrap();
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        let records = store.find_by_license_id(&license_id.to_string(), start, end).await.unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_get_total_bytes() {
        let store = MemoryUsageStore::new();
        let license_id = LicenseId::new();
        
        let record1 = UsageRecord::new(
            license_id.clone(),
            1024,
            TransferType::Upload,
        );
        
        let record2 = UsageRecord::new(
            license_id.clone(),
            2048,
            TransferType::Download,
        );
        
        store.save(&record1).await.unwrap();
        store.save(&record2).await.unwrap();
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        let total = store.get_total_bytes(&license_id.to_string(), start, end).await.unwrap();
        assert_eq!(total, 3072);
    }

    #[tokio::test]
    async fn test_find_by_user() {
        let store = MemoryUsageStore::new();
        let user_id = "user@example.com";
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        // This method is not fully implemented yet (needs license join)
        let records = store.find_by_user(user_id, start, end).await.unwrap();
        assert_eq!(records.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_before() {
        let store = MemoryUsageStore::new();
        let license_id = LicenseId::new();
        
        let record = UsageRecord::new(
            license_id,
            1024,
            TransferType::Upload,
        );
        
        store.save(&record).await.unwrap();
        assert_eq!(store.len(), 1);
        
        let cutoff = Utc::now() + Duration::hours(1);
        let deleted = store.delete_before(cutoff).await.unwrap();
        
        assert_eq!(deleted, 1);
        assert_eq!(store.len(), 0);
    }

    #[tokio::test]
    async fn test_get_recent() {
        let store = MemoryUsageStore::new();
        
        for _ in 0..5 {
            let license_id = LicenseId::new();
            let record = UsageRecord::new(
                license_id,
                1024,
                TransferType::Upload,
            );
            store.save(&record).await.unwrap();
        }
        
        let recent = store.get_recent(3).await.unwrap();
        assert_eq!(recent.len(), 3);
    }
}

// Made with Bob
