//! In-Memory Storage Implementations
//!
//! These implementations use DashMap for thread-safe, lock-free storage.
//! Suitable for development, testing, and single-instance deployments.

use crate::domain::{Connection, Peer, PeerId, Session, SessionId};
use crate::error::Result;
use crate::ports::{ConnectionRepository, PeerRepository, SessionRepository};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;

/// In-memory peer repository
#[derive(Clone)]
pub struct InMemoryPeerRepository {
    peers: Arc<DashMap<PeerId, Peer>>,
}

impl InMemoryPeerRepository {
    /// Create a new in-memory peer repository
    pub fn new() -> Self {
        Self {
            peers: Arc::new(DashMap::new()),
        }
    }

    /// Get all peers (for testing/debugging)
    pub fn all(&self) -> Vec<Peer> {
        self.peers.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Clear all peers (for testing)
    pub fn clear(&self) {
        self.peers.clear();
    }
}

impl Default for InMemoryPeerRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PeerRepository for InMemoryPeerRepository {
    async fn save(&self, peer: &Peer) -> Result<()> {
        self.peers.insert(peer.id().clone(), peer.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &PeerId) -> Result<Option<Peer>> {
        Ok(self.peers.get(id).map(|entry| entry.value().clone()))
    }

    async fn find_active(&self) -> Result<Vec<Peer>> {
        Ok(self
            .peers
            .iter()
            .filter(|entry| entry.value().is_active())
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn find_connected(&self) -> Result<Vec<Peer>> {
        Ok(self
            .peers
            .iter()
            .filter(|entry| entry.value().is_connected())
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn delete(&self, id: &PeerId) -> Result<()> {
        self.peers.remove(id);
        Ok(())
    }

    async fn exists(&self, id: &PeerId) -> Result<bool> {
        Ok(self.peers.contains_key(id))
    }

    async fn count(&self) -> Result<usize> {
        Ok(self.peers.len())
    }

    async fn count_active(&self) -> Result<usize> {
        Ok(self
            .peers
            .iter()
            .filter(|entry| entry.value().is_active())
            .count())
    }
}

/// In-memory session repository
#[derive(Clone)]
pub struct InMemorySessionRepository {
    sessions: Arc<DashMap<SessionId, Session>>,
}

impl InMemorySessionRepository {
    /// Create a new in-memory session repository
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Get all sessions (for testing/debugging)
    pub fn all(&self) -> Vec<Session> {
        self.sessions.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Clear all sessions (for testing)
    pub fn clear(&self) {
        self.sessions.clear();
    }
}

impl Default for InMemorySessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn save(&self, session: &Session) -> Result<()> {
        self.sessions.insert(session.id().clone(), session.clone());
        Ok(())
    }

    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>> {
        Ok(self.sessions.get(id).map(|entry| entry.value().clone()))
    }

    async fn find_active(&self) -> Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .filter(|entry| entry.value().is_active())
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn find_by_peer(&self, peer_id: &PeerId) -> Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .filter(|entry| entry.value().involves_peer(peer_id))
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn find_by_peers(&self, peer1: &PeerId, peer2: &PeerId) -> Result<Vec<Session>> {
        Ok(self
            .sessions
            .iter()
            .filter(|entry| {
                let session = entry.value();
                session.involves_peer(peer1) && session.involves_peer(peer2)
            })
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn delete(&self, id: &SessionId) -> Result<()> {
        self.sessions.remove(id);
        Ok(())
    }

    async fn exists(&self, id: &SessionId) -> Result<bool> {
        Ok(self.sessions.contains_key(id))
    }

    async fn count(&self) -> Result<usize> {
        Ok(self.sessions.len())
    }

    async fn count_active(&self) -> Result<usize> {
        Ok(self
            .sessions
            .iter()
            .filter(|entry| entry.value().is_active())
            .count())
    }

    /// Override of the trait default with a real atomic compare-and-swap.
    ///
    /// `DashMap::entry()` takes a per-shard write lock for the key, so the
    /// load-compare-store sequence below is observed atomically by every
    /// other writer touching the same `SessionId`. Two concurrent
    /// `AcceptSession` messages can no longer both win: exactly one will see
    /// the matching version and commit; the other will get `Conflict` and
    /// can retry against the freshly-updated state.
    async fn update_if_unchanged(
        &self,
        session: &crate::domain::Session,
        expected_version: u64,
    ) -> Result<()> {
        use dashmap::mapref::entry::Entry;
        let id = session.id().clone();
        match self.sessions.entry(id.clone()) {
            Entry::Vacant(_) => Err(crate::error::Error::NotFound(format!(
                "Session {}",
                id.as_uuid()
            ))),
            Entry::Occupied(mut occ) => {
                let current_version = occ.get().version();
                if current_version != expected_version {
                    return Err(crate::error::Error::Conflict(format!(
                        "Session {} version mismatch: expected {}, found {}",
                        id.as_uuid(),
                        expected_version,
                        current_version
                    )));
                }
                occ.insert(session.clone());
                Ok(())
            }
        }
    }
}

/// In-memory connection repository
#[derive(Clone)]
pub struct InMemoryConnectionRepository {
    connections: Arc<DashMap<SessionId, Connection>>,
}

impl InMemoryConnectionRepository {
    /// Create a new in-memory connection repository
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
        }
    }

    /// Get all connections (for testing/debugging)
    pub fn all(&self) -> Vec<Connection> {
        self.connections
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Clear all connections (for testing)
    pub fn clear(&self) {
        self.connections.clear();
    }
}

impl Default for InMemoryConnectionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConnectionRepository for InMemoryConnectionRepository {
    async fn save(&self, connection: &Connection) -> Result<()> {
        self.connections
            .insert(connection.session_id().clone(), connection.clone());
        Ok(())
    }

    async fn find_by_session(&self, session_id: &SessionId) -> Result<Option<Connection>> {
        Ok(self
            .connections
            .get(session_id)
            .map(|entry| entry.value().clone()))
    }

    async fn find_active(&self) -> Result<Vec<Connection>> {
        Ok(self
            .connections
            .iter()
            .filter(|entry| entry.value().is_active())
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn find_by_peer(&self, peer_id: &PeerId) -> Result<Vec<Connection>> {
        Ok(self
            .connections
            .iter()
            .filter(|entry| entry.value().involves_peer(peer_id))
            .map(|entry| entry.value().clone())
            .collect())
    }

    async fn delete(&self, session_id: &SessionId) -> Result<()> {
        self.connections.remove(session_id);
        Ok(())
    }

    async fn exists(&self, session_id: &SessionId) -> Result<bool> {
        Ok(self.connections.contains_key(session_id))
    }

    async fn count(&self) -> Result<usize> {
        Ok(self.connections.len())
    }

    async fn count_active(&self) -> Result<usize> {
        Ok(self
            .connections
            .iter()
            .filter(|entry| entry.value().is_active())
            .count())
    }

    async fn count_established(&self) -> Result<usize> {
        Ok(self
            .connections
            .iter()
            .filter(|entry| entry.value().is_established())
            .count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NatType, PeerCapabilities};

    #[tokio::test]
    async fn test_peer_repository_operations() {
        let repo = InMemoryPeerRepository::new();
        let peer_id = PeerId::new();
        let capabilities = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let peer = Peer::new(peer_id.clone(), capabilities);

        // Save
        repo.save(&peer).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        // Find
        let found = repo.find_by_id(&peer_id).await.unwrap();
        assert!(found.is_some());

        // Exists
        assert!(repo.exists(&peer_id).await.unwrap());

        // Delete
        repo.delete(&peer_id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_session_repository_operations() {
        let repo = InMemorySessionRepository::new();
        let peer1 = PeerId::new();
        let peer2 = PeerId::new();
        let session = Session::new(peer1.clone(), peer2.clone());
        let session_id = session.id().clone();

        // Save
        repo.save(&session).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        // Find by ID
        let found = repo.find_by_id(&session_id).await.unwrap();
        assert!(found.is_some());

        // Find by peer
        let peer_sessions = repo.find_by_peer(&peer1).await.unwrap();
        assert_eq!(peer_sessions.len(), 1);

        // Delete
        repo.delete(&session_id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_connection_repository_operations() {
        let repo = InMemoryConnectionRepository::new();
        let session_id = SessionId::new();
        let peer1 = PeerId::new();
        let peer2 = PeerId::new();
        let connection = Connection::new(
            session_id.clone(),
            peer1.clone(),
            peer2.clone(),
            NatType::FullCone,
            NatType::FullCone,
        );

        // Save
        repo.save(&connection).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        // Find by session
        let found = repo.find_by_session(&session_id).await.unwrap();
        assert!(found.is_some());

        // Find by peer
        let peer_connections = repo.find_by_peer(&peer1).await.unwrap();
        assert_eq!(peer_connections.len(), 1);

        // Delete
        repo.delete(&session_id).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_repository_clear() {
        let peer_repo = InMemoryPeerRepository::new();
        let session_repo = InMemorySessionRepository::new();
        let conn_repo = InMemoryConnectionRepository::new();

        // Add some data
        let capabilities = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string());
        let peer = Peer::new(PeerId::new(), capabilities);
        peer_repo.save(&peer).await.unwrap();

        let session = Session::new(PeerId::new(), PeerId::new());
        session_repo.save(&session).await.unwrap();

        let connection = Connection::new(
            SessionId::new(),
            PeerId::new(),
            PeerId::new(),
            NatType::FullCone,
            NatType::FullCone,
        );
        conn_repo.save(&connection).await.unwrap();

        // Clear
        peer_repo.clear();
        session_repo.clear();
        conn_repo.clear();

        // Verify empty
        assert_eq!(peer_repo.count().await.unwrap(), 0);
        assert_eq!(session_repo.count().await.unwrap(), 0);
        assert_eq!(conn_repo.count().await.unwrap(), 0);
    }
}

// Made with Bob
