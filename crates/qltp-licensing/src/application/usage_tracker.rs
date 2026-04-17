//! Usage tracker service - Application layer
//!
//! Tracks and enforces usage quotas for licenses

use crate::domain::license::LicenseId;
use crate::domain::usage::{Quota, TransferType, UsageRecord};
use crate::error::{LicenseError, Result};
use crate::ports::{LicenseRepository, UsageRepository};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;

/// Usage tracker service
pub struct UsageTracker {
    license_repo: Arc<dyn LicenseRepository>,
    usage_repo: Arc<dyn UsageRepository>,
}

impl UsageTracker {
    /// Create a new usage tracker
    pub fn new(
        license_repo: Arc<dyn LicenseRepository>,
        usage_repo: Arc<dyn UsageRepository>,
    ) -> Self {
        Self {
            license_repo,
            usage_repo,
        }
    }

    /// Record a transfer
    pub async fn record_transfer(
        &self,
        license_id: LicenseId,
        bytes: u64,
        transfer_type: TransferType,
    ) -> Result<UsageRecord> {
        let record = UsageRecord::new(license_id, bytes, transfer_type);
        self.usage_repo.save(&record).await?;
        Ok(record)
    }

    /// Check if transfer is allowed under quota
    pub async fn check_quota(
        &self,
        license_id: &LicenseId,
        bytes: u64,
    ) -> Result<()> {
        // Get license to determine tier
        let license = self.license_repo
            .find_by_id(license_id)
            .await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        let quota = Quota::for_tier(license.tier());
        
        // Check file size limit
        if !quota.is_file_size_allowed(bytes) {
            return Err(LicenseError::QuotaExceeded {
                message: format!(
                    "File size {} exceeds limit of {}",
                    bytes,
                    quota.max_file_size()
                ),
            });
        }
        
        // Check monthly limit
        let monthly_limit = quota.monthly_bytes();
        if monthly_limit != u64::MAX {
            let start = Utc::now() - Duration::days(30);
            let end = Utc::now();
            
            let total_bytes = self.usage_repo
                .get_total_bytes(&license_id.to_string(), start, end)
                .await?;
            
            if !quota.is_within_monthly_limit(total_bytes, bytes) {
                return Err(LicenseError::QuotaExceeded {
                    message: format!(
                        "Monthly quota exceeded: {} / {} bytes used",
                        total_bytes,
                        monthly_limit
                    ),
                });
            }
        }
        
        Ok(())
    }

    /// Get usage statistics for a license
    pub async fn get_usage_stats(
        &self,
        license_id: &LicenseId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<UsageStats> {
        let records = self.usage_repo
            .find_by_license_id(&license_id.to_string(), start, end)
            .await?;
        
        let total_bytes: u64 = records.iter().map(|r| r.bytes()).sum();
        let upload_bytes: u64 = records
            .iter()
            .filter(|r| r.transfer_type() == TransferType::Upload)
            .map(|r| r.bytes())
            .sum();
        let download_bytes: u64 = records
            .iter()
            .filter(|r| r.transfer_type() == TransferType::Download)
            .map(|r| r.bytes())
            .sum();
        
        Ok(UsageStats {
            total_bytes,
            upload_bytes,
            download_bytes,
            transfer_count: records.len(),
            period_start: start,
            period_end: end,
        })
    }

    /// Get current month usage
    pub async fn get_current_month_usage(
        &self,
        license_id: &LicenseId,
    ) -> Result<u64> {
        let start = Utc::now() - Duration::days(30);
        let end = Utc::now();
        
        self.usage_repo
            .get_total_bytes(&license_id.to_string(), start, end)
            .await
    }

    /// Get remaining quota
    pub async fn get_remaining_quota(
        &self,
        license_id: &LicenseId,
    ) -> Result<Option<u64>> {
        // Get license to determine tier
        let license = self.license_repo
            .find_by_id(license_id)
            .await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        let quota = Quota::for_tier(license.tier());
        let monthly_limit = quota.monthly_bytes();
        
        if monthly_limit != u64::MAX {
            let used = self.get_current_month_usage(license_id).await?;
            Ok(Some(quota.remaining_bytes(used)))
        } else {
            Ok(None) // Unlimited
        }
    }

    /// Clean up old usage records
    pub async fn cleanup_old_records(
        &self,
        before: DateTime<Utc>,
    ) -> Result<usize> {
        self.usage_repo.delete_before(before).await
    }
}

/// Usage statistics
#[derive(Debug, Clone)]
pub struct UsageStats {
    pub total_bytes: u64,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub transfer_count: usize,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

impl UsageStats {
    /// Get human-readable total size
    pub fn total_size_human(&self) -> String {
        human_readable_size(self.total_bytes)
    }

    /// Get human-readable upload size
    pub fn upload_size_human(&self) -> String {
        human_readable_size(self.upload_bytes)
    }

    /// Get human-readable download size
    pub fn download_size_human(&self) -> String {
        human_readable_size(self.download_bytes)
    }
}

fn human_readable_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryLicenseStore, MemoryUsageStore};
    use crate::domain::license::{License, LicenseTier};

    #[tokio::test]
    async fn test_record_transfer() {
        let license_repo = Arc::new(MemoryLicenseStore::new());
        let usage_repo = Arc::new(MemoryUsageStore::new());
        let tracker = UsageTracker::new(license_repo, usage_repo);
        
        let license_id = LicenseId::new();
        let record = tracker.record_transfer(
            license_id,
            1024 * 1024,
            TransferType::Upload
        ).await.unwrap();
        
        assert_eq!(record.bytes(), 1024 * 1024);
        assert_eq!(record.transfer_type(), TransferType::Upload);
    }

    #[tokio::test]
    async fn test_get_usage_stats() {
        let license_repo = Arc::new(MemoryLicenseStore::new());
        let usage_repo = Arc::new(MemoryUsageStore::new());
        let tracker = UsageTracker::new(license_repo, usage_repo);
        
        let license_id = LicenseId::new();
        
        // Record some transfers
        tracker.record_transfer(license_id.clone(), 1024, TransferType::Upload).await.unwrap();
        tracker.record_transfer(license_id.clone(), 2048, TransferType::Download).await.unwrap();
        
        let start = Utc::now() - Duration::hours(1);
        let end = Utc::now() + Duration::hours(1);
        
        let stats = tracker.get_usage_stats(&license_id, start, end).await.unwrap();
        
        assert_eq!(stats.total_bytes, 3072);
        assert_eq!(stats.upload_bytes, 1024);
        assert_eq!(stats.download_bytes, 2048);
        assert_eq!(stats.transfer_count, 2);
    }

    #[tokio::test]
    async fn test_usage_stats_human_readable() {
        let stats = UsageStats {
            total_bytes: 1024 * 1024 * 1024, // 1 GB
            upload_bytes: 512 * 1024 * 1024, // 512 MB
            download_bytes: 512 * 1024 * 1024, // 512 MB
            transfer_count: 10,
            period_start: Utc::now() - Duration::days(30),
            period_end: Utc::now(),
        };
        
        assert_eq!(stats.total_size_human(), "1.00 GB");
        assert_eq!(stats.upload_size_human(), "512.00 MB");
        assert_eq!(stats.download_size_human(), "512.00 MB");
    }

    #[tokio::test]
    async fn test_cleanup_old_records() {
        let license_repo = Arc::new(MemoryLicenseStore::new());
        let usage_repo = Arc::new(MemoryUsageStore::new());
        let tracker = UsageTracker::new(license_repo, usage_repo.clone());
        
        let license_id = LicenseId::new();
        tracker.record_transfer(license_id, 1024, TransferType::Upload).await.unwrap();
        
        assert_eq!(usage_repo.len(), 1);
        
        let cutoff = Utc::now() + Duration::hours(1);
        let deleted = tracker.cleanup_old_records(cutoff).await.unwrap();
        
        assert_eq!(deleted, 1);
        assert_eq!(usage_repo.len(), 0);
    }
}

// Made with Bob
