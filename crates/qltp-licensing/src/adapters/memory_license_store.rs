//! In-memory license store adapter
//!
//! Provides an in-memory implementation of LicenseRepository for testing
//! and development purposes.

use crate::domain::license::{License, LicenseId, LicenseKey};
use crate::error::{LicenseError, Result};
use crate::ports::LicenseRepository;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory license store
#[derive(Clone)]
pub struct MemoryLicenseStore {
    licenses: Arc<RwLock<HashMap<String, License>>>,
}

impl MemoryLicenseStore {
    /// Create a new in-memory license store
    pub fn new() -> Self {
        Self {
            licenses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of licenses stored
    pub fn len(&self) -> usize {
        self.licenses.read().unwrap().len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.licenses.read().unwrap().is_empty()
    }

    /// Clear all licenses
    pub fn clear(&self) {
        self.licenses.write().unwrap().clear();
    }
}

impl Default for MemoryLicenseStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LicenseRepository for MemoryLicenseStore {
    async fn save(&self, license: &License) -> Result<()> {
        let id = license.id().to_string();
        let mut licenses = self.licenses.write().unwrap();
        
        if licenses.contains_key(&id) {
            return Err(LicenseError::LicenseAlreadyExists);
        }
        
        licenses.insert(id, license.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &LicenseId) -> Result<Option<License>> {
        let licenses = self.licenses.read().unwrap();
        Ok(licenses.get(&id.to_string()).cloned())
    }

    async fn find_by_key(&self, key: &LicenseKey) -> Result<Option<License>> {
        let licenses = self.licenses.read().unwrap();
        
        // Search by key value since we store by ID
        for license in licenses.values() {
            if license.key().to_string() == key.to_string() {
                return Ok(Some(license.clone()));
            }
        }
        
        Ok(None)
    }

    async fn find_by_user(&self, user_id: &str) -> Result<Option<License>> {
        let licenses = self.licenses.read().unwrap();
        
        for license in licenses.values() {
            if let Some(email) = license.email() {
                if email == user_id {
                    return Ok(Some(license.clone()));
                }
            }
        }
        
        Ok(None)
    }

    async fn update(&self, license: &License) -> Result<()> {
        let id = license.id().to_string();
        let mut licenses = self.licenses.write().unwrap();
        
        if !licenses.contains_key(&id) {
            return Err(LicenseError::LicenseNotFound);
        }
        
        licenses.insert(id, license.clone());
        Ok(())
    }

    async fn delete(&self, key: &LicenseKey) -> Result<()> {
        let mut licenses = self.licenses.write().unwrap();
        
        // Find by key value and remove by ID
        let id_to_remove = licenses
            .iter()
            .find(|(_, license)| license.key().to_string() == key.to_string())
            .map(|(id, _)| id.clone());
        
        if let Some(id) = id_to_remove {
            licenses.remove(&id);
            Ok(())
        } else {
            Err(LicenseError::LicenseNotFound)
        }
    }

    async fn list_all(&self) -> Result<Vec<License>> {
        let licenses = self.licenses.read().unwrap();
        Ok(licenses.values().cloned().collect())
    }

    async fn exists(&self, key: &LicenseKey) -> Result<bool> {
        let licenses = self.licenses.read().unwrap();
        
        // Check if any license has this key
        Ok(licenses.values().any(|license| license.key().to_string() == key.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::license::LicenseTier;

    #[tokio::test]
    async fn test_save_and_find() {
        let store = MemoryLicenseStore::new();
        let license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        
        let found = store.find_by_key(license.key()).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().key(), license.key());
    }

    #[tokio::test]
    async fn test_duplicate_save() {
        let store = MemoryLicenseStore::new();
        let license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        let result = store.save(&license).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LicenseError::LicenseAlreadyExists));
    }

    #[tokio::test]
    async fn test_update() {
        let store = MemoryLicenseStore::new();
        let mut license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        
        license.upgrade_tier(LicenseTier::Team).unwrap();
        store.update(&license).await.unwrap();
        
        let found = store.find_by_key(license.key()).await.unwrap().unwrap();
        assert_eq!(found.tier(), LicenseTier::Team);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = MemoryLicenseStore::new();
        let license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        store.save(&license).await.unwrap();
        assert_eq!(store.len(), 1);
        
        store.delete(license.key()).await.unwrap();
        assert_eq!(store.len(), 0);
    }

    #[tokio::test]
    async fn test_list_all() {
        let store = MemoryLicenseStore::new();
        
        let license1 = License::new(LicenseTier::Pro, Some("user1@example.com".to_string()));
        let license2 = License::new(LicenseTier::Team, Some("user2@example.com".to_string()));
        
        store.save(&license1).await.unwrap();
        store.save(&license2).await.unwrap();
        
        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_exists() {
        let store = MemoryLicenseStore::new();
        let license = License::new(LicenseTier::Pro, Some("user@example.com".to_string()));
        
        assert!(!store.exists(license.key()).await.unwrap());
        
        store.save(&license).await.unwrap();
        assert!(store.exists(license.key()).await.unwrap());
    }
}

// Made with Bob
