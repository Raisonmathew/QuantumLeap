//! NAT type value object and compatibility logic

use serde::{Deserialize, Serialize};

/// Network Address Translation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatType {
    /// No NAT, direct connectivity
    Open,
    /// Easy P2P - Same public port for all destinations
    FullCone,
    /// Moderate - Restricts by IP
    RestrictedCone,
    /// Hard - Restricts by IP:Port
    PortRestricted,
    /// Hardest - Different port per destination
    Symmetric,
    /// Unknown NAT type
    Unknown,
}

impl Default for NatType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl NatType {
    /// Check if this NAT type allows direct P2P connections
    pub fn allows_direct_p2p(&self) -> bool {
        matches!(self, Self::Open | Self::FullCone)
    }

    /// Check if this NAT type requires STUN assistance
    pub fn requires_stun(&self) -> bool {
        !matches!(self, Self::Open)
    }

    /// Check if this NAT type requires TURN relay
    pub fn requires_relay(&self) -> bool {
        matches!(self, Self::Symmetric)
    }

    /// Get difficulty level for P2P (0 = easy, 4 = impossible)
    pub fn p2p_difficulty(&self) -> u8 {
        match self {
            Self::Open => 0,
            Self::FullCone => 1,
            Self::RestrictedCone => 2,
            Self::PortRestricted => 3,
            Self::Symmetric => 4,
            Self::Unknown => 2, // Assume moderate difficulty
        }
    }
}

/// NAT compatibility checker (Domain Service)
pub struct NatCompatibility;

impl NatCompatibility {
    /// Check if direct P2P is likely to work between two NAT types
    pub fn can_direct_p2p(local: NatType, remote: NatType) -> bool {
        matches!(
            (local, remote),
            (NatType::Open, _)
                | (_, NatType::Open)
                | (NatType::FullCone, NatType::FullCone)
        )
    }

    /// Check if STUN-assisted connection is likely to work
    pub fn can_stun_assisted(local: NatType, remote: NatType) -> bool {
        !matches!(
            (local, remote),
            (NatType::Symmetric, _) | (_, NatType::Symmetric)
        )
    }

    /// Check if TURN relay is needed
    pub fn needs_relay(local: NatType, remote: NatType) -> bool {
        matches!(
            (local, remote),
            (NatType::Symmetric, _) | (_, NatType::Symmetric)
        )
    }

    /// Calculate compatibility score (0-100, higher is better)
    pub fn compatibility_score(local: NatType, remote: NatType) -> u8 {
        match (local, remote) {
            (NatType::Open, _) | (_, NatType::Open) => 100,
            (NatType::FullCone, NatType::FullCone) => 95,
            (NatType::FullCone, NatType::RestrictedCone)
            | (NatType::RestrictedCone, NatType::FullCone) => 80,
            (NatType::RestrictedCone, NatType::RestrictedCone) => 70,
            (NatType::FullCone, NatType::PortRestricted)
            | (NatType::PortRestricted, NatType::FullCone) => 60,
            (NatType::RestrictedCone, NatType::PortRestricted)
            | (NatType::PortRestricted, NatType::RestrictedCone) => 50,
            (NatType::PortRestricted, NatType::PortRestricted) => 40,
            (NatType::Symmetric, _) | (_, NatType::Symmetric) => 10,
            (NatType::Unknown, _) | (_, NatType::Unknown) => 50,
        }
    }

    /// Get expected success rate for direct P2P (0.0 to 1.0)
    pub fn p2p_success_rate(local: NatType, remote: NatType) -> f64 {
        Self::compatibility_score(local, remote) as f64 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_type_allows_direct_p2p() {
        assert!(NatType::Open.allows_direct_p2p());
        assert!(NatType::FullCone.allows_direct_p2p());
        assert!(!NatType::Symmetric.allows_direct_p2p());
    }

    #[test]
    fn test_nat_type_requires_stun() {
        assert!(!NatType::Open.requires_stun());
        assert!(NatType::FullCone.requires_stun());
        assert!(NatType::Symmetric.requires_stun());
    }

    #[test]
    fn test_nat_type_requires_relay() {
        assert!(!NatType::Open.requires_relay());
        assert!(!NatType::FullCone.requires_relay());
        assert!(NatType::Symmetric.requires_relay());
    }

    #[test]
    fn test_p2p_difficulty() {
        assert_eq!(NatType::Open.p2p_difficulty(), 0);
        assert_eq!(NatType::FullCone.p2p_difficulty(), 1);
        assert_eq!(NatType::Symmetric.p2p_difficulty(), 4);
    }

    #[test]
    fn test_can_direct_p2p() {
        assert!(NatCompatibility::can_direct_p2p(
            NatType::Open,
            NatType::Symmetric
        ));
        assert!(NatCompatibility::can_direct_p2p(
            NatType::FullCone,
            NatType::FullCone
        ));
        assert!(!NatCompatibility::can_direct_p2p(
            NatType::Symmetric,
            NatType::Symmetric
        ));
    }

    #[test]
    fn test_can_stun_assisted() {
        assert!(NatCompatibility::can_stun_assisted(
            NatType::FullCone,
            NatType::RestrictedCone
        ));
        assert!(!NatCompatibility::can_stun_assisted(
            NatType::Symmetric,
            NatType::FullCone
        ));
    }

    #[test]
    fn test_needs_relay() {
        assert!(NatCompatibility::needs_relay(
            NatType::Symmetric,
            NatType::FullCone
        ));
        assert!(!NatCompatibility::needs_relay(
            NatType::Open,
            NatType::FullCone
        ));
    }

    #[test]
    fn test_compatibility_score() {
        assert_eq!(
            NatCompatibility::compatibility_score(NatType::Open, NatType::Symmetric),
            100
        );
        assert_eq!(
            NatCompatibility::compatibility_score(NatType::FullCone, NatType::FullCone),
            95
        );
        assert_eq!(
            NatCompatibility::compatibility_score(NatType::Symmetric, NatType::Symmetric),
            10
        );
    }

    #[test]
    fn test_p2p_success_rate() {
        assert_eq!(
            NatCompatibility::p2p_success_rate(NatType::Open, NatType::FullCone),
            1.0
        );
        assert_eq!(
            NatCompatibility::p2p_success_rate(NatType::Symmetric, NatType::Symmetric),
            0.1
        );
    }
}

// Made with Bob
