//! ICE Candidate Value Object
//!
//! Represents an ICE (Interactive Connectivity Establishment) candidate
//! for NAT traversal. This is a value object in DDD terms - immutable
//! and defined by its attributes.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;

/// ICE candidate type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandidateType {
    /// Host candidate (local interface)
    Host,
    /// Server reflexive candidate (discovered via STUN)
    ServerReflexive,
    /// Peer reflexive candidate (discovered during connectivity checks)
    PeerReflexive,
    /// Relay candidate (via TURN server)
    Relay,
}

impl CandidateType {
    /// Get priority value for this candidate type
    /// Higher priority = preferred for connection
    pub fn priority(&self) -> u32 {
        match self {
            CandidateType::Host => 126,
            CandidateType::PeerReflexive => 110,
            CandidateType::ServerReflexive => 100,
            CandidateType::Relay => 0,
        }
    }

    /// Check if this candidate type requires a relay server
    pub fn requires_relay(&self) -> bool {
        matches!(self, CandidateType::Relay)
    }

    /// Check if this candidate type is direct (no relay)
    pub fn is_direct(&self) -> bool {
        !self.requires_relay()
    }
}

impl fmt::Display for CandidateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandidateType::Host => write!(f, "host"),
            CandidateType::ServerReflexive => write!(f, "srflx"),
            CandidateType::PeerReflexive => write!(f, "prflx"),
            CandidateType::Relay => write!(f, "relay"),
        }
    }
}

/// ICE Candidate - represents a potential connection endpoint
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IceCandidate {
    /// Socket address (IP:port)
    address: SocketAddr,
    /// Candidate type
    candidate_type: CandidateType,
    /// Priority (higher = more preferred)
    priority: u32,
    /// Foundation (identifies candidates from same interface)
    foundation: String,
    /// Component ID (1 = RTP, 2 = RTCP)
    component: u16,
    /// Related address (for reflexive/relay candidates)
    related_address: Option<SocketAddr>,
}

impl IceCandidate {
    /// Create a new ICE candidate
    pub fn new(
        address: SocketAddr,
        candidate_type: CandidateType,
        foundation: String,
        component: u16,
        related_address: Option<SocketAddr>,
    ) -> Self {
        // Calculate priority based on type and component
        let type_preference = candidate_type.priority();
        let priority = (type_preference << 24) | ((component as u32) << 16);

        Self {
            address,
            candidate_type,
            priority,
            foundation,
            component,
            related_address,
        }
    }

    /// Create a host candidate
    pub fn host(address: SocketAddr, foundation: String) -> Self {
        Self::new(address, CandidateType::Host, foundation, 1, None)
    }

    /// Create a server reflexive candidate
    pub fn server_reflexive(
        address: SocketAddr,
        foundation: String,
        related_address: SocketAddr,
    ) -> Self {
        Self::new(
            address,
            CandidateType::ServerReflexive,
            foundation,
            1,
            Some(related_address),
        )
    }

    /// Create a relay candidate
    pub fn relay(address: SocketAddr, foundation: String, related_address: SocketAddr) -> Self {
        Self::new(
            address,
            CandidateType::Relay,
            foundation,
            1,
            Some(related_address),
        )
    }

    /// Get the socket address
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Get the candidate type
    pub fn candidate_type(&self) -> CandidateType {
        self.candidate_type
    }

    /// Get the priority
    pub fn priority(&self) -> u32 {
        self.priority
    }

    /// Get the foundation
    pub fn foundation(&self) -> &str {
        &self.foundation
    }

    /// Get the component ID
    pub fn component(&self) -> u16 {
        self.component
    }

    /// Get the related address
    pub fn related_address(&self) -> Option<SocketAddr> {
        self.related_address
    }

    /// Check if this candidate requires a relay
    pub fn requires_relay(&self) -> bool {
        self.candidate_type.requires_relay()
    }

    /// Check if this candidate is direct (no relay)
    pub fn is_direct(&self) -> bool {
        self.candidate_type.is_direct()
    }

    /// Convert to ICE candidate string format
    pub fn to_ice_string(&self) -> String {
        let mut s = format!(
            "candidate:{} {} {} {} {} {} typ {}",
            self.foundation,
            self.component,
            "udp", // protocol
            self.priority,
            self.address.ip(),
            self.address.port(),
            self.candidate_type
        );

        if let Some(related) = self.related_address {
            s.push_str(&format!(
                " raddr {} rport {}",
                related.ip(),
                related.port()
            ));
        }

        s
    }
}

impl fmt::Display for IceCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} ({})",
            self.address.ip(),
            self.address.port(),
            self.candidate_type
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_candidate_type_priority() {
        assert_eq!(CandidateType::Host.priority(), 126);
        assert_eq!(CandidateType::PeerReflexive.priority(), 110);
        assert_eq!(CandidateType::ServerReflexive.priority(), 100);
        assert_eq!(CandidateType::Relay.priority(), 0);
    }

    #[test]
    fn test_candidate_type_requires_relay() {
        assert!(!CandidateType::Host.requires_relay());
        assert!(!CandidateType::ServerReflexive.requires_relay());
        assert!(!CandidateType::PeerReflexive.requires_relay());
        assert!(CandidateType::Relay.requires_relay());
    }

    #[test]
    fn test_host_candidate() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());

        assert_eq!(candidate.address(), addr);
        assert_eq!(candidate.candidate_type(), CandidateType::Host);
        assert_eq!(candidate.foundation(), "foundation1");
        assert_eq!(candidate.component(), 1);
        assert!(candidate.related_address().is_none());
        assert!(candidate.is_direct());
        assert!(!candidate.requires_relay());
    }

    #[test]
    fn test_server_reflexive_candidate() {
        let public_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 12345);
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate =
            IceCandidate::server_reflexive(public_addr, "foundation2".to_string(), local_addr);

        assert_eq!(candidate.address(), public_addr);
        assert_eq!(candidate.candidate_type(), CandidateType::ServerReflexive);
        assert_eq!(candidate.related_address(), Some(local_addr));
        assert!(candidate.is_direct());
    }

    #[test]
    fn test_relay_candidate() {
        let relay_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 2)), 3478);
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::relay(relay_addr, "foundation3".to_string(), local_addr);

        assert_eq!(candidate.address(), relay_addr);
        assert_eq!(candidate.candidate_type(), CandidateType::Relay);
        assert_eq!(candidate.related_address(), Some(local_addr));
        assert!(!candidate.is_direct());
        assert!(candidate.requires_relay());
    }

    #[test]
    fn test_ice_string_format() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());
        let ice_string = candidate.to_ice_string();

        assert!(ice_string.contains("candidate:foundation1"));
        assert!(ice_string.contains("192.168.1.100"));
        assert!(ice_string.contains("8080"));
        assert!(ice_string.contains("typ host"));
    }

    #[test]
    fn test_candidate_display() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());
        let display = format!("{}", candidate);

        assert_eq!(display, "192.168.1.100:8080 (host)");
    }

    #[test]
    fn test_candidate_serialization() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let candidate = IceCandidate::host(addr, "foundation1".to_string());

        let json = serde_json::to_string(&candidate).unwrap();
        let deserialized: IceCandidate = serde_json::from_str(&json).unwrap();

        assert_eq!(candidate, deserialized);
    }
}

// Made with Bob
