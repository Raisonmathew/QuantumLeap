//! Unified Relay Service
//!
//! Integrates STUN, TURN, and signaling servers into a single service with
//! authentication, rate limiting, metrics, and intelligent connection cascade

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::stun::{StunServer, StunServerConfig};
use crate::turn::{TurnServer, TurnServerConfig};
use crate::infrastructure::{
    WebSocketServer, WebSocketConfig, AuthManager, RateLimiter, RateLimitConfig,
    ServerMetrics, ConnectionCascade, CascadeConfig,
};
use crate::application::{PeerService, SessionService, ConnectionService};
use crate::domain::NatType;

/// Unified relay service configuration
#[derive(Debug, Clone)]
pub struct RelayServiceConfig {
    /// WebSocket signaling server address
    pub signaling_addr: SocketAddr,
    /// STUN server address
    pub stun_addr: SocketAddr,
    /// TURN server address
    pub turn_addr: SocketAddr,
    /// TURN relay base address
    pub turn_relay_base: SocketAddr,
    /// Maximum TURN allocations
    pub max_turn_allocations: usize,
    /// Server software name
    pub software: String,
    /// Authentication realm
    pub auth_realm: String,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Connection cascade configuration
    pub cascade: CascadeConfig,
}

impl Default for RelayServiceConfig {
    fn default() -> Self {
        Self {
            signaling_addr: "0.0.0.0:8080".parse().unwrap(),
            stun_addr: "0.0.0.0:3478".parse().unwrap(),
            turn_addr: "0.0.0.0:3479".parse().unwrap(),
            turn_relay_base: "0.0.0.0:0".parse().unwrap(),
            max_turn_allocations: 1000,
            software: "QLTP-Relay/1.0".to_string(),
            auth_realm: "qltp.relay".to_string(),
            rate_limit: RateLimitConfig::default(),
            cascade: CascadeConfig::default(),
        }
    }
}

/// Unified relay service
pub struct RelayService {
    config: RelayServiceConfig,
    peer_service: Arc<PeerService>,
    session_service: Arc<SessionService>,
    connection_service: Arc<ConnectionService>,
    auth_manager: Arc<AuthManager>,
    rate_limiter: Arc<RateLimiter>,
    metrics: Arc<ServerMetrics>,
    cascade: Arc<ConnectionCascade>,
}

impl RelayService {
    /// Create new relay service
    pub fn new(config: RelayServiceConfig) -> Self {
        // Create services with default timeouts
        let peer_service = Arc::new(PeerService::new(Duration::from_secs(60)));
        let session_service = Arc::new(SessionService::new(Duration::from_secs(120)));
        let connection_service = Arc::new(ConnectionService::new());

        // Create infrastructure components
        let auth_manager = Arc::new(AuthManager::new(config.auth_realm.clone()));
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit.clone()));
        let metrics = Arc::new(ServerMetrics::new());
        let cascade = Arc::new(ConnectionCascade::new(
            config.cascade.clone(),
            config.stun_addr,
            config.turn_addr,
        ));

        Self {
            config,
            peer_service,
            session_service,
            connection_service,
            auth_manager,
            rate_limiter,
            metrics,
            cascade,
        }
    }

    /// Start all relay services
    pub async fn start(self: Arc<Self>) -> Result<RelayServiceHandles, std::io::Error> {
        println!("Starting QLTP Relay Service...");
        println!("  Signaling: {}", self.config.signaling_addr);
        println!("  STUN:      {}", self.config.stun_addr);
        println!("  TURN:      {}", self.config.turn_addr);

        // Start STUN server
        let stun_config = StunServerConfig {
            bind_addr: self.config.stun_addr,
            max_connections: 10000,
            include_software: true,
            software_name: self.config.software.clone(),
        };
        let stun_server = Arc::new(StunServer::new(stun_config).await?);
        let stun_handle = tokio::spawn({
            let server = stun_server.clone();
            async move {
                if let Err(e) = server.run().await {
                    eprintln!("STUN server error: {}", e);
                }
            }
        });

        // Start TURN server
        let turn_config = TurnServerConfig {
            bind_addr: self.config.turn_addr,
            relay_base_addr: self.config.turn_relay_base,
            max_allocations: self.config.max_turn_allocations,
            default_lifetime: 600,
            cleanup_interval: std::time::Duration::from_secs(60),
            software: self.config.software.clone(),
        };
        let turn_server = Arc::new(TurnServer::new(turn_config).await?);
        let turn_handle = tokio::spawn({
            let server = turn_server.clone();
            async move {
                if let Err(e) = server.run().await {
                    eprintln!("TURN server error: {}", e);
                }
            }
        });

        // Start WebSocket signaling server
        let ws_config = WebSocketConfig {
            bind_addr: self.config.signaling_addr,
            max_message_size: 1024 * 1024,
            ping_interval: Duration::from_secs(30),
            pong_timeout: Duration::from_secs(10),
        };
        let ws_server = Arc::new(WebSocketServer::new(ws_config));
        let ws_handle = tokio::spawn({
            let server = ws_server.clone();
            async move {
                if let Err(e) = server.start().await {
                    eprintln!("WebSocket server error: {}", e);
                }
            }
        });

        println!("✓ All relay services started successfully");

        Ok(RelayServiceHandles {
            stun_handle,
            turn_handle,
            ws_handle,
        })
    }

    /// Get STUN server endpoint
    pub fn stun_endpoint(&self) -> SocketAddr {
        self.config.stun_addr
    }

    /// Get TURN server endpoint
    pub fn turn_endpoint(&self) -> SocketAddr {
        self.config.turn_addr
    }

    /// Get signaling server endpoint
    pub fn signaling_endpoint(&self) -> SocketAddr {
        self.config.signaling_addr
    }

    /// Get peer service
    pub fn peer_service(&self) -> Arc<PeerService> {
        self.peer_service.clone()
    }

    /// Get session service
    pub fn session_service(&self) -> Arc<SessionService> {
        self.session_service.clone()
    }

    /// Get connection service
    pub fn connection_service(&self) -> Arc<ConnectionService> {
        self.connection_service.clone()
    }

    /// Get authentication manager
    pub fn auth_manager(&self) -> Arc<AuthManager> {
        self.auth_manager.clone()
    }

    /// Get rate limiter
    pub fn rate_limiter(&self) -> Arc<RateLimiter> {
        self.rate_limiter.clone()
    }

    /// Get metrics
    pub fn metrics(&self) -> Arc<ServerMetrics> {
        self.metrics.clone()
    }

    /// Get connection cascade
    pub fn cascade(&self) -> Arc<ConnectionCascade> {
        self.cascade.clone()
    }

    /// Establish connection using cascade strategy
    pub async fn establish_connection(
        &self,
        local_nat: NatType,
        remote_nat: NatType,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
    ) -> crate::infrastructure::cascade::CascadeResult {
        // Execute cascade
        let result = self
            .cascade
            .execute(local_nat, remote_nat, local_addr, remote_addr)
            .await;

        // Record result
        if result.is_success() {
            self.metrics.connection_opened();
        }

        result
    }

    /// Add authenticated user
    pub async fn add_user(&self, username: String, password: String) {
        self.auth_manager.add_user(username, password).await;
    }

    /// Check if request is allowed (returns true if allowed, false if rate limited)
    pub async fn check_rate_limit(&self, client_ip: SocketAddr) -> bool {
        self.rate_limiter.check_rate_limit(client_ip.ip()).await
    }
}

/// Handles for all relay service tasks
pub struct RelayServiceHandles {
    pub stun_handle: JoinHandle<()>,
    pub turn_handle: JoinHandle<()>,
    pub ws_handle: JoinHandle<()>,
}

impl RelayServiceHandles {
    /// Wait for all services to complete
    pub async fn join_all(self) -> Result<(), tokio::task::JoinError> {
        tokio::try_join!(
            self.stun_handle,
            self.turn_handle,
            self.ws_handle,
        )?;
        Ok(())
    }

    /// Abort all services
    pub fn abort_all(self) {
        self.stun_handle.abort();
        self.turn_handle.abort();
        self.ws_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_relay_service_creation() {
        let config = RelayServiceConfig::default();
        let service = Arc::new(RelayService::new(config.clone()));

        assert_eq!(service.stun_endpoint(), config.stun_addr);
        assert_eq!(service.turn_endpoint(), config.turn_addr);
        assert_eq!(service.signaling_endpoint(), config.signaling_addr);
    }

    #[tokio::test]
    async fn test_relay_service_endpoints() {
        let config = RelayServiceConfig {
            signaling_addr: "127.0.0.1:8080".parse().unwrap(),
            stun_addr: "127.0.0.1:3478".parse().unwrap(),
            turn_addr: "127.0.0.1:3479".parse().unwrap(),
            ..Default::default()
        };

        let service = Arc::new(RelayService::new(config));

        assert_eq!(service.stun_endpoint().port(), 3478);
        assert_eq!(service.turn_endpoint().port(), 3479);
        assert_eq!(service.signaling_endpoint().port(), 8080);
    }

    #[tokio::test]
    async fn test_relay_service_components() {
        let config = RelayServiceConfig::default();
        let service = Arc::new(RelayService::new(config));

        // Test that all components are accessible
        assert!(service.auth_manager().user_count().await == 0);
        assert!(service.metrics().total_requests() == 0);
        
        // Add a user
        service.add_user("test_user".to_string(), "test_pass".to_string()).await;
        assert!(service.auth_manager().user_count().await == 1);
    }

    #[tokio::test]
    async fn test_establish_connection_direct() {
        let config = RelayServiceConfig::default();
        let service = Arc::new(RelayService::new(config));

        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = service
            .establish_connection(
                NatType::Open,
                NatType::Open,
                local_addr,
                remote_addr,
            )
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), crate::domain::ConnectionMethod::DirectP2P);
    }

    #[tokio::test]
    async fn test_establish_connection_stun() {
        let config = RelayServiceConfig::default();
        let service = Arc::new(RelayService::new(config));

        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = service
            .establish_connection(
                NatType::Symmetric,
                NatType::FullCone,
                local_addr,
                remote_addr,
            )
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), crate::domain::ConnectionMethod::StunAssisted);
    }

    #[tokio::test]
    async fn test_establish_connection_turn() {
        let config = RelayServiceConfig::default();
        let service = Arc::new(RelayService::new(config));

        let local_addr = "192.168.1.100:5000".parse().unwrap();
        let remote_addr = "192.168.1.101:5001".parse().unwrap();

        let result = service
            .establish_connection(
                NatType::Symmetric,
                NatType::Symmetric,
                local_addr,
                remote_addr,
            )
            .await;

        assert!(result.is_success());
        assert_eq!(result.method(), crate::domain::ConnectionMethod::TurnRelay);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let mut config = RelayServiceConfig::default();
        // Set high rate limit for testing
        config.rate_limit.max_requests = 100;
        let service = Arc::new(RelayService::new(config));

        let client_ip = "127.0.0.1:12345".parse().unwrap();

        // check_rate_limit returns true when allowed, false when rate limited
        // First request should be allowed
        let is_allowed = service.check_rate_limit(client_ip).await;
        assert!(is_allowed, "First request should be allowed");
        
        // Second request should also be allowed
        let is_allowed = service.check_rate_limit(client_ip).await;
        assert!(is_allowed, "Second request should be allowed");
    }
}

// Made with Bob