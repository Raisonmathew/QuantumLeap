//! Repository Interfaces
//!
//! These traits define the persistence layer interfaces that the application
//! layer depends on. Concrete implementations will be provided in the
//! infrastructure layer.

use crate::domain::{Connection, Peer, PeerId, Session, SessionId};
use crate::error::Result;
use async_trait::async_trait;

/// Repository for managing peer persistence
#[async_trait]
pub trait PeerRepository: Send + Sync {
    /// Save a peer
    async fn save(&self, peer: &Peer) -> Result<()>;

    /// Find a peer by ID
    async fn find_by_id(&self, id: &PeerId) -> Result<Option<Peer>>;

    /// Find all active peers
    async fn find_active(&self) -> Result<Vec<Peer>>;

    /// Find all connected peers
    async fn find_connected(&self) -> Result<Vec<Peer>>;

    /// Delete a peer
    async fn delete(&self, id: &PeerId) -> Result<()>;

    /// Check if peer exists
    async fn exists(&self, id: &PeerId) -> Result<bool>;

    /// Count total peers
    async fn count(&self) -> Result<usize>;

    /// Count active peers
    async fn count_active(&self) -> Result<usize>;
}

/// Repository for managing session persistence
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Save a session
    async fn save(&self, session: &Session) -> Result<()>;

    /// Find a session by ID
    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>>;

    /// Find all active sessions
    async fn find_active(&self) -> Result<Vec<Session>>;

    /// Find sessions for a specific peer
    async fn find_by_peer(&self, peer_id: &PeerId) -> Result<Vec<Session>>;

    /// Find sessions between two peers
    async fn find_by_peers(&self, peer1: &PeerId, peer2: &PeerId) -> Result<Vec<Session>>;

    /// Delete a session
    async fn delete(&self, id: &SessionId) -> Result<()>;

    /// Check if session exists
    async fn exists(&self, id: &SessionId) -> Result<bool>;

    /// Count total sessions
    async fn count(&self) -> Result<usize>;

    /// Count active sessions
    async fn count_active(&self) -> Result<usize>;
}

/// Repository for managing connection persistence
#[async_trait]
pub trait ConnectionRepository: Send + Sync {
    /// Save a connection
    async fn save(&self, connection: &Connection) -> Result<()>;

    /// Find a connection by session ID
    async fn find_by_session(&self, session_id: &SessionId) -> Result<Option<Connection>>;

    /// Find all active connections
    async fn find_active(&self) -> Result<Vec<Connection>>;

    /// Find connections for a specific peer
    async fn find_by_peer(&self, peer_id: &PeerId) -> Result<Vec<Connection>>;

    /// Delete a connection
    async fn delete(&self, session_id: &SessionId) -> Result<()>;

    /// Check if connection exists
    async fn exists(&self, session_id: &SessionId) -> Result<bool>;

    /// Count total connections
    async fn count(&self) -> Result<usize>;

    /// Count active connections
    async fn count_active(&self) -> Result<usize>;

    /// Count established connections
    async fn count_established(&self) -> Result<usize>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NatType, PeerCapabilities};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Mock implementation for testing
    struct MockPeerRepository {
        peers: Arc<Mutex<HashMap<PeerId, Peer>>>,
    }

    impl MockPeerRepository {
        fn new() -> Self {
            Self {
                peers: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl PeerRepository for MockPeerRepository {
        async fn save(&self, peer: &Peer) -> Result<()> {
            let mut peers = self.peers.lock().unwrap();
            peers.insert(peer.id().clone(), peer.clone());
            Ok(())
        }

        async fn find_by_id(&self, id: &PeerId) -> Result<Option<Peer>> {
            let peers = self.peers.lock().unwrap();
            Ok(peers.get(id).cloned())
        }

        async fn find_active(&self) -> Result<Vec<Peer>> {
            let peers = self.peers.lock().unwrap();
            Ok(peers.values().filter(|p| p.is_active()).cloned().collect())
        }

        async fn find_connected(&self) -> Result<Vec<Peer>> {
            let peers = self.peers.lock().unwrap();
            Ok(peers
                .values()
                .filter(|p| p.is_connected())
                .cloned()
                .collect())
        }

        async fn delete(&self, id: &PeerId) -> Result<()> {
            let mut peers = self.peers.lock().unwrap();
            peers.remove(id);
            Ok(())
        }

        async fn exists(&self, id: &PeerId) -> Result<bool> {
            let peers = self.peers.lock().unwrap();
            Ok(peers.contains_key(id))
        }

        async fn count(&self) -> Result<usize> {
            let peers = self.peers.lock().unwrap();
            Ok(peers.len())
        }

        async fn count_active(&self) -> Result<usize> {
            let peers = self.peers.lock().unwrap();
            Ok(peers.values().filter(|p| p.is_active()).count())
        }
    }

    #[tokio::test]
    async fn test_peer_repository_save_and_find() {
        let repo = MockPeerRepository::new();
        let peer_id = PeerId::new();
        let capabilities = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let peer = Peer::new(peer_id.clone(), capabilities);

        // Save peer
        repo.save(&peer).await.unwrap();

        // Find peer
        let found = repo.find_by_id(&peer_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), &peer_id);
    }

    #[tokio::test]
    async fn test_peer_repository_delete() {
        let repo = MockPeerRepository::new();
        let peer_id = PeerId::new();
        let capabilities = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let peer = Peer::new(peer_id.clone(), capabilities);

        repo.save(&peer).await.unwrap();
        assert!(repo.exists(&peer_id).await.unwrap());

        repo.delete(&peer_id).await.unwrap();
        assert!(!repo.exists(&peer_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_peer_repository_count() {
        let repo = MockPeerRepository::new();

        assert_eq!(repo.count().await.unwrap(), 0);

        let capabilities1 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let capabilities2 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let peer1 = Peer::new(PeerId::new(), capabilities1);
        let peer2 = Peer::new(PeerId::new(), capabilities2);

        repo.save(&peer1).await.unwrap();
        repo.save(&peer2).await.unwrap();

        assert_eq!(repo.count().await.unwrap(), 2);
    }
}

// Made with Bob
