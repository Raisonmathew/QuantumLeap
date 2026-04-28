//! Peer Service - Application Layer
//!
//! Orchestrates peer-related use cases and business workflows.
//! This service coordinates between the domain layer and infrastructure.

use crate::domain::{
    IceCandidate, NatType, Peer, PeerCapabilities, PeerId, PeerState,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Error types for peer service operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum PeerServiceError {
    #[error("Peer not found: {0}")]
    PeerNotFound(PeerId),
    
    #[error("Peer already exists: {0}")]
    PeerAlreadyExists(PeerId),
    
    #[error("Invalid peer state: expected {expected:?}, got {actual:?}")]
    InvalidState {
        expected: PeerState,
        actual: PeerState,
    },
    
    #[error("Peer timed out: {0}")]
    PeerTimedOut(PeerId),
    
    #[error("Incompatible peers: {0} and {1}")]
    IncompatiblePeers(PeerId, PeerId),
}

pub type Result<T> = std::result::Result<T, PeerServiceError>;

/// Peer registration request
#[derive(Debug, Clone)]
pub struct RegisterPeerRequest {
    pub peer_id: PeerId,
    pub nat_type: NatType,
    pub client_version: String,
    pub signaling_address: SocketAddr,
}

/// Peer service - manages peer lifecycle and operations
pub struct PeerService {
    /// In-memory peer registry (in production, this would be a repository)
    peers: Arc<RwLock<HashMap<PeerId, Peer>>>,
    /// Timeout duration for inactive peers
    timeout_duration: Duration,
}

impl PeerService {
    /// Create a new peer service
    pub fn new(timeout_duration: Duration) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration,
        }
    }

    /// Register a new peer
    pub fn register_peer(&self, request: RegisterPeerRequest) -> Result<PeerId> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        // Check if peer already exists
        if peers.contains_key(&request.peer_id) {
            return Err(PeerServiceError::PeerAlreadyExists(request.peer_id));
        }

        // Create peer with capabilities
        let capabilities = PeerCapabilities::new(request.nat_type, request.client_version);
        let mut peer = Peer::new(request.peer_id.clone(), capabilities);
        peer.set_signaling_address(request.signaling_address);

        // Store peer
        let peer_id = peer.id().clone();
        peers.insert(peer_id.clone(), peer);

        Ok(peer_id)
    }

    /// Register a peer with full capabilities
    pub fn register_peer_with_capabilities(
        &self,
        peer_id: PeerId,
        capabilities: PeerCapabilities,
        signaling_address: SocketAddr,
    ) -> Result<PeerId> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        if peers.contains_key(&peer_id) {
            return Err(PeerServiceError::PeerAlreadyExists(peer_id));
        }

        let mut peer = Peer::new(peer_id.clone(), capabilities);
        peer.set_signaling_address(signaling_address);

        let id = peer.id().clone();
        peers.insert(id.clone(), peer);

        Ok(id)
    }

    /// Connect a peer (transition to connected state)
    pub fn connect_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.connect();
        Ok(())
    }

    /// Disconnect a peer
    pub fn disconnect_peer(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.disconnect();
        Ok(())
    }

    /// Remove a peer from the registry
    pub fn remove_peer(&self, peer_id: &PeerId) -> Result<Peer> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        peers
            .remove(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))
    }

    /// Get a peer by ID
    pub fn get_peer(&self, peer_id: &PeerId) -> Result<Peer> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());

        peers
            .get(peer_id)
            .cloned()
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))
    }

    /// Add ICE candidate to a peer
    pub fn add_ice_candidate(&self, peer_id: &PeerId, candidate: IceCandidate) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.add_ice_candidate(candidate);
        Ok(())
    }

    /// Add multiple ICE candidates to a peer
    pub fn add_ice_candidates(
        &self,
        peer_id: &PeerId,
        candidates: Vec<IceCandidate>,
    ) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.add_ice_candidates(candidates);
        Ok(())
    }

    /// Get ICE candidates for a peer
    pub fn get_ice_candidates(&self, peer_id: &PeerId) -> Result<Vec<IceCandidate>> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        Ok(peer.ice_candidates().to_vec())
    }

    /// Check if two peers can connect
    pub fn can_peers_connect(&self, peer1_id: &PeerId, peer2_id: &PeerId) -> Result<bool> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());

        let peer1 = peers
            .get(peer1_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer1_id.clone()))?;

        let peer2 = peers
            .get(peer2_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer2_id.clone()))?;

        Ok(peer1.can_connect_to(peer2))
    }

    /// Get all active peers
    pub fn get_active_peers(&self) -> Vec<Peer> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());
        peers
            .values()
            .filter(|p| p.is_active())
            .cloned()
            .collect()
    }

    /// Get all connected peers
    pub fn get_connected_peers(&self) -> Vec<Peer> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());
        peers
            .values()
            .filter(|p| p.is_connected())
            .cloned()
            .collect()
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());
        peers.len()
    }

    /// Get active peer count
    pub fn active_peer_count(&self) -> usize {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());
        peers.values().filter(|p| p.is_active()).count()
    }

    /// Update peer activity timestamp
    pub fn update_peer_activity(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.update_activity();
        Ok(())
    }

    /// Increment session count for a peer
    pub fn increment_peer_sessions(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.increment_sessions();
        Ok(())
    }

    /// Decrement session count for a peer
    pub fn decrement_peer_sessions(&self, peer_id: &PeerId) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.decrement_sessions();
        Ok(())
    }

    /// Add bytes transferred for a peer
    pub fn add_peer_bytes_transferred(&self, peer_id: &PeerId, bytes: u64) -> Result<()> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get_mut(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        peer.add_bytes_transferred(bytes);
        Ok(())
    }

    /// Clean up timed out peers
    pub fn cleanup_timed_out_peers(&self) -> Vec<PeerId> {
        let mut peers = self.peers.write().unwrap_or_else(|p| p.into_inner());
        let mut timed_out = Vec::new();

        // Find timed out peers
        let to_remove: Vec<PeerId> = peers
            .iter()
            .filter(|(_, peer)| peer.is_timed_out(self.timeout_duration))
            .map(|(id, _)| id.clone())
            .collect();

        // Remove them
        for peer_id in to_remove {
            if let Some(mut peer) = peers.remove(&peer_id) {
                peer.fail();
                timed_out.push(peer_id);
            }
        }

        timed_out
    }

    /// Find compatible peers for a given peer
    pub fn find_compatible_peers(&self, peer_id: &PeerId) -> Result<Vec<PeerId>> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());

        let source_peer = peers
            .get(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        let compatible: Vec<PeerId> = peers
            .iter()
            .filter(|(id, peer)| {
                *id != peer_id && peer.is_connected() && source_peer.can_connect_to(peer)
            })
            .map(|(id, _)| id.clone())
            .collect();

        Ok(compatible)
    }

    /// Get peer statistics
    pub fn get_peer_stats(&self, peer_id: &PeerId) -> Result<PeerStats> {
        let peers = self.peers.read().unwrap_or_else(|p| p.into_inner());

        let peer = peers
            .get(peer_id)
            .ok_or_else(|| PeerServiceError::PeerNotFound(peer_id.clone()))?;

        Ok(PeerStats {
            peer_id: peer.id().clone(),
            state: peer.state(),
            active_sessions: peer.active_sessions(),
            bytes_transferred: peer.bytes_transferred(),
            ice_candidate_count: peer.ice_candidates().len(),
            connection_duration: peer.connection_duration(),
        })
    }
}

/// Peer statistics
#[derive(Debug, Clone)]
pub struct PeerStats {
    pub peer_id: PeerId,
    pub state: PeerState,
    pub active_sessions: usize,
    pub bytes_transferred: u64,
    pub ice_candidate_count: usize,
    pub connection_duration: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::nat_type::NatType;
    use crate::domain::peer_capabilities::TransportProtocol;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_service() -> PeerService {
        PeerService::new(Duration::from_secs(30))
    }

    fn create_test_request() -> RegisterPeerRequest {
        RegisterPeerRequest {
            peer_id: PeerId::new(),
            nat_type: NatType::Open,
            client_version: "1.0.0".to_string(),
            signaling_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        }
    }

    #[test]
    fn test_register_peer() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = request.peer_id.clone();

        let result = service.register_peer(request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), peer_id);
        assert_eq!(service.peer_count(), 1);
    }

    #[test]
    fn test_register_duplicate_peer() {
        let service = create_test_service();
        let request = create_test_request();

        service.register_peer(request.clone()).unwrap();
        let result = service.register_peer(request);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PeerServiceError::PeerAlreadyExists(_)
        ));
    }

    #[test]
    fn test_connect_peer() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        service.connect_peer(&peer_id).unwrap();

        let peer = service.get_peer(&peer_id).unwrap();
        assert!(peer.is_connected());
    }

    #[test]
    fn test_disconnect_peer() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        service.connect_peer(&peer_id).unwrap();
        service.disconnect_peer(&peer_id).unwrap();

        let peer = service.get_peer(&peer_id).unwrap();
        assert_eq!(peer.state(), PeerState::Disconnecting);
    }

    #[test]
    fn test_remove_peer() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        let removed = service.remove_peer(&peer_id);
        assert!(removed.is_ok());
        assert_eq!(service.peer_count(), 0);
    }

    #[test]
    fn test_add_ice_candidate() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());

        service.add_ice_candidate(&peer_id, candidate).unwrap();

        let candidates = service.get_ice_candidates(&peer_id).unwrap();
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn test_can_peers_connect() {
        let service = create_test_service();

        // Create peers with compatible capabilities (both support TCP and P2P)
        let peer1_id = PeerId::new();
        let caps1 = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        service.register_peer_with_capabilities(peer1_id.clone(), caps1, addr1).unwrap();
        service.connect_peer(&peer1_id).unwrap();

        let peer2_id = PeerId::new();
        let caps2 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        service.register_peer_with_capabilities(peer2_id.clone(), caps2, addr2).unwrap();
        service.connect_peer(&peer2_id).unwrap();

        let can_connect = service.can_peers_connect(&peer1_id, &peer2_id).unwrap();
        assert!(can_connect);
    }

    #[test]
    fn test_get_active_peers() {
        let service = create_test_service();

        let mut request1 = create_test_request();
        request1.peer_id = PeerId::new();
        let peer1_id = service.register_peer(request1).unwrap();
        service.connect_peer(&peer1_id).unwrap();

        let mut request2 = create_test_request();
        request2.peer_id = PeerId::new();
        service.register_peer(request2).unwrap();

        let active = service.get_active_peers();
        assert_eq!(active.len(), 2); // Both connecting and connected are active
    }

    #[test]
    fn test_get_connected_peers() {
        let service = create_test_service();

        let mut request1 = create_test_request();
        request1.peer_id = PeerId::new();
        let peer1_id = service.register_peer(request1).unwrap();
        service.connect_peer(&peer1_id).unwrap();

        let mut request2 = create_test_request();
        request2.peer_id = PeerId::new();
        service.register_peer(request2).unwrap();

        let connected = service.get_connected_peers();
        assert_eq!(connected.len(), 1);
    }

    #[test]
    fn test_peer_sessions() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        service.increment_peer_sessions(&peer_id).unwrap();
        service.increment_peer_sessions(&peer_id).unwrap();

        let peer = service.get_peer(&peer_id).unwrap();
        assert_eq!(peer.active_sessions(), 2);

        service.decrement_peer_sessions(&peer_id).unwrap();
        let peer = service.get_peer(&peer_id).unwrap();
        assert_eq!(peer.active_sessions(), 1);
    }

    #[test]
    fn test_peer_stats() {
        let service = create_test_service();
        let request = create_test_request();
        let peer_id = service.register_peer(request).unwrap();

        service.connect_peer(&peer_id).unwrap();
        service.add_peer_bytes_transferred(&peer_id, 1000).unwrap();

        let stats = service.get_peer_stats(&peer_id).unwrap();
        assert_eq!(stats.state, PeerState::Connected);
        assert_eq!(stats.bytes_transferred, 1000);
    }
}

// Made with Bob
