//! License repository port
//!
//! Defines the interface for license persistence

use crate::domain::license::{License, LicenseId, LicenseKey};
use crate::error::Result;
use async_trait::async_trait;

/// Repository for license persistence
#[async_trait]
pub trait LicenseRepository: Send + Sync {
    /// Save a license
    async fn save(&self, license: &License) -> Result<()>;

    /// Find license by ID
    async fn find_by_id(&self, id: &LicenseId) -> Result<Option<License>>;

    /// Find license by key
    async fn find_by_key(&self, key: &LicenseKey) -> Result<Option<License>>;

    /// Find license by user token
    async fn find_by_user(&self, user_id: &str) -> Result<Option<License>>;

    /// Update license
    async fn update(&self, license: &License) -> Result<()>;

    /// Delete license
    async fn delete(&self, key: &LicenseKey) -> Result<()>;

    /// List all licenses (for admin purposes)
    async fn list_all(&self) -> Result<Vec<License>>;

    /// Check if license exists
    async fn exists(&self, key: &LicenseKey) -> Result<bool>;
}

// Made with Bob
