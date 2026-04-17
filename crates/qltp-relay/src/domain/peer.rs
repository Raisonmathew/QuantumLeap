//! Peer Entity
//!
//! Represents a peer in the relay network. This is an entity in DDD terms -
//! it has identity (PeerId) and lifecycle, and can change state over time.

use super::ice_candidate::IceCandidate;
use super::peer_capabilities::PeerCapabilities;
use super::peer_id::PeerId;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

/// Peer connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    /// Peer is connecting
    Connecting,
    /// Peer is connected and ready
    Connected,
    /// Peer is disconnecting
    Disconnecting,
    /// Peer is disconnected
    Disconnected,
    /// Peer connection failed
    Failed,
}

impl PeerState {
    /// Check if peer is in an active state
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            PeerState::Connecting | PeerState::Connected | PeerState::Disconnecting
        )
    }

    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        matches!(self, PeerState::Connected)
    }

    /// Check if peer is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, PeerState::Disconnected | PeerState::Failed)
    }
}

/// Peer entity - represents a connected peer in the relay network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    /// Unique peer identifier
    id: PeerId,
    /// Peer capabilities
    capabilities: PeerCapabilities,
    /// Current connection state
    state: PeerState,
    /// ICE candidates for this peer
    ice_candidates: Vec<IceCandidate>,
    /// WebSocket connection address (if connected via signaling server)
    signaling_address: Option<SocketAddr>,
    /// Time when peer connected
    connected_at: SystemTime,
    /// Time of last activity
    last_activity: SystemTime,
    /// Number of active sessions
    active_sessions: usize,
    /// Total bytes transferred
    bytes_transferred: u64,
    /// Metadata (custom key-value pairs)
    metadata: std::collections::HashMap<String, String>,
}

impl Peer {
    /// Create a new peer
    pub fn new(id: PeerId, capabilities: PeerCapabilities) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            capabilities,
            state: PeerState::Connecting,
            ice_candidates: Vec::new(),
            signaling_address: None,
            connected_at: now,
            last_activity: now,
            active_sessions: 0,
            bytes_transferred: 0,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Get peer ID
    pub fn id(&self) -> &PeerId {
        &self.id
    }

    /// Get peer capabilities
    pub fn capabilities(&self) -> &PeerCapabilities {
        &self.capabilities
    }

    /// Get current state
    pub fn state(&self) -> PeerState {
        self.state
    }

    /// Get ICE candidates
    pub fn ice_candidates(&self) -> &[IceCandidate] {
        &self.ice_candidates
    }

    /// Get signaling address
    pub fn signaling_address(&self) -> Option<SocketAddr> {
        self.signaling_address
    }

    /// Get connection time
    pub fn connected_at(&self) -> SystemTime {
        self.connected_at
    }

    /// Get last activity time
    pub fn last_activity(&self) -> SystemTime {
        self.last_activity
    }

    /// Get number of active sessions
    pub fn active_sessions(&self) -> usize {
        self.active_sessions
    }

    /// Get total bytes transferred
    pub fn bytes_transferred(&self) -> u64 {
        self.bytes_transferred
    }

    /// Get metadata
    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }

    /// Check if peer is active
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.state.is_connected()
    }

    /// Check if peer has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        if let Ok(elapsed) = self.last_activity.elapsed() {
            elapsed > timeout
        } else {
            false
        }
    }

    /// Get connection duration
    pub fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.elapsed().ok()
    }

    /// Set signaling address
    pub fn set_signaling_address(&mut self, address: SocketAddr) {
        self.signaling_address = Some(address);
    }

    /// Add ICE candidate
    pub fn add_ice_candidate(&mut self, candidate: IceCandidate) {
        // Avoid duplicates
        if !self.ice_candidates.contains(&candidate) {
            self.ice_candidates.push(candidate);
        }
    }

    /// Add multiple ICE candidates
    pub fn add_ice_candidates(&mut self, candidates: Vec<IceCandidate>) {
        for candidate in candidates {
            self.add_ice_candidate(candidate);
        }
    }

    /// Clear ICE candidates
    pub fn clear_ice_candidates(&mut self) {
        self.ice_candidates.clear();
    }

    /// Get direct (non-relay) ICE candidates
    pub fn direct_candidates(&self) -> Vec<&IceCandidate> {
        self.ice_candidates
            .iter()
            .filter(|c| c.is_direct())
            .collect()
    }

    /// Get relay ICE candidates
    pub fn relay_candidates(&self) -> Vec<&IceCandidate> {
        self.ice_candidates
            .iter()
            .filter(|c| c.requires_relay())
            .collect()
    }

    /// Transition to connected state
    pub fn connect(&mut self) {
        if self.state == PeerState::Connecting {
            self.state = PeerState::Connected;
            self.last_activity = SystemTime::now();
        }
    }

    /// Transition to disconnecting state
    pub fn disconnect(&mut self) {
        if self.state.is_active() {
            self.state = PeerState::Disconnecting;
            self.last_activity = SystemTime::now();
        }
    }

    /// Transition to disconnected state
    pub fn disconnected(&mut self) {
        self.state = PeerState::Disconnected;
        self.last_activity = SystemTime::now();
    }

    /// Transition to failed state
    pub fn fail(&mut self) {
        self.state = PeerState::Failed;
        self.last_activity = SystemTime::now();
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Increment active sessions
    pub fn increment_sessions(&mut self) {
        self.active_sessions += 1;
        self.update_activity();
    }

    /// Decrement active sessions
    pub fn decrement_sessions(&mut self) {
        if self.active_sessions > 0 {
            self.active_sessions -= 1;
        }
        self.update_activity();
    }

    /// Add bytes transferred
    pub fn add_bytes_transferred(&mut self, bytes: u64) {
        self.bytes_transferred = self.bytes_transferred.saturating_add(bytes);
        self.update_activity();
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Remove metadata value
    pub fn remove_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }

    /// Check if peer can connect to another peer
    pub fn can_connect_to(&self, other: &Peer) -> bool {
        // Both must be connected (not just active, as disconnecting peers shouldn't accept new connections)
        if !self.is_connected() || !other.is_connected() {
            return false;
        }

        // Must have compatible capabilities
        self.capabilities.is_compatible_with(&other.capabilities)
    }

    /// Get a summary of peer status
    pub fn status_summary(&self) -> String {
        format!(
            "Peer {} - State: {:?}, Sessions: {}, Candidates: {}, Bytes: {}",
            self.id,
            self.state,
            self.active_sessions,
            self.ice_candidates.len(),
            self.bytes_transferred
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::nat_type::NatType;
    use std::net::{IpAddr, Ipv4Addr};
    use std::thread;
    use std::time::Duration;

    fn create_test_peer() -> Peer {
        let id = PeerId::new();
        let capabilities = PeerCapabilities::new(NatType::Open, "1.0.0".to_string());
        Peer::new(id, capabilities)
    }

    #[test]
    fn test_new_peer() {
        let peer = create_test_peer();

        assert_eq!(peer.state(), PeerState::Connecting);
        assert_eq!(peer.active_sessions(), 0);
        assert_eq!(peer.bytes_transferred(), 0);
        assert!(peer.ice_candidates().is_empty());
    }

    #[test]
    fn test_peer_state_transitions() {
        let mut peer = create_test_peer();

        assert_eq!(peer.state(), PeerState::Connecting);
        assert!(peer.is_active());
        assert!(!peer.is_connected());

        peer.connect();
        assert_eq!(peer.state(), PeerState::Connected);
        assert!(peer.is_active());
        assert!(peer.is_connected());

        peer.disconnect();
        assert_eq!(peer.state(), PeerState::Disconnecting);
        assert!(peer.is_active());

        peer.disconnected();
        assert_eq!(peer.state(), PeerState::Disconnected);
        assert!(!peer.is_active());
    }

    #[test]
    fn test_peer_fail() {
        let mut peer = create_test_peer();
        peer.connect();

        peer.fail();
        assert_eq!(peer.state(), PeerState::Failed);
        assert!(!peer.is_active());
    }

    #[test]
    fn test_ice_candidates() {
        let mut peer = create_test_peer();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());

        peer.add_ice_candidate(candidate.clone());
        assert_eq!(peer.ice_candidates().len(), 1);

        // Adding duplicate should not increase count
        peer.add_ice_candidate(candidate);
        assert_eq!(peer.ice_candidates().len(), 1);
    }

    #[test]
    fn test_direct_and_relay_candidates() {
        let mut peer = create_test_peer();

        let host_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let relay_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 3478);
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);

        peer.add_ice_candidate(IceCandidate::host(host_addr, "f1".to_string()));
        peer.add_ice_candidate(IceCandidate::relay(relay_addr, "f2".to_string(), local_addr));

        assert_eq!(peer.direct_candidates().len(), 1);
        assert_eq!(peer.relay_candidates().len(), 1);
    }

    #[test]
    fn test_session_management() {
        let mut peer = create_test_peer();

        peer.increment_sessions();
        assert_eq!(peer.active_sessions(), 1);

        peer.increment_sessions();
        assert_eq!(peer.active_sessions(), 2);

        peer.decrement_sessions();
        assert_eq!(peer.active_sessions(), 1);

        peer.decrement_sessions();
        assert_eq!(peer.active_sessions(), 0);

        // Should not go negative
        peer.decrement_sessions();
        assert_eq!(peer.active_sessions(), 0);
    }

    #[test]
    fn test_bytes_transferred() {
        let mut peer = create_test_peer();

        peer.add_bytes_transferred(1000);
        assert_eq!(peer.bytes_transferred(), 1000);

        peer.add_bytes_transferred(500);
        assert_eq!(peer.bytes_transferred(), 1500);
    }

    #[test]
    fn test_metadata() {
        let mut peer = create_test_peer();

        peer.set_metadata("region".to_string(), "us-west".to_string());
        assert_eq!(peer.get_metadata("region"), Some(&"us-west".to_string()));

        peer.set_metadata("region".to_string(), "us-east".to_string());
        assert_eq!(peer.get_metadata("region"), Some(&"us-east".to_string()));

        let removed = peer.remove_metadata("region");
        assert_eq!(removed, Some("us-east".to_string()));
        assert_eq!(peer.get_metadata("region"), None);
    }

    #[test]
    fn test_timeout() {
        let mut peer = create_test_peer();

        // Should not be timed out immediately
        assert!(!peer.is_timed_out(Duration::from_secs(1)));

        // Simulate passage of time
        thread::sleep(Duration::from_millis(100));

        // Should be timed out with very short timeout
        assert!(peer.is_timed_out(Duration::from_millis(50)));

        // Update activity
        peer.update_activity();

        // Should not be timed out after activity update
        assert!(!peer.is_timed_out(Duration::from_secs(1)));
    }

    #[test]
    fn test_can_connect_to() {
        use crate::domain::peer_capabilities::TransportProtocol;

        let id1 = PeerId::new();
        let caps1 = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);
        let mut peer1 = Peer::new(id1, caps1);
        peer1.connect();

        let id2 = PeerId::new();
        let caps2 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);
        let mut peer2 = Peer::new(id2, caps2);
        peer2.connect();

        assert!(peer1.can_connect_to(&peer2));
        assert!(peer2.can_connect_to(&peer1));

        // Disconnect one peer
        peer2.disconnect();
        assert!(!peer1.can_connect_to(&peer2));
    }

    #[test]
    fn test_status_summary() {
        let mut peer = create_test_peer();
        peer.connect();
        peer.increment_sessions();

        let summary = peer.status_summary();
        assert!(summary.contains("Connected"));
        assert!(summary.contains("Sessions: 1"));
    }

    #[test]
    fn test_signaling_address() {
        let mut peer = create_test_peer();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        assert_eq!(peer.signaling_address(), None);

        peer.set_signaling_address(addr);
        assert_eq!(peer.signaling_address(), Some(addr));
    }
}

// Made with Bob
