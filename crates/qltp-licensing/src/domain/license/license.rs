//! License aggregate root

use super::{
    device::{Device, DeviceId},
    feature_flags::{Feature, FeatureFlags},
    license_key::LicenseKey,
    license_tier::LicenseTier,
};
use crate::error::{LicenseError, Result};
use chrono::{DateTime, Utc};
use qltp_auth::AuthToken;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// License aggregate root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    /// Unique license identifier
    id: LicenseId,
    /// License key
    key: LicenseKey,
    /// License tier
    tier: LicenseTier,
    /// Feature flags
    features: FeatureFlags,
    /// User authentication token (optional, for registered users)
    user_token: Option<AuthToken>,
    /// Email address
    email: Option<String>,
    /// Activated devices
    devices: Vec<Device>,
    /// Creation timestamp
    created_at: DateTime<Utc>,
    /// Expiration timestamp (None for perpetual)
    expires_at: Option<DateTime<Utc>>,
    /// Last validation timestamp
    last_validated_at: DateTime<Utc>,
    /// Whether license is active
    is_active: bool,
}

impl License {
    /// Create a new license
    pub fn new(tier: LicenseTier, email: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: LicenseId::new(),
            key: LicenseKey::generate(tier),
            tier,
            features: FeatureFlags::for_tier(tier),
            user_token: None,
            email,
            devices: Vec::new(),
            created_at: now,
            expires_at: None, // Default to perpetual
            last_validated_at: now,
            is_active: true,
        }
    }

    /// Create a license with expiration
    pub fn new_with_expiration(
        tier: LicenseTier,
        email: Option<String>,
        expires_at: DateTime<Utc>,
    ) -> Self {
        let mut license = Self::new(tier, email);
        license.expires_at = Some(expires_at);
        license
    }

    /// Get license ID
    pub fn id(&self) -> &LicenseId {
        &self.id
    }

    /// Get license key
    pub fn key(&self) -> &LicenseKey {
        &self.key
    }

    /// Get license tier
    pub fn tier(&self) -> LicenseTier {
        self.tier
    }

    /// Get feature flags
    pub fn features(&self) -> &FeatureFlags {
        &self.features
    }

    /// Get user token
    pub fn user_token(&self) -> Option<&AuthToken> {
        self.user_token.as_ref()
    }

    /// Get email
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Get devices
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    /// Get active devices
    pub fn active_devices(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.is_active()).collect()
    }

    /// Get creation timestamp
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get expiration timestamp
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    /// Check if license is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Check if license is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Validate license
    pub fn validate(&mut self) -> Result<()> {
        if !self.is_active {
            return Err(LicenseError::LicenseNotFound);
        }

        if self.is_expired() {
            self.is_active = false;
            return Err(LicenseError::LicenseExpired);
        }

        self.last_validated_at = Utc::now();
        Ok(())
    }

    /// Check if a feature is available
    pub fn has_feature(&mut self, feature: Feature) -> Result<()> {
        self.validate()?;

        if self.features.has_feature(feature) {
            Ok(())
        } else {
            Err(LicenseError::FeatureNotAvailable {
                tier: self.tier.to_string(),
            })
        }
    }

    /// Link license to user
    pub fn link_user(&mut self, token: AuthToken) {
        self.user_token = Some(token);
    }

    /// Unlink user
    pub fn unlink_user(&mut self) {
        self.user_token = None;
    }

    /// Activate device
    pub fn activate_device(&mut self, device: Device) -> Result<()> {
        self.validate()?;

        // Check device limit
        let active_count = self.active_devices().len();
        let max_devices = self.tier.max_devices();

        if active_count >= max_devices {
            return Err(LicenseError::DeviceLimitExceeded { max: max_devices });
        }

        // Check if device already exists
        if let Some(existing) = self.devices.iter().find(|d| d.fingerprint() == device.fingerprint()) {
            if existing.is_active() {
                return Err(LicenseError::AlreadyActivated {
                    device_id: existing.id().to_string(),
                });
            }
        }

        self.devices.push(device);
        Ok(())
    }

    /// Deactivate device
    pub fn deactivate_device(&mut self, device_id: &DeviceId) -> Result<()> {
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.id() == device_id)
            .ok_or(LicenseError::LicenseNotFound)?;

        device.deactivate();
        Ok(())
    }

    /// Reactivate device
    pub fn reactivate_device(&mut self, device_id: &DeviceId) -> Result<()> {
        self.validate()?;

        // Check device limit first
        let active_count = self.active_devices().len();
        let max_devices = self.tier.max_devices();

        if active_count >= max_devices {
            return Err(LicenseError::DeviceLimitExceeded { max: max_devices });
        }

        // Then reactivate the device
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.id() == device_id)
            .ok_or(LicenseError::LicenseNotFound)?;

        device.reactivate();
        Ok(())
    }

    /// Update device last seen
    pub fn update_device_last_seen(&mut self, device_id: &DeviceId) -> Result<()> {
        let device = self
            .devices
            .iter_mut()
            .find(|d| d.id() == device_id)
            .ok_or(LicenseError::LicenseNotFound)?;

        device.update_last_seen();
        Ok(())
    }

    /// Upgrade tier
    pub fn upgrade_tier(&mut self, new_tier: LicenseTier) -> Result<()> {
        self.validate()?;
        self.tier = new_tier;
        self.features = FeatureFlags::for_tier(new_tier);
        // Note: We keep the same license key when upgrading
        Ok(())
    }

    /// Deactivate license
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Reactivate license
    pub fn reactivate(&mut self) -> Result<()> {
        if self.is_expired() {
            return Err(LicenseError::LicenseExpired);
        }
        self.is_active = true;
        Ok(())
    }

    /// Get days until expiration
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expires_at.map(|exp| {
            let now = Utc::now();
            (exp - now).num_days()
        })
    }

    /// Check if license is expiring soon (within 30 days)
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(days) = self.days_until_expiration() {
            days > 0 && days <= 30
        } else {
            false
        }
    }
}

/// License identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LicenseId(Uuid);

impl LicenseId {
    /// Create a new license ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get as UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get as string
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for LicenseId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LicenseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_license_creation() {
        let license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        assert_eq!(license.tier(), LicenseTier::Pro);
        assert_eq!(license.email(), Some("user@example.com"));
        assert!(license.is_active());
        assert!(!license.is_expired());
    }

    #[test]
    fn test_license_with_expiration() {
        let expires_at = Utc::now() + Duration::days(30);
        let license = License::new_with_expiration(
            LicenseTier::Team,
            None,
            expires_at,
        );

        assert!(!license.is_expired());
        // Allow for timing variations (29-30 days)
        let days = license.days_until_expiration().unwrap();
        assert!(days >= 29 && days <= 30, "Expected 29-30 days, got {}", days);
    }

    #[test]
    fn test_license_validation() {
        let mut license = License::new(LicenseTier::Free, None);
        assert!(license.validate().is_ok());

        license.deactivate();
        assert!(license.validate().is_err());
    }

    #[test]
    fn test_feature_check() {
        let mut license = License::new(LicenseTier::Pro, None);
        
        assert!(license.has_feature(Feature::Compression).is_ok());
        assert!(license.has_feature(Feature::Encryption).is_ok());
        assert!(license.has_feature(Feature::ParallelTransfers).is_err());
    }

    #[test]
    fn test_device_activation() {
        let mut license = License::new(LicenseTier::Pro, None);
        
        let device = Device::new(
            "Laptop".to_string(),
            "fp123".to_string(),
            "Linux".to_string(),
            "laptop-01".to_string(),
        );

        assert!(license.activate_device(device).is_ok());
        assert_eq!(license.active_devices().len(), 1);
    }

    #[test]
    fn test_device_limit() {
        let mut license = License::new(LicenseTier::Free, None); // Max 1 device

        let device1 = Device::new(
            "Device 1".to_string(),
            "fp1".to_string(),
            "Linux".to_string(),
            "host1".to_string(),
        );

        let device2 = Device::new(
            "Device 2".to_string(),
            "fp2".to_string(),
            "Linux".to_string(),
            "host2".to_string(),
        );

        assert!(license.activate_device(device1).is_ok());
        assert!(license.activate_device(device2).is_err());
    }

    #[test]
    fn test_tier_upgrade() {
        let mut license = License::new(LicenseTier::Free, None);
        
        assert_eq!(license.tier(), LicenseTier::Free);
        assert!(license.upgrade_tier(LicenseTier::Pro).is_ok());
        assert_eq!(license.tier(), LicenseTier::Pro);
    }

    #[test]
    fn test_user_linking() {
        let mut license = License::new(LicenseTier::Team, None);
        let token = AuthToken::new();

        assert!(license.user_token().is_none());
        license.link_user(token.clone());
        assert!(license.user_token().is_some());
        
        license.unlink_user();
        assert!(license.user_token().is_none());
    }

    #[test]
    fn test_expiring_soon() {
        let expires_at = Utc::now() + Duration::days(15);
        let license = License::new_with_expiration(
            LicenseTier::Pro,
            None,
            expires_at,
        );

        assert!(license.is_expiring_soon());
    }
}

// Made with Bob
