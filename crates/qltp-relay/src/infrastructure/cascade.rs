//! Connection Cascade Strategy
//!
//! Implements intelligent connection establishment with fallback:
//! 1. Direct P2P (if both peers have public IPs)
//! 2. STUN-Assisted P2P (for symmetric NAT traversal)
//! 3. TURN Relay (when P2P fails)

use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::domain::{ConnectionMethod, NatType};

/// Connection cascade result
#[derive(Debug, Clone, PartialEq)]
pub enum CascadeResult {
    /// Direct P2P connection established
    DirectP2P {
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    },
    /// STUN-assisted P2P connection established
    StunAssisted {
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        stun_server: SocketAddr,
    },
    /// TURN relay connection established
    TurnRelay {
        relay_addr: SocketAddr,
        turn_server: SocketAddr,
    },
    /// All connection attempts failed
    Failed {
        reason: String,
    },
}

impl CascadeResult {
    /// Get the connection method used
    pub fn method(&self) -> ConnectionMethod {
        match self {
            CascadeResult::DirectP2P { .. } => ConnectionMethod::DirectP2P,
            CascadeResult::StunAssisted { .. } => ConnectionMethod::StunAssisted,
            CascadeResult::TurnRelay { .. } => ConnectionMethod::TurnRelay,
            CascadeResult::Failed { .. } => ConnectionMethod::DirectP2P, // Default
        }
    }

    /// Check if connection was successful
    pub fn is_success(&self) -> bool {
        !matches!(self, CascadeResult::Failed { .. })
    }
}

/// Connection cascade configuration
#[derive(Debug, Clone)]
pub struct CascadeConfig {
    /// Timeout for direct P2P attempt
    pub direct_timeout: Duration,
    /// Timeout for STUN-assisted attempt
    pub stun_timeout: Duration,
    /// Timeout for TURN relay attempt
    pub turn_timeout: Duration,
    /// Enable direct P2P
    pub enable_direct: bool,
    /// Enable STUN-assisted P2P
    pub enable_stun: bool,
    /// Enable TURN relay
    pub enable_turn: bool,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            direct_timeout: Duration::from_secs(5),
            stun_timeout: Duration::from_secs(10),
            turn_timeout: Duration::from_secs(15),
            enable_direct: true,
            enable_stun: true,
            enable_turn: true,
        }
    }
}

/// Connection cascade strategy
pub struct ConnectionCascade {
    config: CascadeConfig,
    stun_server: SocketAddr,
    turn_server: SocketAddr,
}

impl ConnectionCascade {
    /// Create new connection cascade
    pub fn new(
        config: CascadeConfig,
        stun_server: SocketAddr,
        turn_server: SocketAddr,
    ) -> Self {
        Self {
            config,
            stun_server,
            turn_server,
        }
    }

    /// Execute connection cascade
    pub async fn execute(
        &self,
        local_nat: NatType,
        remote_nat: NatType,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> CascadeResult {
        info!(
            "Starting connection cascade: local_nat={:?}, remote_nat={:?}",
            local_nat, remote_nat
        );

        // Step 1: Try direct P2P if both peers are compatible
        if self.config.enable_direct && self.can_direct_connect(local_nat, remote_nat) {
            debug!("Attempting direct P2P connection...");
            if let Ok(result) = timeout(
                self.config.direct_timeout,
                self.try_direct_p2p(local_addr, remote_addr),
            )
            .await
            {
                if let Some(res) = result {
                    info!("✓ Direct P2P connection established");
                    return res;
                }
            }
            warn!("✗ Direct P2P connection failed");
        }

        // Step 2: Try STUN-assisted P2P
        if self.config.enable_stun && self.can_stun_assist(local_nat, remote_nat) {
            debug!("Attempting STUN-assisted P2P connection...");
            if let Ok(result) = timeout(
                self.config.stun_timeout,
                self.try_stun_assisted(local_addr, remote_addr),
            )
            .await
            {
                if let Some(res) = result {
                    info!("✓ STUN-assisted P2P connection established");
                    return res;
                }
            }
            warn!("✗ STUN-assisted P2P connection failed");
        }

        // Step 3: Fall back to TURN relay
        if self.config.enable_turn {
            debug!("Attempting TURN relay connection...");
            if let Ok(result) = timeout(
                self.config.turn_timeout,
                self.try_turn_relay(local_addr),
            )
            .await
            {
                if let Some(res) = result {
                    info!("✓ TURN relay connection established");
                    return res;
                }
            }
            warn!("✗ TURN relay connection failed");
        }

        // All attempts failed
        warn!("✗ All connection attempts failed");
        CascadeResult::Failed {
            reason: "All connection methods exhausted".to_string(),
        }
    }

    /// Check if direct P2P is possible
    fn can_direct_connect(&self, local_nat: NatType, remote_nat: NatType) -> bool {
        use NatType::*;
        
        match (local_nat, remote_nat) {
            // Both have public IPs (Open = no NAT)
            (Open, Open) => true,
            // One public, one behind NAT
            (Open, _) | (_, Open) => true,
            // Both behind full cone NAT
            (FullCone, FullCone) => true,
            // One full cone, one restricted
            (FullCone, RestrictedCone) | (RestrictedCone, FullCone) => true,
            // Both restricted cone
            (RestrictedCone, RestrictedCone) => true,
            // Port restricted scenarios
            (FullCone, PortRestricted) | (PortRestricted, FullCone) => true,
            (RestrictedCone, PortRestricted) | (PortRestricted, RestrictedCone) => true,
            // Symmetric NAT requires STUN or TURN
            (Symmetric, _) | (_, Symmetric) => false,
            // Unknown NAT types - try anyway
            (Unknown, _) | (_, Unknown) => true,
            _ => false,
        }
    }

    /// Check if STUN-assisted P2P is possible
    fn can_stun_assist(&self, local_nat: NatType, remote_nat: NatType) -> bool {
        use NatType::*;
        
        match (local_nat, remote_nat) {
            // STUN helps with most NAT combinations except double symmetric
            (Symmetric, Symmetric) => false,
            // STUN can help with one symmetric NAT
            (Symmetric, _) | (_, Symmetric) => true,
            // STUN helps with port restricted scenarios
            (PortRestricted, _) | (_, PortRestricted) => true,
            // Unknown NAT - try STUN
            (Unknown, _) | (_, Unknown) => true,
            // Already handled by direct P2P
            _ => false,
        }
    }

    /// Try direct P2P connection
    async fn try_direct_p2p(
        &self,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> Option<CascadeResult> {
        // Simulate connection attempt
        // In production, this would:
        // 1. Create UDP socket
        // 2. Send connection request to remote_addr
        // 3. Wait for response
        // 4. Establish bidirectional communication
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // For now, return success if addresses are valid
        if local_addr.port() > 0 && remote_addr.port() > 0 {
            Some(CascadeResult::DirectP2P {
                local_addr,
                remote_addr,
            })
        } else {
            None
        }
    }

    /// Try STUN-assisted P2P connection
    async fn try_stun_assisted(
        &self,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> Option<CascadeResult> {
        // Simulate STUN-assisted connection
        // In production, this would:
        // 1. Send STUN Binding Request to discover public address
        // 2. Exchange discovered addresses with peer via signaling
        // 3. Perform hole punching
        // 4. Establish P2P connection
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        Some(CascadeResult::StunAssisted {
            local_addr,
            remote_addr,
            stun_server: self.stun_server,
        })
    }

    /// Try TURN relay connection
    async fn try_turn_relay(&self, local_addr: SocketAddr) -> Option<CascadeResult> {
        // Simulate TURN relay allocation
        // In production, this would:
        // 1. Send TURN Allocate Request
        // 2. Receive relay address
        // 3. Create permissions for peer
        // 4. Relay data through TURN server
        
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Generate relay address (in production, this comes from TURN server)
        let relay_addr = SocketAddr::new(
            self.turn_server.ip(),
            49152 + (local_addr.port() % 1000),
        );
        
        Some(CascadeResult::TurnRelay {
            relay_addr,
            turn_server: self.turn_server,
        })
    }

    /// Get recommended connection method based on NAT types
    pub fn recommend_method(&self, local_nat: NatType, remote_nat: NatType) -> ConnectionMethod {
        if self.can_direct_connect(local_nat, remote_nat) {
            ConnectionMethod::DirectP2P
        } else if self.can_stun_assist(local_nat, remote_nat) {
            ConnectionMethod::StunAssisted
        } else {
            ConnectionMethod::TurnRelay
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_cascade() -> ConnectionCascade {
        ConnectionCascade::new(
            CascadeConfig::default(),
            "127.0.0.1:3478".parse().unwrap(),
            "127.0.0.1:3479".parse().unwrap(),
        )
    }

    #[test]
    fn test_can_direct_connect() {
        let cascade = create_cascade();

        // Both open (no NAT) - should work
        assert!(cascade.can_direct_connect(NatType::Open, NatType::Open));

        // One open - should work
        assert!(cascade.can_direct_connect(NatType::Open, NatType::FullCone));
        assert!(cascade.can_direct_connect(NatType::RestrictedCone, NatType::Open));

        // Both full cone - should work
        assert!(cascade.can_direct_connect(NatType::FullCone, NatType::FullCone));

        // Symmetric NAT - should not work
        assert!(!cascade.can_direct_connect(NatType::Symmetric, NatType::Symmetric));
        assert!(!cascade.can_direct_connect(NatType::Symmetric, NatType::FullCone));
    }

    #[test]
    fn test_can_stun_assist() {
        let cascade = create_cascade();

        // One symmetric - STUN can help
        assert!(cascade.can_stun_assist(NatType::Symmetric, NatType::FullCone));
        assert!(cascade.can_stun_assist(NatType::RestrictedCone, NatType::Symmetric));

        // Both symmetric - STUN cannot help
        assert!(!cascade.can_stun_assist(NatType::Symmetric, NatType::Symmetric));

        // Port restricted - STUN helps
        assert!(cascade.can_stun_assist(NatType::PortRestricted, NatType::FullCone));
    }

    #[test]
    fn test_recommend_method() {
        let cascade = create_cascade();

        // Open to open - direct
        assert_eq!(
            cascade.recommend_method(NatType::Open, NatType::Open),
            ConnectionMethod::DirectP2P
        );

        // One symmetric - STUN
        assert_eq!(
            cascade.recommend_method(NatType::Symmetric, NatType::FullCone),
            ConnectionMethod::StunAssisted
        );

        // Both symmetric - TURN
        assert_eq!(
            cascade.recommend_method(NatType::Symmetric, NatType::Symmetric),
            ConnectionMethod::TurnRelay
        );
    }

    #[tokio::test]
    async fn test_execute_direct_p2p() {
        let cascade = create_cascade();
        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = cascade
            .execute(NatType::Open, NatType::Open, local_addr, remote_addr)
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), ConnectionMethod::DirectP2P);
    }

    #[tokio::test]
    async fn test_execute_stun_assisted() {
        let cascade = create_cascade();
        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = cascade
            .execute(NatType::Symmetric, NatType::FullCone, local_addr, remote_addr)
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), ConnectionMethod::StunAssisted);
    }

    #[tokio::test]
    async fn test_execute_turn_relay() {
        let cascade = create_cascade();
        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = cascade
            .execute(
                NatType::Symmetric,
                NatType::Symmetric,
                local_addr,
                remote_addr,
            )
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), ConnectionMethod::TurnRelay);
    }

    #[tokio::test]
    async fn test_cascade_result_methods() {
        let direct = CascadeResult::DirectP2P {
            local_addr: "127.0.0.1:5000".parse().unwrap(),
            remote_addr: "127.0.0.1:5001".parse().unwrap(),
        };
        assert!(direct.is_success());
        assert_eq!(direct.method(), ConnectionMethod::DirectP2P);

        let failed = CascadeResult::Failed {
            reason: "Test failure".to_string(),
        };
        assert!(!failed.is_success());
    }
}

// Made with Bob
