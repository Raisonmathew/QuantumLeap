//! License service - Application layer
//!
//! Orchestrates license operations using domain entities and repository ports

use crate::domain::license::{Device, DeviceId, License, LicenseKey, LicenseTier};
use crate::domain::usage::Quota;
use crate::error::{LicenseError, Result};
use crate::ports::LicenseRepository;
use qltp_auth::AuthToken;
use std::str::FromStr;
use std::sync::Arc;

/// License service for managing licenses
pub struct LicenseService {
    repository: Arc<dyn LicenseRepository>,
}

impl LicenseService {
    /// Create a new license service
    pub fn new(repository: Arc<dyn LicenseRepository>) -> Self {
        Self { repository }
    }

    /// Create a new license
    pub async fn create_license(
        &self,
        tier: LicenseTier,
        email: Option<String>,
    ) -> Result<License> {
        let license = License::new(tier, email);
        self.repository.save(&license).await?;
        Ok(license)
    }

    /// Activate a license with a key
    pub async fn activate_license(&self, key: &str) -> Result<License> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        Ok(license)
    }

    /// Activate a device for a license
    pub async fn activate_device(
        &self,
        key: &str,
        device_name: String,
        fingerprint: String,
    ) -> Result<()> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let mut license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        // Get system information
        let os = std::env::consts::OS.to_string();
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        
        let device = Device::new(device_name, fingerprint, os, hostname);
        license.activate_device(device)?;
        self.repository.update(&license).await?;
        
        Ok(())
    }

    /// Deactivate a device
    pub async fn deactivate_device(
        &self,
        key: &str,
        device_id: &str,
    ) -> Result<()> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        let device_id = DeviceId::from_str(device_id)
            .map_err(|_| LicenseError::Internal("Invalid device ID".to_string()))?;
        
        let mut license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        license.deactivate_device(&device_id)?;
        self.repository.update(&license).await?;
        
        Ok(())
    }

    /// Upgrade license tier
    pub async fn upgrade_tier(
        &self,
        key: &str,
        new_tier: LicenseTier,
    ) -> Result<License> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let mut license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        license.upgrade_tier(new_tier)?;
        self.repository.update(&license).await?;
        
        Ok(license)
    }

    /// Link license to user
    pub async fn link_user(
        &self,
        key: &str,
        token: AuthToken,
    ) -> Result<()> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let mut license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        license.link_user(token);
        self.repository.update(&license).await?;
        
        Ok(())
    }

    /// Get license by key
    pub async fn get_license(&self, key: &str) -> Result<License> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)
    }

    /// Get license by user
    pub async fn get_license_by_user(&self, user_id: &str) -> Result<License> {
        self.repository.find_by_user(user_id).await?
            .ok_or(LicenseError::LicenseNotFound)
    }

    /// Validate license and check feature access
    pub async fn validate_license_for_feature(
        &self,
        key: &str,
        feature: crate::domain::license::Feature,
    ) -> Result<()> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let mut license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        license.validate()?;
        license.has_feature(feature)?;
        
        Ok(())
    }

    /// Get quota for license
    pub async fn get_quota(&self, key: &str) -> Result<Quota> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        
        let license = self.repository.find_by_key(&license_key).await?
            .ok_or(LicenseError::LicenseNotFound)?;
        
        Ok(Quota::for_tier(license.tier()))
    }

    /// List all licenses (admin function)
    pub async fn list_all_licenses(&self) -> Result<Vec<License>> {
        self.repository.list_all().await
    }

    /// Delete license
    pub async fn delete_license(&self, key: &str) -> Result<()> {
        let license_key = LicenseKey::from_string(key.to_string())?;
        self.repository.delete(&license_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemoryLicenseStore;

    #[tokio::test]
    async fn test_create_license() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo);
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        assert_eq!(license.tier(), LicenseTier::Pro);
        assert_eq!(license.email(), Some("user@example.com"));
    }

    #[tokio::test]
    async fn test_activate_license() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo);
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        let key = license.key().to_string();
        let activated = service.activate_license(&key).await.unwrap();
        
        assert_eq!(activated.key(), license.key());
    }

    #[tokio::test]
    async fn test_activate_device() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo.clone());
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        let key = license.key().to_string();
        let fingerprint = DeviceFingerprint::generate();
        
        service.activate_device(
            &key,
            "My Laptop".to_string(),
            fingerprint
        ).await.unwrap();
        
        // Verify device was activated
        let updated_license = repo.find_by_key(license.key()).await.unwrap().unwrap();
        assert_eq!(updated_license.active_devices().len(), 1);
        assert_eq!(updated_license.active_devices()[0].name(), "My Laptop");
    }

    #[tokio::test]
    async fn test_upgrade_tier() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo);
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        let key = license.key().to_string();
        let upgraded = service.upgrade_tier(&key, LicenseTier::Team).await.unwrap();
        
        assert_eq!(upgraded.tier(), LicenseTier::Team);
    }

    #[tokio::test]
    async fn test_validate_license_for_feature() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo);
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        let key = license.key().to_string();
        
        // Pro tier has encryption
        service.validate_license_for_feature(
            &key,
            crate::domain::license::Feature::Encryption
        ).await.unwrap();
        
        // Pro tier doesn't have parallel transfers
        let result = service.validate_license_for_feature(
            &key,
            crate::domain::license::Feature::ParallelTransfers
        ).await;
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_quota() {
        let repo = Arc::new(MemoryLicenseStore::new());
        let service = LicenseService::new(repo);
        
        let license = service.create_license(
            LicenseTier::Pro,
            Some("user@example.com".to_string())
        ).await.unwrap();
        
        let key = license.key().to_string();
        let quota = service.get_quota(&key).await.unwrap();
        
        assert_eq!(quota.monthly_bytes(), 100 * 1024 * 1024 * 1024); // 100GB
    }
}

// Made with Bob
