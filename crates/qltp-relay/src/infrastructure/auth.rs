//! Authentication Module
//!
//! Provides authentication mechanisms for TURN server

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// Authentication credentials
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub realm: String,
}

/// Authentication manager
pub struct AuthManager {
    /// Static credentials (username -> password)
    credentials: Arc<RwLock<HashMap<String, String>>>,
    /// Realm for authentication
    realm: String,
}

impl AuthManager {
    /// Create new authentication manager
    pub fn new(realm: String) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
            realm,
        }
    }

    /// Add user credentials
    pub async fn add_user(&self, username: String, password: String) {
        let mut creds = self.credentials.write().await;
        creds.insert(username, password);
    }

    /// Remove user
    pub async fn remove_user(&self, username: &str) {
        let mut creds = self.credentials.write().await;
        creds.remove(username);
    }

    /// Verify MESSAGE-INTEGRITY attribute
    pub async fn verify_message_integrity(
        &self,
        username: &str,
        message: &[u8],
        integrity: &[u8; 20],
    ) -> bool {
        let creds = self.credentials.read().await;
        
        if let Some(password) = creds.get(username) {
            // Compute HMAC-SHA1
            let key = self.compute_key(username, password);
            let mut mac = HmacSha1::new_from_slice(&key).expect("HMAC can take key of any size");
            mac.update(message);
            
            let result = mac.finalize().into_bytes();
            result.as_slice() == integrity
        } else {
            false
        }
    }

    /// Compute long-term credential key
    fn compute_key(&self, username: &str, password: &str) -> Vec<u8> {
        // key = MD5(username ":" realm ":" password)
        let input = format!("{}:{}:{}", username, self.realm, password);
        md5::compute(input.as_bytes()).to_vec()
    }

    /// Get realm
    pub fn realm(&self) -> &str {
        &self.realm
    }

    /// Check if user exists
    pub async fn user_exists(&self, username: &str) -> bool {
        let creds = self.credentials.read().await;
        creds.contains_key(username)
    }

    /// Get user count
    pub async fn user_count(&self) -> usize {
        let creds = self.credentials.read().await;
        creds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_remove_user() {
        let auth = AuthManager::new("qltp.relay".to_string());
        
        auth.add_user("alice".to_string(), "secret123".to_string()).await;
        assert!(auth.user_exists("alice").await);
        assert_eq!(auth.user_count().await, 1);
        
        auth.remove_user("alice").await;
        assert!(!auth.user_exists("alice").await);
        assert_eq!(auth.user_count().await, 0);
    }

    #[tokio::test]
    async fn test_realm() {
        let auth = AuthManager::new("test.realm".to_string());
        assert_eq!(auth.realm(), "test.realm");
    }

    #[tokio::test]
    async fn test_multiple_users() {
        let auth = AuthManager::new("qltp.relay".to_string());
        
        auth.add_user("alice".to_string(), "pass1".to_string()).await;
        auth.add_user("bob".to_string(), "pass2".to_string()).await;
        auth.add_user("charlie".to_string(), "pass3".to_string()).await;
        
        assert_eq!(auth.user_count().await, 3);
        assert!(auth.user_exists("alice").await);
        assert!(auth.user_exists("bob").await);
        assert!(auth.user_exists("charlie").await);
    }
}

// Made with Bob