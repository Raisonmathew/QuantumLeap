//! Quota value object for usage limits

use crate::domain::license::LicenseTier;
use serde::{Deserialize, Serialize};

/// Quota limits for a license tier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quota {
    /// Monthly data transfer limit in bytes
    monthly_bytes: u64,
    /// Maximum file size in bytes
    max_file_size: u64,
    /// Maximum concurrent transfers
    max_concurrent: usize,
}

impl Quota {
    /// Create quota for a tier
    pub fn for_tier(tier: LicenseTier) -> Self {
        Self {
            monthly_bytes: tier.monthly_quota(),
            max_file_size: tier.max_file_size(),
            max_concurrent: tier.max_concurrent_transfers(),
        }
    }

    /// Create custom quota
    pub fn custom(monthly_bytes: u64, max_file_size: u64, max_concurrent: usize) -> Self {
        Self {
            monthly_bytes,
            max_file_size,
            max_concurrent,
        }
    }

    /// Get monthly bytes limit
    pub fn monthly_bytes(&self) -> u64 {
        self.monthly_bytes
    }

    /// Get max file size
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// Get max concurrent transfers
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Check if bytes amount is within monthly limit
    pub fn is_within_monthly_limit(&self, used_bytes: u64, additional_bytes: u64) -> bool {
        if self.monthly_bytes == u64::MAX {
            return true; // Unlimited
        }
        used_bytes + additional_bytes <= self.monthly_bytes
    }

    /// Check if file size is within limit
    pub fn is_file_size_allowed(&self, file_size: u64) -> bool {
        if self.max_file_size == u64::MAX {
            return true; // Unlimited
        }
        file_size <= self.max_file_size
    }

    /// Check if concurrent transfers are within limit
    pub fn is_concurrent_allowed(&self, current_count: usize) -> bool {
        if self.max_concurrent == usize::MAX {
            return true; // Unlimited
        }
        current_count < self.max_concurrent
    }

    /// Get remaining bytes for the month
    pub fn remaining_bytes(&self, used_bytes: u64) -> u64 {
        if self.monthly_bytes == u64::MAX {
            return u64::MAX; // Unlimited
        }
        self.monthly_bytes.saturating_sub(used_bytes)
    }

    /// Get usage percentage
    pub fn usage_percentage(&self, used_bytes: u64) -> f64 {
        if self.monthly_bytes == u64::MAX {
            return 0.0; // Unlimited
        }
        (used_bytes as f64 / self.monthly_bytes as f64) * 100.0
    }

    /// Check if quota is nearly exhausted (>90%)
    pub fn is_nearly_exhausted(&self, used_bytes: u64) -> bool {
        self.usage_percentage(used_bytes) > 90.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_for_tier() {
        let quota = Quota::for_tier(LicenseTier::Pro);
        assert_eq!(quota.monthly_bytes(), 100 * 1024 * 1024 * 1024);
        assert_eq!(quota.max_file_size(), 10 * 1024 * 1024 * 1024);
        assert_eq!(quota.max_concurrent(), 3);
    }

    #[test]
    fn test_quota_custom() {
        let quota = Quota::custom(1000, 100, 5);
        assert_eq!(quota.monthly_bytes(), 1000);
        assert_eq!(quota.max_file_size(), 100);
        assert_eq!(quota.max_concurrent(), 5);
    }

    #[test]
    fn test_within_monthly_limit() {
        let quota = Quota::custom(1000, 100, 5);
        assert!(quota.is_within_monthly_limit(500, 400));
        assert!(quota.is_within_monthly_limit(500, 500));
        assert!(!quota.is_within_monthly_limit(500, 501));
    }

    #[test]
    fn test_file_size_allowed() {
        let quota = Quota::custom(1000, 100, 5);
        assert!(quota.is_file_size_allowed(50));
        assert!(quota.is_file_size_allowed(100));
        assert!(!quota.is_file_size_allowed(101));
    }

    #[test]
    fn test_concurrent_allowed() {
        let quota = Quota::custom(1000, 100, 3);
        assert!(quota.is_concurrent_allowed(0));
        assert!(quota.is_concurrent_allowed(2));
        assert!(!quota.is_concurrent_allowed(3));
    }

    #[test]
    fn test_remaining_bytes() {
        let quota = Quota::custom(1000, 100, 5);
        assert_eq!(quota.remaining_bytes(300), 700);
        assert_eq!(quota.remaining_bytes(1000), 0);
        assert_eq!(quota.remaining_bytes(1100), 0); // Saturating sub
    }

    #[test]
    fn test_usage_percentage() {
        let quota = Quota::custom(1000, 100, 5);
        assert_eq!(quota.usage_percentage(500), 50.0);
        assert_eq!(quota.usage_percentage(900), 90.0);
    }

    #[test]
    fn test_nearly_exhausted() {
        let quota = Quota::custom(1000, 100, 5);
        assert!(!quota.is_nearly_exhausted(800));
        assert!(quota.is_nearly_exhausted(910));
    }

    #[test]
    fn test_unlimited_quota() {
        let quota = Quota::for_tier(LicenseTier::Enterprise);
        assert!(quota.is_within_monthly_limit(u64::MAX - 1, 1));
        assert!(quota.is_file_size_allowed(u64::MAX));
        assert_eq!(quota.remaining_bytes(1000), u64::MAX);
    }
}

// Made with Bob
