//! Usage repository port
//!
//! Defines the interface for usage tracking persistence

use crate::domain::usage::UsageRecord;
use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Repository for usage tracking persistence
#[async_trait]
pub trait UsageRepository: Send + Sync {
    /// Save a usage record
    async fn save(&self, record: &UsageRecord) -> Result<()>;

    /// Find usage record by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<UsageRecord>>;

    /// Get usage records for a license ID within a time range
    async fn find_by_license_id(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>>;

    /// Get total bytes transferred for a license ID within a time range
    async fn get_total_bytes(
        &self,
        license_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<u64>;

    /// Get usage records for a user within a time range
    async fn find_by_user(
        &self,
        user_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<UsageRecord>>;

    /// Delete old usage records (for cleanup)
    async fn delete_before(&self, timestamp: DateTime<Utc>) -> Result<usize>;

    /// Get recent usage records (for monitoring)
    async fn get_recent(&self, limit: usize) -> Result<Vec<UsageRecord>>;
}

// Made with Bob
