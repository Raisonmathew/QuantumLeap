//! Anonymous user handling
//!
//! Manages anonymous users who haven't registered yet

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Anonymous user identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AnonymousId(String);

impl AnonymousId {
    /// Generate a new anonymous ID
    pub fn generate() -> Self {
        Self(format!("anon_{}", Uuid::new_v4()))
    }

    /// Create from existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.0.clone()
    }
}

/// Anonymous user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymousUser {
    /// Anonymous ID
    id: AnonymousId,
    /// When the anonymous session was created
    created_at: chrono::DateTime<chrono::Utc>,
    /// Device fingerprint
    device_fingerprint: Option<String>,
}

impl AnonymousUser {
    /// Create a new anonymous user
    pub fn new() -> Self {
        Self {
            id: AnonymousId::generate(),
            created_at: chrono::Utc::now(),
            device_fingerprint: None,
        }
    }

    /// Create with device fingerprint
    pub fn with_fingerprint(fingerprint: String) -> Self {
        Self {
            id: AnonymousId::generate(),
            created_at: chrono::Utc::now(),
            device_fingerprint: Some(fingerprint),
        }
    }

    /// Get the anonymous ID
    pub fn id(&self) -> &AnonymousId {
        &self.id
    }

    /// Get creation time
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }

    /// Get device fingerprint
    pub fn device_fingerprint(&self) -> Option<&str> {
        self.device_fingerprint.as_deref()
    }

    /// Check if anonymous session is expired (30 days)
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        duration.num_days() > 30
    }
}

impl Default for AnonymousUser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_id_generation() {
        let id1 = AnonymousId::generate();
        let id2 = AnonymousId::generate();
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("anon_"));
    }

    #[test]
    fn test_anonymous_user_creation() {
        let user = AnonymousUser::new();
        assert!(user.id().as_str().starts_with("anon_"));
        assert!(!user.is_expired());
    }

    #[test]
    fn test_anonymous_user_with_fingerprint() {
        let user = AnonymousUser::with_fingerprint("test-fingerprint".to_string());
        assert_eq!(user.device_fingerprint(), Some("test-fingerprint"));
    }

    #[test]
    fn test_anonymous_id_from_string() {
        let id = AnonymousId::from_string("anon_test123".to_string());
        assert_eq!(id.as_str(), "anon_test123");
    }
}

// Made with Bob
