//! Authentication service (application layer)

use crate::domain::{AuthToken, Credentials, Session};
use crate::error::{AuthError, Result};
use crate::ports::SessionStore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Session information for display
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub username: String,
    pub age: Duration,
    pub remaining: Duration,
    pub is_expired: bool,
}

/// Authentication service (application layer)
/// 
/// Orchestrates authentication operations using domain entities and ports.
/// This is the main entry point for authentication functionality.
pub struct AuthService {
    /// User credentials storage (username -> password hash)
    credentials: Arc<RwLock<HashMap<String, String>>>,
    /// Session storage (via port/adapter pattern)
    session_store: Arc<dyn SessionStore>,
    /// Session time-to-live
    session_ttl: Duration,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(session_store: Arc<dyn SessionStore>, session_ttl: Duration) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
            session_store,
            session_ttl,
        }
    }

    /// Add a user with credentials
    pub fn add_user(&self, username: String, password: String) -> Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let password_hash = hex::encode(hasher.finalize());

        let mut creds = self
            .credentials
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;

        creds.insert(username, password_hash);
        Ok(())
    }

    /// Remove a user
    pub fn remove_user(&self, username: &str) -> Result<()> {
        let mut creds = self
            .credentials
            .write()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;

        creds.remove(username);
        Ok(())
    }

    /// Authenticate with credentials and create session
    pub fn authenticate(&self, credentials: &Credentials) -> Result<AuthToken> {
        // Verify credentials
        let creds = self
            .credentials
            .read()
            .map_err(|e| AuthError::Internal(format!("Lock error: {}", e)))?;

        let mut hasher = Sha256::new();
        hasher.update(credentials.password.as_bytes());
        let password_hash = hex::encode(hasher.finalize());

        match creds.get(&credentials.username) {
            Some(stored_hash) if stored_hash == &password_hash => {
                drop(creds); // Release read lock

                // Create session
                let token = AuthToken::new();
                let session = Session::new(
                    token.clone(),
                    credentials.username.clone(),
                    self.session_ttl,
                );

                self.session_store.save(session)?;
                Ok(token)
            }
            _ => Err(AuthError::InvalidCredentials),
        }
    }

    /// Verify a token and refresh session
    pub fn verify_token(&self, token: &AuthToken) -> Result<String> {
        match self.session_store.get(token)? {
            Some(mut session) if !session.is_expired() => {
                session.refresh(self.session_ttl);
                let username = session.username().to_string();
                self.session_store.save(session)?;
                Ok(username)
            }
            Some(_) => {
                self.session_store.remove(token)?;
                Err(AuthError::TokenExpired)
            }
            None => Err(AuthError::InvalidToken),
        }
    }

    /// Revoke a token (logout)
    pub fn revoke_token(&self, token: &AuthToken) -> Result<()> {
        self.session_store.remove(token)
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> Result<usize> {
        self.session_store.cleanup_expired()
    }

    /// Get active session count
    pub fn active_sessions(&self) -> Result<usize> {
        self.session_store.count()
    }

    /// Get session info for a token
    pub fn get_session_info(&self, token: &AuthToken) -> Result<SessionInfo> {
        match self.session_store.get(token)? {
            Some(session) => {
                let now = std::time::SystemTime::now();
                let age = now
                    .duration_since(session.created_at())
                    .unwrap_or(Duration::ZERO);
                let remaining = session
                    .expires_at()
                    .duration_since(now)
                    .unwrap_or(Duration::ZERO);

                Ok(SessionInfo {
                    username: session.username().to_string(),
                    age,
                    remaining,
                    is_expired: session.is_expired(),
                })
            }
            None => Err(AuthError::InvalidToken),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::MemorySessionStore;

    fn create_auth_service() -> AuthService {
        let store = Arc::new(MemorySessionStore::new());
        AuthService::new(store, Duration::from_secs(3600))
    }

    #[test]
    fn test_add_user_and_authenticate() {
        let service = create_auth_service();
        service.add_user("alice".to_string(), "password123".to_string()).unwrap();

        let creds = Credentials::new("alice".to_string(), "password123".to_string());
        let token = service.authenticate(&creds).unwrap();
        assert!(!token.as_str().is_empty());
    }

    #[test]
    fn test_invalid_credentials() {
        let service = create_auth_service();
        service.add_user("alice".to_string(), "password123".to_string()).unwrap();

        let creds = Credentials::new("alice".to_string(), "wrongpassword".to_string());
        let result = service.authenticate(&creds);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token() {
        let service = create_auth_service();
        service.add_user("bob".to_string(), "secret".to_string()).unwrap();

        let creds = Credentials::new("bob".to_string(), "secret".to_string());
        let token = service.authenticate(&creds).unwrap();

        let username = service.verify_token(&token).unwrap();
        assert_eq!(username, "bob");
    }

    #[test]
    fn test_revoke_token() {
        let service = create_auth_service();
        service.add_user("charlie".to_string(), "pass".to_string()).unwrap();

        let creds = Credentials::new("charlie".to_string(), "pass".to_string());
        let token = service.authenticate(&creds).unwrap();

        service.revoke_token(&token).unwrap();

        let result = service.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_session_expiry() {
        let store = Arc::new(MemorySessionStore::new());
        let service = AuthService::new(store, Duration::from_millis(100));
        service.add_user("dave".to_string(), "test".to_string()).unwrap();

        let creds = Credentials::new("dave".to_string(), "test".to_string());
        let token = service.authenticate(&creds).unwrap();

        std::thread::sleep(Duration::from_millis(150));

        let result = service.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let store = Arc::new(MemorySessionStore::new());
        let service = AuthService::new(store, Duration::from_millis(100));
        service.add_user("eve".to_string(), "pwd".to_string()).unwrap();

        let creds = Credentials::new("eve".to_string(), "pwd".to_string());
        let _token = service.authenticate(&creds).unwrap();

        assert_eq!(service.active_sessions().unwrap(), 1);

        std::thread::sleep(Duration::from_millis(150));

        let removed = service.cleanup_expired().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(service.active_sessions().unwrap(), 0);
    }

    #[test]
    fn test_session_info() {
        let service = create_auth_service();
        service.add_user("frank".to_string(), "key".to_string()).unwrap();

        let creds = Credentials::new("frank".to_string(), "key".to_string());
        let token = service.authenticate(&creds).unwrap();

        let info = service.get_session_info(&token).unwrap();
        assert_eq!(info.username, "frank");
        assert!(!info.is_expired);
        assert!(info.remaining.as_secs() > 3500);
    }
}

// Made with Bob
