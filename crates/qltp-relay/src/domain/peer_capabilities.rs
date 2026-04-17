//! Peer Capabilities Value Object
//!
//! Represents the capabilities and features supported by a peer.
//! This is a value object in DDD terms - immutable and defined by its attributes.

use super::nat_type::NatType;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Transport protocol support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    /// TCP transport
    Tcp,
    /// QUIC transport
    Quic,
    /// UDP transport
    Udp,
    /// WebRTC data channels
    WebRtc,
}

/// Peer capabilities - what features and protocols a peer supports
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerCapabilities {
    /// NAT type of the peer
    nat_type: NatType,
    /// Supported transport protocols
    transports: HashSet<TransportProtocol>,
    /// Whether peer supports direct P2P connections
    supports_p2p: bool,
    /// Whether peer can act as a relay
    can_relay: bool,
    /// Maximum bandwidth in bytes per second (0 = unlimited)
    max_bandwidth: u64,
    /// Whether peer supports IPv6
    supports_ipv6: bool,
    /// Whether peer supports compression
    supports_compression: bool,
    /// Whether peer supports Forward Error Correction
    supports_fec: bool,
    /// Client version string
    client_version: String,
}

impl PeerCapabilities {
    /// Create new peer capabilities
    pub fn new(nat_type: NatType, client_version: String) -> Self {
        Self {
            nat_type,
            transports: HashSet::new(),
            supports_p2p: true,
            can_relay: false,
            max_bandwidth: 0,
            supports_ipv6: false,
            supports_compression: false,
            supports_fec: false,
            client_version,
        }
    }

    /// Create capabilities with all features enabled
    pub fn full_featured(nat_type: NatType, client_version: String) -> Self {
        let mut transports = HashSet::new();
        transports.insert(TransportProtocol::Tcp);
        transports.insert(TransportProtocol::Quic);
        transports.insert(TransportProtocol::Udp);
        transports.insert(TransportProtocol::WebRtc);

        Self {
            nat_type,
            transports,
            supports_p2p: true,
            can_relay: true,
            max_bandwidth: 0,
            supports_ipv6: true,
            supports_compression: true,
            supports_fec: true,
            client_version,
        }
    }

    /// Create minimal capabilities (TCP only, no advanced features)
    pub fn minimal(nat_type: NatType, client_version: String) -> Self {
        let mut transports = HashSet::new();
        transports.insert(TransportProtocol::Tcp);

        Self {
            nat_type,
            transports,
            supports_p2p: false,
            can_relay: false,
            max_bandwidth: 0,
            supports_ipv6: false,
            supports_compression: false,
            supports_fec: false,
            client_version,
        }
    }

    /// Get NAT type
    pub fn nat_type(&self) -> NatType {
        self.nat_type
    }

    /// Get supported transports
    pub fn transports(&self) -> &HashSet<TransportProtocol> {
        &self.transports
    }

    /// Check if peer supports a specific transport
    pub fn supports_transport(&self, transport: TransportProtocol) -> bool {
        self.transports.contains(&transport)
    }

    /// Check if peer supports P2P
    pub fn supports_p2p(&self) -> bool {
        self.supports_p2p
    }

    /// Check if peer can act as relay
    pub fn can_relay(&self) -> bool {
        self.can_relay
    }

    /// Get maximum bandwidth
    pub fn max_bandwidth(&self) -> u64 {
        self.max_bandwidth
    }

    /// Check if peer supports IPv6
    pub fn supports_ipv6(&self) -> bool {
        self.supports_ipv6
    }

    /// Check if peer supports compression
    pub fn supports_compression(&self) -> bool {
        self.supports_compression
    }

    /// Check if peer supports FEC
    pub fn supports_fec(&self) -> bool {
        self.supports_fec
    }

    /// Get client version
    pub fn client_version(&self) -> &str {
        &self.client_version
    }

    /// Add transport support
    pub fn with_transport(mut self, transport: TransportProtocol) -> Self {
        self.transports.insert(transport);
        self
    }

    /// Enable P2P support
    pub fn with_p2p(mut self, enabled: bool) -> Self {
        self.supports_p2p = enabled;
        self
    }

    /// Enable relay capability
    pub fn with_relay(mut self, enabled: bool) -> Self {
        self.can_relay = enabled;
        self
    }

    /// Set maximum bandwidth
    pub fn with_max_bandwidth(mut self, bandwidth: u64) -> Self {
        self.max_bandwidth = bandwidth;
        self
    }

    /// Enable IPv6 support
    pub fn with_ipv6(mut self, enabled: bool) -> Self {
        self.supports_ipv6 = enabled;
        self
    }

    /// Enable compression support
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.supports_compression = enabled;
        self
    }

    /// Enable FEC support
    pub fn with_fec(mut self, enabled: bool) -> Self {
        self.supports_fec = enabled;
        self
    }

    /// Check if two peers have compatible capabilities for direct connection
    pub fn is_compatible_with(&self, other: &PeerCapabilities) -> bool {
        // Must have at least one common transport
        let has_common_transport = self
            .transports
            .intersection(&other.transports)
            .next()
            .is_some();

        // Both must support P2P for direct connection
        let both_support_p2p = self.supports_p2p && other.supports_p2p;

        has_common_transport && both_support_p2p
    }

    /// Get common transports with another peer
    pub fn common_transports(&self, other: &PeerCapabilities) -> HashSet<TransportProtocol> {
        self.transports
            .intersection(&other.transports)
            .copied()
            .collect()
    }

    /// Check if capabilities are sufficient for high-performance transfer
    pub fn is_high_performance(&self) -> bool {
        // High performance requires QUIC or UDP, compression, and FEC
        let has_fast_transport = self.supports_transport(TransportProtocol::Quic)
            || self.supports_transport(TransportProtocol::Udp);

        has_fast_transport && self.supports_compression && self.supports_fec
    }

    /// Get a score representing overall capability level (0-100)
    pub fn capability_score(&self) -> u8 {
        let mut score = 0u8;

        // Transport protocols (max 30 points)
        score += (self.transports.len() as u8 * 7).min(30);

        // Features (10 points each)
        if self.supports_p2p {
            score += 10;
        }
        if self.can_relay {
            score += 10;
        }
        if self.supports_ipv6 {
            score += 10;
        }
        if self.supports_compression {
            score += 10;
        }
        if self.supports_fec {
            score += 10;
        }

        // NAT type bonus (max 20 points)
        score += match self.nat_type {
            NatType::Open => 20,
            NatType::FullCone => 15,
            NatType::RestrictedCone => 10,
            NatType::PortRestricted => 5,
            NatType::Symmetric => 0,
            NatType::Unknown => 0,
        };

        score.min(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_capabilities() {
        let caps = PeerCapabilities::new(NatType::Open, "1.0.0".to_string());

        assert_eq!(caps.nat_type(), NatType::Open);
        assert_eq!(caps.client_version(), "1.0.0");
        assert!(caps.transports().is_empty());
        assert!(caps.supports_p2p());
        assert!(!caps.can_relay());
    }

    #[test]
    fn test_full_featured_capabilities() {
        let caps = PeerCapabilities::full_featured(NatType::Open, "1.0.0".to_string());

        assert_eq!(caps.transports().len(), 4);
        assert!(caps.supports_transport(TransportProtocol::Tcp));
        assert!(caps.supports_transport(TransportProtocol::Quic));
        assert!(caps.supports_transport(TransportProtocol::Udp));
        assert!(caps.supports_transport(TransportProtocol::WebRtc));
        assert!(caps.supports_p2p());
        assert!(caps.can_relay());
        assert!(caps.supports_ipv6());
        assert!(caps.supports_compression());
        assert!(caps.supports_fec());
    }

    #[test]
    fn test_minimal_capabilities() {
        let caps = PeerCapabilities::minimal(NatType::Symmetric, "1.0.0".to_string());

        assert_eq!(caps.transports().len(), 1);
        assert!(caps.supports_transport(TransportProtocol::Tcp));
        assert!(!caps.supports_p2p());
        assert!(!caps.can_relay());
        assert!(!caps.supports_compression());
    }

    #[test]
    fn test_builder_pattern() {
        let caps = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Quic)
            .with_transport(TransportProtocol::Tcp)
            .with_compression(true)
            .with_fec(true)
            .with_max_bandwidth(1_000_000_000);

        assert_eq!(caps.transports().len(), 2);
        assert!(caps.supports_compression());
        assert!(caps.supports_fec());
        assert_eq!(caps.max_bandwidth(), 1_000_000_000);
    }

    #[test]
    fn test_compatibility() {
        let caps1 = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Quic)
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);

        let caps2 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);

        assert!(caps1.is_compatible_with(&caps2));

        let common = caps1.common_transports(&caps2);
        assert_eq!(common.len(), 1);
        assert!(common.contains(&TransportProtocol::Tcp));
    }

    #[test]
    fn test_incompatibility_no_common_transport() {
        let caps1 = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Quic)
            .with_p2p(true);

        let caps2 = PeerCapabilities::new(NatType::FullCone, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);

        assert!(!caps1.is_compatible_with(&caps2));
    }

    #[test]
    fn test_incompatibility_no_p2p() {
        let caps1 = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(true);

        let caps2 = PeerCapabilities::new(NatType::Symmetric, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp)
            .with_p2p(false);

        assert!(!caps1.is_compatible_with(&caps2));
    }

    #[test]
    fn test_high_performance() {
        let high_perf = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Quic)
            .with_compression(true)
            .with_fec(true);

        assert!(high_perf.is_high_performance());

        let low_perf = PeerCapabilities::new(NatType::Open, "1.0.0".to_string())
            .with_transport(TransportProtocol::Tcp);

        assert!(!low_perf.is_high_performance());
    }

    #[test]
    fn test_capability_score() {
        let full = PeerCapabilities::full_featured(NatType::Open, "1.0.0".to_string());
        let minimal = PeerCapabilities::minimal(NatType::Symmetric, "1.0.0".to_string());

        assert!(full.capability_score() > minimal.capability_score());
        assert!(full.capability_score() >= 80);
        assert!(minimal.capability_score() <= 20);
    }

    #[test]
    fn test_serialization() {
        let caps = PeerCapabilities::full_featured(NatType::Open, "1.0.0".to_string());

        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: PeerCapabilities = serde_json::from_str(&json).unwrap();

        assert_eq!(caps, deserialized);
    }
}

// Made with Bob
