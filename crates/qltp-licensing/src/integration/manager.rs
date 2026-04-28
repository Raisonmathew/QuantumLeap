//! Unified Authentication and Licensing Manager
//!
//! Combines authentication and licensing into a single, cohesive system

use crate::application::{LicenseService, UsageTracker};
use crate::domain::license::{License, LicenseTier};
use crate::domain::usage::TransferType;
use crate::error::{LicenseError, Result};
use crate::integration::session::EnhancedSession;
use crate::integration::user::{UserAccount, UserRegistration};
use crate::ports::{LicenseRepository, UsageRepository};
use qltp_auth::{AuthManager, Credentials, MemorySessionStore};
use std::sync::{Arc, Mutex};

/// Unified manager for authentication and licensing
pub struct AuthLicenseManager {
    /// Authentication manager
    auth_manager: Arc<AuthManager>,
    /// License service
    license_service: Arc<LicenseService>,
    /// Usage tracker
    usage_tracker: Arc<UsageTracker>,
    /// Current session
    current_session: Arc<Mutex<Option<EnhancedSession>>>,
    /// User accounts (in-memory for now, would be database in production)
    users: Arc<Mutex<Vec<UserAccount>>>,
}

impl AuthLicenseManager {
    /// Create a new manager
    pub fn new(
        license_repo: Arc<dyn LicenseRepository>,
        usage_repo: Arc<dyn UsageRepository>,
    ) -> Self {
        let session_store = Arc::new(MemorySessionStore::new());
        Self {
            auth_manager: Arc::new(AuthManager::new(
                session_store,
                std::time::Duration::from_secs(3600),
            )),
            license_service: Arc::new(LicenseService::new(license_repo.clone())),
            usage_tracker: Arc::new(UsageTracker::new(license_repo, usage_repo)),
            current_session: Arc::new(Mutex::new(None)),
            users: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Start an anonymous session
    pub fn start_anonymous_session(&self) -> Result<EnhancedSession> {
        let session = EnhancedSession::anonymous();
        *self.current_session.lock().unwrap() = Some(session.clone());
        Ok(session)
    }

    /// Register a new user
    pub async fn register_user(&self, registration: UserRegistration) -> Result<UserAccount> {
        // Validate registration
        registration.validate()?;

        // Check if email already exists
        let users = self.users.lock().unwrap();
        if users.iter().any(|u| u.email() == registration.email) {
            return Err(LicenseError::InvalidInput {
                field: "email".to_string(),
                message: "Email already registered".to_string(),
            });
        }
        drop(users);

        // Hash password with Argon2id (OWASP-recommended). The hash is stored
        // both on the UserAccount and inside the AuthManager session store;
        // the AuthManager copy is what we verify against during login, but
        // the UserAccount copy is preserved for migration / introspection.
        let password_hash = {
            use argon2::Argon2;
            use password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(registration.password.as_bytes(), &salt)
                .map_err(|e| LicenseError::Internal(format!("Password hashing failed: {}", e)))?
                .to_string()
        };

        // Register user with AuthManager
        self.auth_manager
            .add_user(registration.email.clone(), registration.password.clone())?;

        // Create user account
        let mut account = UserAccount::from_registration(registration.clone(), password_hash);

        // If migrating from anonymous, create a Free license
        if registration.anonymous_id.is_some() {
            let license = self
                .license_service
                .create_license(LicenseTier::Free, Some(account.email().to_string()))
                .await?;
            account.add_license_key(license.key().clone());
        }

        // Store user
        self.users.lock().unwrap().push(account.clone());

        Ok(account)
    }

    /// Authenticate user and create session
    pub async fn authenticate(&self, email: &str, password: &str) -> Result<EnhancedSession> {
        // Find user
        let users = self.users.lock().unwrap();
        let user = users
            .iter()
            .find(|u| u.email() == email)
            .ok_or(LicenseError::InvalidCredentials)?;

        // Verify password (simplified)
        // In production, use proper password verification with constant-time comparison
        let _expected_hash = format!("hashed_{}", password);

        if !user.is_active() {
            return Err(LicenseError::InvalidInput {
                field: "account".to_string(),
                message: "Account is not active".to_string(),
            });
        }

        // Create auth token
        let credentials = Credentials::new(email.to_string(), password.to_string());
        let token = self.auth_manager.authenticate(&credentials)?;

        // Get user's license (use first one, or create Free tier)
        let license = if let Some(key) = user.license_keys().first() {
            self.license_service
                .get_license(&key.to_string())
                .await?
        } else {
            // Create a Free license for the user
            self.license_service
                .create_license(LicenseTier::Free, Some(email.to_string()))
                .await?
        };

        // Create enhanced session
        let session = EnhancedSession::registered(user.id().clone(), token, license);
        *self.current_session.lock().unwrap() = Some(session.clone());

        Ok(session)
    }

    /// Get current session
    pub fn get_session(&self) -> Result<EnhancedSession> {
        self.current_session
            .lock()
            .unwrap()
            .clone()
            .ok_or(LicenseError::NoActiveSession)
    }

    /// Activate a license for current session
    pub async fn activate_license(&self, license_key: &str) -> Result<License> {
        let mut session_guard = self.current_session.lock().unwrap();
        let session = session_guard
            .as_mut()
            .ok_or(LicenseError::NoActiveSession)?;

        // Get the license
        let license = self.license_service.get_license(license_key).await?;

        // Update session with new license
        session.set_license(license.clone());

        Ok(license)
    }

    /// Record a transfer
    pub async fn record_transfer(&self, bytes: u64, transfer_type: TransferType) -> Result<()> {
        let session = self.get_session()?;
        let license = session
            .license()
            .ok_or(LicenseError::LicenseNotFound)?;

        // Record usage
        self.usage_tracker
            .record_transfer(license.id().clone(), bytes, transfer_type)
            .await?;

        // Update session quota
        let mut session_guard = self.current_session.lock().unwrap();
        if let Some(ref mut sess) = *session_guard {
            sess.info_mut().add_quota_usage(bytes);
        }

        Ok(())
    }

    /// Check if transfer is allowed
    pub async fn can_transfer(&self, bytes: u64) -> Result<bool> {
        let session = self.get_session()?;
        
        // Check quota
        if session.info().is_quota_exceeded() {
            return Ok(false);
        }

        // Check if adding this transfer would exceed quota
        if let Some(remaining) = session.info().quota_remaining() {
            if bytes > remaining {
                return Ok(false);
            }
        }

        // Check with usage tracker
        let license = session
            .license()
            .ok_or(LicenseError::LicenseNotFound)?;
        
        self.usage_tracker
            .check_quota(license.id(), bytes)
            .await?;

        Ok(true)
    }

    /// Get current usage statistics
    pub async fn get_usage_stats(&self) -> Result<(u64, Option<u64>)> {
        let session = self.get_session()?;
        let used = session.info().quota_used();
        let limit = session.info().quota_limit();
        Ok((used, limit))
    }

    /// Upgrade from anonymous to registered
    pub async fn upgrade_anonymous_to_registered(
        &self,
        registration: UserRegistration,
    ) -> Result<UserAccount> {
        // Get current anonymous session
        let current_session = self.get_session()?;
        if !current_session.info().is_anonymous() {
            return Err(LicenseError::InvalidInput {
                field: "session".to_string(),
                message: "Session is not anonymous".to_string(),
            });
        }

        // Register user with anonymous ID
        let account = self.register_user(registration).await?;

        Ok(account)
    }

    /// Logout current session
    pub fn logout(&self) -> Result<()> {
        let mut session_guard = self.current_session.lock().unwrap();
        if let Some(session) = session_guard.as_ref() {
            if let Some(token) = session.info().auth_token() {
                self.auth_manager.revoke_token(token)?;
            }
        }
        *session_guard = None;
        Ok(())
    }

    /// Get license service
    pub fn license_service(&self) -> &LicenseService {
        &self.license_service
    }

    /// Get usage tracker
    pub fn usage_tracker(&self) -> &UsageTracker {
        &self.usage_tracker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{MemoryLicenseStore, MemoryUsageStore};

    fn create_manager() -> AuthLicenseManager {
        let license_repo = Arc::new(MemoryLicenseStore::new());
        let usage_repo = Arc::new(MemoryUsageStore::new());
        AuthLicenseManager::new(license_repo, usage_repo)
    }

    #[tokio::test]
    async fn test_anonymous_session() {
        let manager = create_manager();
        let session = manager.start_anonymous_session().unwrap();
        assert!(session.info().is_anonymous());
        assert_eq!(session.info().tier(), LicenseTier::Free);
    }

    #[tokio::test]
    async fn test_user_registration() {
        let manager = create_manager();
        let registration = UserRegistration::new(
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        let account = manager.register_user(registration).await.unwrap();
        assert_eq!(account.email(), "test@example.com");
    }

    #[tokio::test]
    async fn test_duplicate_email_registration() {
        let manager = create_manager();
        let registration = UserRegistration::new(
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        manager.register_user(registration.clone()).await.unwrap();
        
        // Try to register again with same email
        let result = manager.register_user(registration).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_authentication() {
        let manager = create_manager();
        
        // Register user
        let registration = UserRegistration::new(
            "test@example.com".to_string(),
            "password123".to_string(),
        );
        manager.register_user(registration).await.unwrap();
        
        // Authenticate
        let session = manager.authenticate("test@example.com", "password123").await.unwrap();
        assert!(session.info().is_registered());
    }

    #[tokio::test]
    async fn test_license_activation() {
        let manager = create_manager();
        manager.start_anonymous_session().unwrap();
        
        // Create a license
        let license = manager
            .license_service()
            .create_license(LicenseTier::Pro, Some("test@example.com".to_string()))
            .await
            .unwrap();
        
        // Activate it
        let activated = manager.activate_license(&license.key().to_string()).await.unwrap();
        assert_eq!(activated.tier(), LicenseTier::Pro);
        
        // Check session was updated
        let session = manager.get_session().unwrap();
        assert_eq!(session.info().tier(), LicenseTier::Pro);
    }

    #[tokio::test]
    async fn test_transfer_recording() {
        let manager = create_manager();
        manager.start_anonymous_session().unwrap();
        
        // Create and activate a license
        let license = manager
            .license_service()
            .create_license(LicenseTier::Pro, Some("test@example.com".to_string()))
            .await
            .unwrap();
        manager.activate_license(&license.key().to_string()).await.unwrap();
        
        // Record a transfer
        manager.record_transfer(1024 * 1024, TransferType::Upload).await.unwrap();
        
        // Check usage
        let (used, _) = manager.get_usage_stats().await.unwrap();
        assert_eq!(used, 1024 * 1024);
    }

    #[tokio::test]
    async fn test_can_transfer() {
        let manager = create_manager();
        manager.start_anonymous_session().unwrap();
        
        // Create and activate a Free license
        let license = manager
            .license_service()
            .create_license(LicenseTier::Free, Some("test@example.com".to_string()))
            .await
            .unwrap();
        manager.activate_license(&license.key().to_string()).await.unwrap();
        
        // Should be able to transfer small amount
        assert!(manager.can_transfer(1024 * 1024).await.unwrap());
    }
}

// Made with Bob
