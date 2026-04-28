//! SQLite-based usage repository implementation

use crate::domain::license::LicenseId;
use crate::domain::usage::{UsageRecord, UsageRecordId};
use crate::error::Result;
use crate::ports::UsageRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// SQLite-based usage storage
pub struct SqliteUsageStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteUsageStore {
    /// Create a new SQLite usage store
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_records (
                id TEXT PRIMARY KEY,
                license_id TEXT NOT NULL,
                bytes INTEGER NOT NULL,
                transfer_type TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;

        // Create indexes for efficient queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_license ON usage_records(license_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_timestamp ON usage_records(timestamp)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_usage_license_timestamp 
             ON usage_records(license_id, timestamp)",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> Result<Self> {
        Self::new(":memory:")
    }
}

#[async_trait]
impl UsageRepository for SqliteUsageStore {
    async fn save(&self, record: &UsageRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        conn.execute(
            "INSERT INTO usage_records (id, license_id, bytes, transfer_type, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.id().as_str(),
                record.license_id().as_str(),
                record.bytes() as i64,
                format!("{:?}", record.transfer_type()),
                record.timestamp().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<UsageRecord>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        let result = conn
            .query_row(
                "SELECT id, license_id, bytes, transfer_type, timestamp
                 FROM usage_records WHERE id = ?1",
                params![id],
                |row| {
                    let id_str: String = row.get(0)?;
                    let license_id_str: String = row.get(1)?;
                    let bytes: i64 = row.get(2)?;
                    let transfer_type_str: String = row.get(3)?;
                    let timestamp_str: String = row.get(4)?;
                    
                    Ok((id_str, license_id_str, bytes, transfer_type_str, timestamp_str))
                },
            )
            .optional()?;

        match result {
            Some((id_str, license_id_str, bytes, transfer_type_str, timestamp_str)) => {
                let _id = UsageRecordId::from_uuid(Uuid::parse_str(&id_str).unwrap());
                let license_id = LicenseId::from_uuid(Uuid::parse_str(&license_id_str).unwrap());
                let _timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                    .unwrap()
                    .with_timezone(&Utc);
                
                let transfer_type = match transfer_type_str.as_str() {
                    "Upload" => crate::domain::usage::TransferType::Upload,
                    "Download" => crate::domain::usage::TransferType::Download,
                    _ => crate::domain::usage::TransferType::Upload,
                };
                
                let record = UsageRecord::new(
                    license_id,
                    bytes as u64,
                    transfer_type,
                );
                
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    async fn find_by_license_id(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        let mut stmt = conn.prepare(
            "SELECT id, license_id, bytes, transfer_type, timestamp
             FROM usage_records
             WHERE license_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             ORDER BY timestamp DESC",
        )?;

        let rows = stmt.query_map(
            params![
                license_id,
                start.to_rfc3339(),
                end.to_rfc3339(),
            ],
            |row| {
                let id_str: String = row.get(0)?;
                let license_id_str: String = row.get(1)?;
                let bytes: i64 = row.get(2)?;
                let transfer_type_str: String = row.get(3)?;
                let timestamp_str: String = row.get(4)?;
                
                Ok((id_str, license_id_str, bytes, transfer_type_str, timestamp_str))
            },
        )?;

        let mut records = Vec::new();
        for row in rows {
            let (_id_str, license_id_str, bytes, transfer_type_str, _timestamp_str) = row?;
            
            let license_id = LicenseId::from_uuid(Uuid::parse_str(&license_id_str).unwrap());
            
            let transfer_type = match transfer_type_str.as_str() {
                "Upload" => crate::domain::usage::TransferType::Upload,
                "Download" => crate::domain::usage::TransferType::Download,
                _ => crate::domain::usage::TransferType::Upload,
            };
            
            let record = UsageRecord::new(
                license_id,
                bytes as u64,
                transfer_type,
            );
            
            records.push(record);
        }

        Ok(records)
    }

    async fn get_total_bytes(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<u64> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        let total: Option<i64> = conn.query_row(
            "SELECT SUM(bytes) FROM usage_records 
             WHERE license_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3",
            params![
                license_id,
                start.to_rfc3339(),
                end.to_rfc3339(),
            ],
            |row| row.get(0),
        )?;

        Ok(total.unwrap_or(0) as u64)
    }

    async fn find_by_user(
        &self,
        _user_id: &str,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>> {
        // For now, return empty since we don't have user_id in the domain model
        // This method would need to be implemented when user tracking is added
        Ok(Vec::new())
    }

    async fn delete_before(&self, timestamp: DateTime<Utc>) -> Result<usize> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        let rows_affected = conn.execute(
            "DELETE FROM usage_records WHERE timestamp < ?1",
            params![timestamp.to_rfc3339()],
        )?;

        Ok(rows_affected)
    }

    async fn get_recent(&self, limit: usize) -> Result<Vec<UsageRecord>> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        
        let mut stmt = conn.prepare(
            "SELECT id, license_id, bytes, transfer_type, timestamp
             FROM usage_records
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(
            params![limit as i64],
            |row| {
                let id_str: String = row.get(0)?;
                let license_id_str: String = row.get(1)?;
                let bytes: i64 = row.get(2)?;
                let transfer_type_str: String = row.get(3)?;
                let timestamp_str: String = row.get(4)?;
                
                Ok((id_str, license_id_str, bytes, transfer_type_str, timestamp_str))
            },
        )?;

        let mut records = Vec::new();
        for row in rows {
            let (_id_str, license_id_str, bytes, transfer_type_str, _timestamp_str) = row?;
            
            let license_id = LicenseId::from_uuid(Uuid::parse_str(&license_id_str).unwrap());
            
            let transfer_type = match transfer_type_str.as_str() {
                "Upload" => crate::domain::usage::TransferType::Upload,
                "Download" => crate::domain::usage::TransferType::Download,
                _ => crate::domain::usage::TransferType::Upload,
            };
            
            let record = UsageRecord::new(
                license_id,
                bytes as u64,
                transfer_type,
            );
            
            records.push(record);
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::license::License;
    use crate::domain::license::LicenseTier;
    use crate::domain::usage::TransferType;
    use chrono::Duration;

    #[tokio::test]
    async fn test_save_and_find() {
        let store = SqliteUsageStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, None);
        let record = UsageRecord::new(license.id().clone(), 1024, TransferType::Upload);
        
        store.save(&record).await.unwrap();
        
        let found = store.find_by_id(&record.id().as_str()).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().bytes(), 1024);
    }

    #[tokio::test]
    async fn test_find_by_license_id() {
        let store = SqliteUsageStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, None);
        
        let record1 = UsageRecord::new(license.id().clone(), 1024, TransferType::Upload);
        let record2 = UsageRecord::new(license.id().clone(), 2048, TransferType::Download);
        
        store.save(&record1).await.unwrap();
        store.save(&record2).await.unwrap();
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        let records = store.find_by_license_id(&license.id().as_str(), start, end).await.unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_get_total_bytes() {
        let store = SqliteUsageStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, None);
        
        let record1 = UsageRecord::new(license.id().clone(), 1024, TransferType::Upload);
        let record2 = UsageRecord::new(license.id().clone(), 2048, TransferType::Download);
        
        store.save(&record1).await.unwrap();
        store.save(&record2).await.unwrap();
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        let total = store.get_total_bytes(&license.id().as_str(), start, end).await.unwrap();
        assert_eq!(total, 3072);
    }

    #[tokio::test]
    async fn test_delete_before() {
        let store = SqliteUsageStore::in_memory().unwrap();
        let license = License::new(LicenseTier::Pro, None);
        
        let record = UsageRecord::new(license.id().clone(), 1024, TransferType::Upload);
        store.save(&record).await.unwrap();
        
        // Delete records older than now + 1 hour (should delete the record)
        let deleted = store.delete_before(Utc::now() + Duration::hours(1)).await.unwrap();
        assert_eq!(deleted, 1);
        
        let found = store.find_by_id(&record.id().as_str()).await.unwrap();
        assert!(found.is_none());
    }
}

// Made with Bob
