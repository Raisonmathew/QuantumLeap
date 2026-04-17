//! Peer identifier value object

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a peer in the relay network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(Uuid);

impl PeerId {
    /// Create a new random peer ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for PeerId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for PeerId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_creation() {
        let id1 = PeerId::new();
        let id2 = PeerId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_peer_id_display() {
        let id = PeerId::new();
        let display = format!("{}", id);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_peer_id_serialization() {
        let id = PeerId::new();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: PeerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}

// Made with Bob
