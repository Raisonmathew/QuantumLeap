//! Transfer validation middleware for license enforcement

use crate::application::{LicenseService, UsageTracker};
use crate::domain::license::{Feature, License};
use crate::domain::usage::TransferType;
use crate::error::LicenseError;
use qltp_auth::AuthToken;
use std::sync::Arc;
use tracing::{debug, warn};

/// Context for transfer validation
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// User's authentication token
    pub auth_token: AuthToken,
    /// License key to validate against
    pub license_key: String,
    /// File size in bytes
    pub file_size: u64,
    /// Transfer type (upload/download)
    pub transfer_type: TransferType,
    /// Whether compression is requested
    pub use_compression: bool,
    /// Whether encryption is requested
    pub use_encryption: bool,
    /// Whether parallel transfers are requested
    pub use_parallel: bool,
    /// Whether QUIC protocol is requested
    pub use_quic: bool,
}

/// Transfer validation errors
#[derive(Debug, thiserror::Error)]
pub enum TransferValidationError {
    #[error("License validation failed: {0}")]
    LicenseError(#[from] LicenseError),

    #[error("Feature not available: {feature}. Upgrade to {required_tier} or higher")]
    FeatureNotAvailable {
        feature: String,
        required_tier: String,
    },

    #[error("File size {size} bytes exceeds quota limit of {limit} bytes")]
    FileSizeLimitExceeded { size: u64, limit: u64 },

    #[error("Monthly quota exceeded: {used} / {limit} bytes used")]
    MonthlyQuotaExceeded { used: u64, limit: u64 },

    #[error("Concurrent transfer limit reached: {current} / {limit}")]
    ConcurrentLimitReached { current: u32, limit: u32 },

    #[error("License has expired")]
    LicenseExpired,

    #[error("Device not activated for this license")]
    DeviceNotActivated,
}

/// Transfer validator for license enforcement
pub struct TransferValidator {
    license_service: Arc<LicenseService>,
    usage_tracker: Arc<UsageTracker>,
}

impl TransferValidator {
    /// Create a new transfer validator
    pub fn new(license_service: Arc<LicenseService>, usage_tracker: Arc<UsageTracker>) -> Self {
        Self {
            license_service,
            usage_tracker,
        }
    }

    /// Validate a transfer request before execution
    pub async fn validate_transfer(
        &self,
        context: &ValidationContext,
    ) -> Result<(), TransferValidationError> {
        debug!(
            "Validating transfer: license_key={}, file_size={}, type={:?}",
            context.license_key, context.file_size, context.transfer_type
        );

        // 1. Get and validate license
        let license = self
            .license_service
            .get_license(&context.license_key)
            .await?;

        // 2. Check if license is expired
        if license.is_expired() {
            warn!("License {} has expired", license.id());
            return Err(TransferValidationError::LicenseExpired);
        }

        // 3. Validate required features
        self.validate_features(&license, context)?;

        // 4. Check file size quota
        let quota = crate::domain::usage::Quota::for_tier(license.tier());
        if !quota.is_file_size_allowed(context.file_size) {
            warn!(
                "File size {} exceeds limit {}",
                context.file_size,
                quota.max_file_size()
            );
            return Err(TransferValidationError::FileSizeLimitExceeded {
                size: context.file_size,
                limit: quota.max_file_size(),
            });
        }

        // 5. Check monthly quota
        let current_usage = self
            .usage_tracker
            .get_current_month_usage(license.id())
            .await?;

        let monthly_limit = quota.monthly_bytes();
        if current_usage + context.file_size > monthly_limit {
            warn!(
                "Monthly quota would be exceeded: {} + {} > {}",
                current_usage, context.file_size, monthly_limit
            );
            return Err(TransferValidationError::MonthlyQuotaExceeded {
                used: current_usage,
                limit: monthly_limit,
            });
        }

        // 6. Check concurrent transfer limit
        // Note: This would require tracking active transfers, which we'll implement later
        // For now, we'll skip this check

        debug!("Transfer validation passed for license {}", license.id());
        Ok(())
    }

    /// Validate that required features are available in the license
    fn validate_features(
        &self,
        license: &License,
        context: &ValidationContext,
    ) -> Result<(), TransferValidationError> {
        let features = license.features();

        // Check compression feature
        if context.use_compression && !features.has_feature(Feature::Compression) {
            return Err(TransferValidationError::FeatureNotAvailable {
                feature: "Compression".to_string(),
                required_tier: "Pro".to_string(),
            });
        }

        // Check encryption feature
        if context.use_encryption && !features.has_feature(Feature::Encryption) {
            return Err(TransferValidationError::FeatureNotAvailable {
                feature: "Encryption".to_string(),
                required_tier: "Pro".to_string(),
            });
        }

        // Check parallel transfers feature
        if context.use_parallel && !features.has_feature(Feature::ParallelTransfers) {
            return Err(TransferValidationError::FeatureNotAvailable {
                feature: "Parallel Transfers".to_string(),
                required_tier: "Team".to_string(),
            });
        }

        // Check QUIC protocol feature
        if context.use_quic && !features.has_feature(Feature::Quic) {
            return Err(TransferValidationError::FeatureNotAvailable {
                feature: "QUIC Protocol".to_string(),
                required_tier: "Team".to_string(),
            });
        }

        Ok(())
    }

    /// Record a completed transfer for usage tracking
    pub async fn record_transfer(
        &self,
        license_key: &str,
        bytes_transferred: u64,
        transfer_type: TransferType,
    ) -> Result<(), TransferValidationError> {
        let license = self.license_service.get_license(license_key).await?;
        
        debug!(
            "Recording transfer: license={}, bytes={}",
            license.id(), bytes_transferred
        );

        self.usage_tracker
            .record_transfer(
                license.id().clone(),
                bytes_transferred,
                transfer_type,
            )
            .await?;

        Ok(())
    }

    /// Get remaining quota for a license
    pub async fn get_remaining_quota(
        &self,
        license_key: &str,
    ) -> Result<u64, TransferValidationError> {
        let license = self
            .license_service
            .get_license(license_key)
            .await?;

        let quota = crate::domain::usage::Quota::for_tier(license.tier());
        let current_usage = self
            .usage_tracker
            .get_current_month_usage(license.id())
            .await?;

        let remaining = quota.monthly_bytes().saturating_sub(current_usage);
        Ok(remaining)
    }

    /// Check if a feature is available for a license
    pub async fn is_feature_available(
        &self,
        license_key: &str,
        feature: Feature,
    ) -> Result<bool, TransferValidationError> {
        let license = self
            .license_service
            .get_license(license_key)
            .await?;

        Ok(license.features().has_feature(feature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryLicenseStore, MemoryUsageStore};
    use crate::domain::license::{Device, LicenseKey, LicenseTier};
    use std::sync::Arc;

    async fn setup() -> (TransferValidator, String) {
        let license_store = Arc::new(MemoryLicenseStore::new());
        let usage_store = Arc::new(MemoryUsageStore::new());
        let license_service = Arc::new(LicenseService::new(license_store.clone()));
        let usage_tracker = Arc::new(UsageTracker::new(license_store.clone(), usage_store));

        // Create a Pro license for testing
        let license = license_service
            .create_license(LicenseTier::Pro, Some("test@example.com".to_string()))
            .await
            .unwrap();

        let license_key = license.key().to_string();

        let validator = TransferValidator::new(license_service, usage_tracker);

        (validator, license_key)
    }

    #[tokio::test]
    async fn test_validate_transfer_success() {
        let (validator, license_key) = setup().await;

        let context = ValidationContext {
            auth_token: AuthToken::new(),
            license_key,
            file_size: 1024 * 1024, // 1 MB
            transfer_type: TransferType::Upload,
            use_compression: true,
            use_encryption: true,
            use_parallel: false,
            use_quic: false,
        };

        let result = validator.validate_transfer(&context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_transfer_file_size_exceeded() {
        let (validator, license_key) = setup().await;

        let context = ValidationContext {
            auth_token: AuthToken::new(),
            license_key,
            file_size: 11 * 1024 * 1024 * 1024, // 11 GB (exceeds Pro limit of 10 GB)
            transfer_type: TransferType::Upload,
            use_compression: false,
            use_encryption: false,
            use_parallel: false,
            use_quic: false,
        };

        let result = validator.validate_transfer(&context).await;
        assert!(matches!(
            result,
            Err(TransferValidationError::FileSizeLimitExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn test_validate_transfer_feature_not_available() {
        let (validator, license_key) = setup().await;

        // Pro tier doesn't have parallel transfers
        let context = ValidationContext {
            auth_token: AuthToken::new(),
            license_key,
            file_size: 1024 * 1024,
            transfer_type: TransferType::Upload,
            use_compression: true,
            use_encryption: true,
            use_parallel: true, // Not available in Pro
            use_quic: false,
        };

        let result = validator.validate_transfer(&context).await;
        assert!(matches!(
            result,
            Err(TransferValidationError::FeatureNotAvailable { .. })
        ));
    }

    #[tokio::test]
    async fn test_record_transfer() {
        let (validator, license_key) = setup().await;

        let result = validator.record_transfer(&license_key, 1024 * 1024, TransferType::Upload).await;
        assert!(result.is_ok());

        // Verify usage was recorded
        let remaining = validator.get_remaining_quota(&license_key).await.unwrap();
        assert_eq!(remaining, 100 * 1024 * 1024 * 1024 - 1024 * 1024); // 100 GB - 1 MB
    }

    #[tokio::test]
    async fn test_get_remaining_quota() {
        let (validator, license_key) = setup().await;

        let remaining = validator.get_remaining_quota(&license_key).await.unwrap();
        assert_eq!(remaining, 100 * 1024 * 1024 * 1024); // 100 GB for Pro tier
    }

    #[tokio::test]
    async fn test_is_feature_available() {
        let (validator, license_key) = setup().await;

        // Pro tier has compression
        let has_compression = validator
            .is_feature_available(&license_key, Feature::Compression)
            .await
            .unwrap();
        assert!(has_compression);

        // Pro tier doesn't have parallel transfers
        let has_parallel = validator
            .is_feature_available(&license_key, Feature::ParallelTransfers)
            .await
            .unwrap();
        assert!(!has_parallel);
    }

    #[tokio::test]
    async fn test_monthly_quota_exceeded() {
        let (validator, license_key) = setup().await;

        // Record usage close to limit
        validator
            .record_transfer(&license_key, 99 * 1024 * 1024 * 1024, TransferType::Upload)
            .await
            .unwrap();

        // Try to transfer 2 GB more (would exceed 100 GB limit)
        let context = ValidationContext {
            auth_token: AuthToken::new(),
            license_key,
            file_size: 2 * 1024 * 1024 * 1024,
            transfer_type: TransferType::Upload,
            use_compression: false,
            use_encryption: false,
            use_parallel: false,
            use_quic: false,
        };

        let result = validator.validate_transfer(&context).await;
        assert!(matches!(
            result,
            Err(TransferValidationError::MonthlyQuotaExceeded { .. })
        ));
    }
}

// Made with Bob
