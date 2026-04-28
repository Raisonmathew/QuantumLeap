//! User account management
//!
//! Handles registered user accounts and their associated licenses

use crate::domain::license::LicenseKey;
use crate::error::{LicenseError, Result};
use crate::integration::anonymous::AnonymousId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User account identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Uuid);

impl UserId {
    /// Generate a new user ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

/// User registration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegistration {
    /// Email address
    pub email: String,
    /// Password (will be hashed)
    pub password: String,
    /// Optional name
    pub name: Option<String>,
    /// Anonymous ID to migrate from (if any)
    pub anonymous_id: Option<AnonymousId>,
}

impl UserRegistration {
    /// Create a new registration
    pub fn new(email: String, password: String) -> Self {
        Self {
            email,
            password,
            name: None,
            anonymous_id: None,
        }
    }

    /// Set name
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Set anonymous ID for migration
    pub fn with_anonymous_id(mut self, id: AnonymousId) -> Self {
        self.anonymous_id = Some(id);
        self
    }

    /// Validate registration data
    pub fn validate(&self) -> Result<()> {
        // Validate email
        if !self.email.contains('@') {
            return Err(LicenseError::InvalidInput {
                field: "email".to_string(),
                message: "Invalid email format".to_string(),
            });
        }

        // Validate password
        if self.password.len() < 8 {
            return Err(LicenseError::InvalidInput {
                field: "password".to_string(),
                message: "Password must be at least 8 characters".to_string(),
            });
        }

        Ok(())
    }
}

/// User account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    /// User ID
    id: UserId,
    /// Email address
    email: String,
    /// Display name
    name: Option<String>,
    /// Password hash (not stored in memory long-term)
    #[serde(skip)]
    password_hash: Option<String>,
    /// Associated license keys
    license_keys: Vec<LicenseKey>,
    /// Previous anonymous ID (if migrated)
    previous_anonymous_id: Option<AnonymousId>,
    /// Account creation time
    created_at: DateTime<Utc>,
    /// Email verified
    email_verified: bool,
    /// Account status
    status: AccountStatus,
}

/// Account status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    /// Active account
    Active,
    /// Suspended account
    Suspended,
    /// Deleted account
    Deleted,
}

impl UserAccount {
    /// Create a new user account
    pub fn new(email: String) -> Self {
        Self {
            id: UserId::new(),
            email,
            name: None,
            password_hash: None,
            license_keys: Vec::new(),
            previous_anonymous_id: None,
            created_at: Utc::now(),
            email_verified: false,
            status: AccountStatus::Active,
        }
    }

    /// Create from registration
    pub fn from_registration(registration: UserRegistration, password_hash: String) -> Self {
        Self {
            id: UserId::new(),
            email: registration.email,
            name: registration.name,
            password_hash: Some(password_hash),
            license_keys: Vec::new(),
            previous_anonymous_id: registration.anonymous_id,
            created_at: Utc::now(),
            email_verified: false,
            status: AccountStatus::Active,
        }
    }

    /// Get user ID
    pub fn id(&self) -> &UserId {
        &self.id
    }

    /// Get email
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Verify a password against the stored Argon2id PHC hash.
    ///
    /// Returns `true` if the password matches. Any parse error or absence of
    /// a stored hash returns `false`. Verification is constant-time inside
    /// the underlying Argon2 implementation.
    pub fn verify_password(&self, password: &str) -> bool {
        use password_hash::PasswordVerifier;
        let Some(stored) = self.password_hash.as_deref() else {
            return false;
        };
        match password_hash::PasswordHash::new(stored) {
            Ok(parsed) => argon2::Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    }

    /// Get name
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Set name
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// Get license keys
    pub fn license_keys(&self) -> &[LicenseKey] {
        &self.license_keys
    }

    /// Add license key
    pub fn add_license_key(&mut self, key: LicenseKey) {
        if !self.license_keys.contains(&key) {
            self.license_keys.push(key);
        }
    }

    /// Remove license key
    pub fn remove_license_key(&mut self, key: &LicenseKey) {
        self.license_keys.retain(|k| k != key);
    }

    /// Get previous anonymous ID
    pub fn previous_anonymous_id(&self) -> Option<&AnonymousId> {
        self.previous_anonymous_id.as_ref()
    }

    /// Get creation time
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Check if email is verified
    pub fn is_email_verified(&self) -> bool {
        self.email_verified
    }

    /// Mark email as verified
    pub fn verify_email(&mut self) {
        self.email_verified = true;
    }

    /// Get account status
    pub fn status(&self) -> AccountStatus {
        self.status
    }

    /// Check if account is active
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    /// Suspend account
    pub fn suspend(&mut self) {
        self.status = AccountStatus::Suspended;
    }

    /// Reactivate account
    pub fn reactivate(&mut self) {
        self.status = AccountStatus::Active;
    }

    /// Delete account (soft delete)
    pub fn delete(&mut self) {
        self.status = AccountStatus::Deleted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_id_generation() {
        let id1 = UserId::new();
        let id2 = UserId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_user_registration_validation() {
        let valid = UserRegistration::new(
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        assert!(valid.validate().is_ok());

        let invalid_email = UserRegistration::new(
            "invalid-email".to_string(),
            "password123".to_string(),
        );
        assert!(invalid_email.validate().is_err());

        let short_password = UserRegistration::new(
            "test@example.com".to_string(),
            "short".to_string(),
        );
        assert!(short_password.validate().is_err());
    }

    #[test]
    fn test_user_account_creation() {
        let account = UserAccount::new("test@example.com".to_string());
        assert_eq!(account.email(), "test@example.com");
        assert!(account.is_active());
        assert!(!account.is_email_verified());
        assert_eq!(account.license_keys().len(), 0);
    }

    #[test]
    fn test_user_account_license_management() {
        let mut account = UserAccount::new("test@example.com".to_string());
        let key = LicenseKey::generate(crate::domain::license::LicenseTier::Pro);
        
        account.add_license_key(key.clone());
        assert_eq!(account.license_keys().len(), 1);
        
        // Adding same key again should not duplicate
        account.add_license_key(key.clone());
        assert_eq!(account.license_keys().len(), 1);
        
        account.remove_license_key(&key);
        assert_eq!(account.license_keys().len(), 0);
    }

    #[test]
    fn test_user_account_status() {
        let mut account = UserAccount::new("test@example.com".to_string());
        assert!(account.is_active());
        
        account.suspend();
        assert_eq!(account.status(), AccountStatus::Suspended);
        assert!(!account.is_active());
        
        account.reactivate();
        assert!(account.is_active());
        
        account.delete();
        assert_eq!(account.status(), AccountStatus::Deleted);
    }

    #[test]
    fn test_user_registration_with_anonymous_id() {
        let anon_id = AnonymousId::generate();
        let registration = UserRegistration::new(
            "test@example.com".to_string(),
            "password123".to_string(),
        )
        .with_anonymous_id(anon_id.clone());
        
        assert_eq!(registration.anonymous_id.as_ref(), Some(&anon_id));
    }
}

// Made with Bob
