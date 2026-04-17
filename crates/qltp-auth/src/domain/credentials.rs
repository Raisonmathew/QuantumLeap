//! User credentials value object

use super::token::AuthToken;
use serde::{Deserialize, Serialize};

/// User credentials value object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    /// Create new credentials
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    /// Generate authentication token from credentials
    pub fn to_token(&self) -> AuthToken {
        AuthToken::from_credentials(&self.username, &self.password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_creation() {
        let creds = Credentials::new("alice".to_string(), "secret123".to_string());
        assert_eq!(creds.username, "alice");
        assert_eq!(creds.password, "secret123");
    }

    #[test]
    fn test_credentials_to_token() {
        let creds = Credentials::new("alice".to_string(), "secret123".to_string());
        let token = creds.to_token();
        assert!(!token.as_str().is_empty());
    }
}

// Made with Bob
