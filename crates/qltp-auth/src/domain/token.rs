//! Authentication token entity

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Authentication token entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AuthToken(String);

impl AuthToken {
    /// Create a new random authentication token
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create token from string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get token as string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate token from credentials (deterministic)
    pub fn from_credentials(username: &str, password: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(username.as_bytes());
        hasher.update(b":");
        hasher.update(password.as_bytes());
        let hash = hasher.finalize();
        Self(hex::encode(hash))
    }
}

impl Default for AuthToken {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_token_creation() {
        let token1 = AuthToken::new();
        let token2 = AuthToken::new();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_auth_token_from_credentials() {
        let token1 = AuthToken::from_credentials("user1", "pass1");
        let token2 = AuthToken::from_credentials("user1", "pass1");
        let token3 = AuthToken::from_credentials("user2", "pass1");
        
        assert_eq!(token1, token2);
        assert_ne!(token1, token3);
    }

    #[test]
    fn test_auth_token_display() {
        let token = AuthToken::from_string("test-token-123".to_string());
        assert_eq!(token.to_string(), "test-token-123");
    }
}

// Made with Bob
