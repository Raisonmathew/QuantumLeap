//! Session entity

use super::token::AuthToken;
use std::time::{Duration, SystemTime};

/// Session entity representing an active user session
#[derive(Debug, Clone)]
pub struct Session {
    token: AuthToken,
    username: String,
    created_at: SystemTime,
    last_activity: SystemTime,
    expires_at: SystemTime,
}

impl Session {
    /// Create a new session
    pub fn new(token: AuthToken, username: String, ttl: Duration) -> Self {
        let now = SystemTime::now();
        Self {
            token,
            username,
            created_at: now,
            last_activity: now,
            expires_at: now + ttl,
        }
    }

    /// Get the session token
    pub fn token(&self) -> &AuthToken {
        &self.token
    }

    /// Get the username
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    /// Refresh the session (update last activity and expiration)
    pub fn refresh(&mut self, ttl: Duration) {
        self.last_activity = SystemTime::now();
        self.expires_at = self.last_activity + ttl;
    }

    /// Get session creation time
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Get session expiration time
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    /// Get last activity time
    pub fn last_activity(&self) -> SystemTime {
        self.last_activity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let token = AuthToken::new();
        let session = Session::new(token.clone(), "alice".to_string(), Duration::from_secs(3600));
        
        assert_eq!(session.token(), &token);
        assert_eq!(session.username(), "alice");
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_expiry() {
        let token = AuthToken::new();
        let session = Session::new(token, "bob".to_string(), Duration::from_millis(10));
        
        std::thread::sleep(Duration::from_millis(20));
        assert!(session.is_expired());
    }

    #[test]
    fn test_session_refresh() {
        let token = AuthToken::new();
        let mut session = Session::new(token, "charlie".to_string(), Duration::from_secs(1));
        
        std::thread::sleep(Duration::from_millis(100));
        session.refresh(Duration::from_secs(3600));
        
        assert!(!session.is_expired());
    }
}

// Made with Bob
