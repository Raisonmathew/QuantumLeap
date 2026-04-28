//! Configuration for Relay Service

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

/// Relay service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// WebSocket server configuration
    pub websocket: WebSocketConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Timeouts configuration
    pub timeouts: TimeoutsConfig,
    /// Limits configuration
    pub limits: LimitsConfig,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            websocket: WebSocketConfig::default(),
            storage: StorageConfig::default(),
            timeouts: TimeoutsConfig::default(),
            limits: LimitsConfig::default(),
        }
    }
}

/// WebSocket server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Bind address
    pub bind_addr: SocketAddr,
    /// Maximum message size (bytes)
    pub max_message_size: usize,
    /// Ping interval
    pub ping_interval: Duration,
    /// Pong timeout
    pub pong_timeout: Duration,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            // Use infallible constructor so a future refactor that
            // mistypes the literal can't crash a binary at startup.
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080),
            max_message_size: 1024 * 1024, // 1MB
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage type
    pub storage_type: StorageType,
    /// Cleanup interval
    pub cleanup_interval: Duration,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            storage_type: StorageType::InMemory,
            cleanup_interval: Duration::from_secs(60),
        }
    }
}

/// Storage type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageType {
    /// In-memory storage (for development/testing)
    InMemory,
    /// Redis storage (for production)
    Redis { url: String },
    /// PostgreSQL storage (for production)
    PostgreSQL { url: String },
}

/// Timeouts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutsConfig {
    /// Peer registration timeout
    pub peer_registration: Duration,
    /// Peer connection timeout
    pub peer_connection: Duration,
    /// Session establishment timeout
    pub session_establishment: Duration,
    /// Connection attempt timeout
    pub connection_attempt: Duration,
    /// ICE gathering timeout
    pub ice_gathering: Duration,
}

impl Default for TimeoutsConfig {
    fn default() -> Self {
        Self {
            peer_registration: Duration::from_secs(30),
            peer_connection: Duration::from_secs(60),
            session_establishment: Duration::from_secs(120),
            connection_attempt: Duration::from_secs(30),
            ice_gathering: Duration::from_secs(10),
        }
    }
}

/// Limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    /// Maximum peers
    pub max_peers: usize,
    /// Maximum sessions per peer
    pub max_sessions_per_peer: usize,
    /// Maximum ICE candidates per peer
    pub max_ice_candidates_per_peer: usize,
    /// Maximum concurrent connections
    pub max_concurrent_connections: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_peers: 10_000,
            max_sessions_per_peer: 10,
            max_ice_candidates_per_peer: 20,
            max_concurrent_connections: 5_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RelayConfig::default();
        assert_eq!(config.websocket.max_message_size, 1024 * 1024);
        assert_eq!(config.limits.max_peers, 10_000);
    }

    #[test]
    fn test_config_serialization() {
        let config = RelayConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RelayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.limits.max_peers, deserialized.limits.max_peers);
    }
}

// Made with Bob
