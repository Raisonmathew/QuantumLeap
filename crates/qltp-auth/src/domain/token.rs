//! Authentication token entity

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Authentication token entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AuthToken(String);

impl AuthToken {
    /// Create a new random authentication token (256 bits of entropy from the
    /// OS CSPRNG, hex-encoded). Suitable for use as an opaque session token.
    ///
    /// UUID v4 was previously used here but only carries 122 bits of entropy
    /// and is not specified to come from a CSPRNG, so it is unsuitable for
    /// auth bearer tokens.
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(hex::encode(bytes))
    }

    /// Create token from string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get token as string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate a deterministic identifier from credentials.
    ///
    /// **Not** for use as a session/bearer token: the output is fully
    /// determined by `(username, password)`, so anyone who learns the password
    /// can recompute it. Use [`AuthToken::new`] for session tokens.
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
