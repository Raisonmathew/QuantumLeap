//! Connection Entity
//!
//! Represents an active connection attempt or established connection.
//! This is an entity in DDD terms - it has identity and manages the
//! connection lifecycle including strategy selection and fallback.

use super::ice_candidate::IceCandidate;
use super::nat_type::NatType;
use super::peer_id::PeerId;
use super::session_id::SessionId;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime};

/// Connection strategy type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStrategy {
    /// Try direct P2P first, then STUN, then TURN
    DirectP2PFirst,
    /// Balanced approach - try all methods in parallel
    Balanced,
    /// Prefer relay for reliability
    RelayFirst,
}

impl ConnectionStrategy {
    /// Select strategy based on NAT types
    pub fn select(local_nat: NatType, remote_nat: NatType) -> Self {
        use super::nat_type::NatCompatibility;

        let score = NatCompatibility::compatibility_score(local_nat, remote_nat);

        if score >= 80 {
            ConnectionStrategy::DirectP2PFirst
        } else if score >= 40 {
            ConnectionStrategy::Balanced
        } else {
            ConnectionStrategy::RelayFirst
        }
    }

    /// Get attempt order for this strategy
    pub fn attempt_order(&self) -> Vec<ConnectionMethod> {
        match self {
            ConnectionStrategy::DirectP2PFirst => vec![
                ConnectionMethod::DirectP2P,
                ConnectionMethod::StunAssisted,
                ConnectionMethod::TurnRelay,
            ],
            ConnectionStrategy::Balanced => vec![
                ConnectionMethod::DirectP2P,
                ConnectionMethod::StunAssisted,
                ConnectionMethod::TurnRelay,
            ],
            ConnectionStrategy::RelayFirst => vec![
                ConnectionMethod::TurnRelay,
                ConnectionMethod::StunAssisted,
                ConnectionMethod::DirectP2P,
            ],
        }
    }

    /// Check if strategy uses parallel attempts
    pub fn is_parallel(&self) -> bool {
        matches!(self, ConnectionStrategy::Balanced)
    }
}

/// Connection method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionMethod {
    /// Direct peer-to-peer
    DirectP2P,
    /// STUN-assisted
    StunAssisted,
    /// TURN relay
    TurnRelay,
}

impl ConnectionMethod {
    /// Get timeout for this method
    pub fn timeout(&self) -> Duration {
        match self {
            ConnectionMethod::DirectP2P => Duration::from_secs(5),
            ConnectionMethod::StunAssisted => Duration::from_secs(8),
            ConnectionMethod::TurnRelay => Duration::from_secs(10),
        }
    }

    /// Get priority (higher = preferred)
    pub fn priority(&self) -> u8 {
        match self {
            ConnectionMethod::DirectP2P => 100,
            ConnectionMethod::StunAssisted => 80,
            ConnectionMethod::TurnRelay => 50,
        }
    }

    /// Check if method requires relay server
    pub fn requires_relay(&self) -> bool {
        matches!(self, ConnectionMethod::TurnRelay)
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Initializing connection
    Initializing,
    /// Attempting connection
    Attempting,
    /// Connection established
    Established,
    /// Connection failed
    Failed,
    /// Connection closed
    Closed,
}

impl ConnectionState {
    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            ConnectionState::Initializing
                | ConnectionState::Attempting
                | ConnectionState::Established
        )
    }

    /// Check if connection is established
    pub fn is_established(&self) -> bool {
        matches!(self, ConnectionState::Established)
    }

    /// Check if connection is terminal
    pub fn is_terminal(&self) -> bool {
        matches!(self, ConnectionState::Failed | ConnectionState::Closed)
    }
}

/// Connection attempt result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAttempt {
    /// Method used
    method: ConnectionMethod,
    /// Start time
    started_at: SystemTime,
    /// End time (if completed)
    ended_at: Option<SystemTime>,
    /// Whether attempt succeeded
    succeeded: bool,
    /// Error message (if failed)
    error: Option<String>,
    /// Selected candidates (if succeeded)
    selected_candidates: Option<(IceCandidate, IceCandidate)>,
}

impl ConnectionAttempt {
    /// Create new attempt
    pub fn new(method: ConnectionMethod) -> Self {
        Self {
            method,
            started_at: SystemTime::now(),
            ended_at: None,
            succeeded: false,
            error: None,
            selected_candidates: None,
        }
    }

    /// Mark attempt as succeeded
    pub fn succeed(
        &mut self,
        local_candidate: IceCandidate,
        remote_candidate: IceCandidate,
    ) {
        self.ended_at = Some(SystemTime::now());
        self.succeeded = true;
        self.selected_candidates = Some((local_candidate, remote_candidate));
    }

    /// Mark attempt as failed
    pub fn fail(&mut self, error: String) {
        self.ended_at = Some(SystemTime::now());
        self.succeeded = false;
        self.error = Some(error);
    }

    /// Get attempt duration
    pub fn duration(&self) -> Option<Duration> {
        self.ended_at
            .and_then(|end| end.duration_since(self.started_at).ok())
    }

    /// Check if attempt is complete
    pub fn is_complete(&self) -> bool {
        self.ended_at.is_some()
    }

    /// Get method
    pub fn method(&self) -> ConnectionMethod {
        self.method
    }

    /// Check if succeeded
    pub fn succeeded(&self) -> bool {
        self.succeeded
    }

    /// Get error
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Get selected candidates
    pub fn selected_candidates(&self) -> Option<&(IceCandidate, IceCandidate)> {
        self.selected_candidates.as_ref()
    }
}

/// Connection entity - manages connection establishment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Session ID this connection belongs to
    session_id: SessionId,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Remote peer ID
    remote_peer_id: PeerId,
    /// Local NAT type
    local_nat: NatType,
    /// Remote NAT type
    remote_nat: NatType,
    /// Connection strategy
    strategy: ConnectionStrategy,
    /// Current state
    state: ConnectionState,
    /// Connection attempts
    attempts: Vec<ConnectionAttempt>,
    /// Currently active method (if attempting)
    active_method: Option<ConnectionMethod>,
    /// Successful method (if established)
    successful_method: Option<ConnectionMethod>,
    /// Selected local candidate (if established)
    local_candidate: Option<IceCandidate>,
    /// Selected remote candidate (if established)
    remote_candidate: Option<IceCandidate>,
    /// Relay server address (if using relay)
    relay_address: Option<SocketAddr>,
    /// Creation time
    created_at: SystemTime,
    /// Establishment time (if established)
    established_at: Option<SystemTime>,
    /// Last activity time
    last_activity: SystemTime,
}

impl Connection {
    /// Create new connection
    pub fn new(
        session_id: SessionId,
        local_peer_id: PeerId,
        remote_peer_id: PeerId,
        local_nat: NatType,
        remote_nat: NatType,
    ) -> Self {
        let strategy = ConnectionStrategy::select(local_nat, remote_nat);
        let now = SystemTime::now();

        Self {
            session_id,
            local_peer_id,
            remote_peer_id,
            local_nat,
            remote_nat,
            strategy,
            state: ConnectionState::Initializing,
            attempts: Vec::new(),
            active_method: None,
            successful_method: None,
            local_candidate: None,
            remote_candidate: None,
            relay_address: None,
            created_at: now,
            established_at: None,
            last_activity: now,
        }
    }

    /// Get session ID
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get remote peer ID
    pub fn remote_peer_id(&self) -> &PeerId {
        &self.remote_peer_id
    }

    /// Get local NAT type
    pub fn local_nat(&self) -> NatType {
        self.local_nat
    }

    /// Get remote NAT type
    pub fn remote_nat(&self) -> NatType {
        self.remote_nat
    }

    /// Get strategy
    pub fn strategy(&self) -> ConnectionStrategy {
        self.strategy
    }

    /// Get state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Get attempts
    pub fn attempts(&self) -> &[ConnectionAttempt] {
        &self.attempts
    }

    /// Get active method
    pub fn active_method(&self) -> Option<ConnectionMethod> {
        self.active_method
    }

    /// Get successful method
    pub fn successful_method(&self) -> Option<ConnectionMethod> {
        self.successful_method
    }

    /// Get selected candidates
    pub fn selected_candidates(&self) -> Option<(&IceCandidate, &IceCandidate)> {
        match (&self.local_candidate, &self.remote_candidate) {
            (Some(local), Some(remote)) => Some((local, remote)),
            _ => None,
        }
    }

    /// Get relay address
    pub fn relay_address(&self) -> Option<SocketAddr> {
        self.relay_address
    }

    /// Get creation time
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Get establishment time
    pub fn established_at(&self) -> Option<SystemTime> {
        self.established_at
    }

    /// Get last activity time
    pub fn last_activity(&self) -> SystemTime {
        self.last_activity
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Check if connection is established
    pub fn is_established(&self) -> bool {
        self.state.is_established()
    }

    /// Check if connection involves a specific peer
    pub fn involves_peer(&self, peer_id: &PeerId) -> bool {
        &self.local_peer_id == peer_id || &self.remote_peer_id == peer_id
    }

    /// Get number of attempts
    pub fn attempt_count(&self) -> usize {
        self.attempts.len()
    }

    /// Get successful attempts
    pub fn successful_attempts(&self) -> Vec<&ConnectionAttempt> {
        self.attempts.iter().filter(|a| a.succeeded()).collect()
    }

    /// Get failed attempts
    pub fn failed_attempts(&self) -> Vec<&ConnectionAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.is_complete() && !a.succeeded())
            .collect()
    }

    /// Get establishment duration
    pub fn establishment_duration(&self) -> Option<Duration> {
        self.established_at
            .and_then(|est| est.duration_since(self.created_at).ok())
    }

    /// Start connection attempt with method
    pub fn start_attempt(&mut self, method: ConnectionMethod) {
        if self.state == ConnectionState::Initializing
            || self.state == ConnectionState::Attempting
        {
            self.state = ConnectionState::Attempting;
            self.active_method = Some(method);
            self.attempts.push(ConnectionAttempt::new(method));
            self.update_activity();
        }
    }

    /// Complete current attempt successfully
    pub fn complete_attempt(
        &mut self,
        local_candidate: IceCandidate,
        remote_candidate: IceCandidate,
        relay_address: Option<SocketAddr>,
    ) {
        if let Some(method) = self.active_method {
            if let Some(attempt) = self.attempts.last_mut() {
                attempt.succeed(local_candidate.clone(), remote_candidate.clone());
            }

            self.state = ConnectionState::Established;
            self.successful_method = Some(method);
            self.local_candidate = Some(local_candidate);
            self.remote_candidate = Some(remote_candidate);
            self.relay_address = relay_address;
            self.established_at = Some(SystemTime::now());
            self.active_method = None;
            self.update_activity();
        }
    }

    /// Fail current attempt
    pub fn fail_attempt(&mut self, error: String) {
        if let Some(attempt) = self.attempts.last_mut() {
            attempt.fail(error);
        }
        self.active_method = None;
        self.update_activity();
    }

    /// Mark entire connection as failed
    pub fn fail(&mut self) {
        self.state = ConnectionState::Failed;
        self.active_method = None;
        self.update_activity();
    }

    /// Close connection
    pub fn close(&mut self) {
        self.state = ConnectionState::Closed;
        self.active_method = None;
        self.update_activity();
    }

    /// Update last activity
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Get next method to try based on strategy
    pub fn next_method(&self) -> Option<ConnectionMethod> {
        let order = self.strategy.attempt_order();
        let attempted: Vec<ConnectionMethod> =
            self.attempts.iter().map(|a| a.method()).collect();

        order.into_iter().find(|m| !attempted.contains(m))
    }

    /// Check if all methods have been tried
    pub fn all_methods_tried(&self) -> bool {
        self.next_method().is_none()
    }

    /// Get connection summary
    pub fn summary(&self) -> String {
        format!(
            "Connection {} - {} <-> {} - State: {:?}, Strategy: {:?}, Attempts: {}/{}",
            self.session_id,
            self.local_peer_id,
            self.remote_peer_id,
            self.state,
            self.strategy,
            self.successful_attempts().len(),
            self.attempts.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_connection() -> Connection {
        let session_id = SessionId::new();
        let local_peer = PeerId::new();
        let remote_peer = PeerId::new();
        Connection::new(
            session_id,
            local_peer,
            remote_peer,
            NatType::Open,
            NatType::FullCone,
        )
    }

    #[test]
    fn test_new_connection() {
        let conn = create_test_connection();

        assert_eq!(conn.state(), ConnectionState::Initializing);
        assert_eq!(conn.strategy(), ConnectionStrategy::DirectP2PFirst);
        assert_eq!(conn.attempt_count(), 0);
        assert!(conn.is_active());
        assert!(!conn.is_established());
    }

    #[test]
    fn test_strategy_selection() {
        assert_eq!(
            ConnectionStrategy::select(NatType::Open, NatType::Open),
            ConnectionStrategy::DirectP2PFirst
        );

        assert_eq!(
            ConnectionStrategy::select(NatType::RestrictedCone, NatType::PortRestricted),
            ConnectionStrategy::Balanced
        );

        assert_eq!(
            ConnectionStrategy::select(NatType::Symmetric, NatType::Symmetric),
            ConnectionStrategy::RelayFirst
        );
    }

    #[test]
    fn test_connection_attempt() {
        let mut conn = create_test_connection();

        conn.start_attempt(ConnectionMethod::DirectP2P);
        assert_eq!(conn.state(), ConnectionState::Attempting);
        assert_eq!(conn.active_method(), Some(ConnectionMethod::DirectP2P));
        assert_eq!(conn.attempt_count(), 1);

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        conn.complete_attempt(candidate1, candidate2, None);
        assert_eq!(conn.state(), ConnectionState::Established);
        assert!(conn.is_established());
        assert_eq!(
            conn.successful_method(),
            Some(ConnectionMethod::DirectP2P)
        );
        assert!(conn.established_at().is_some());
    }

    #[test]
    fn test_failed_attempt() {
        let mut conn = create_test_connection();

        conn.start_attempt(ConnectionMethod::DirectP2P);
        conn.fail_attempt("Timeout".to_string());

        assert_eq!(conn.active_method(), None);
        assert_eq!(conn.failed_attempts().len(), 1);
        assert_eq!(conn.state(), ConnectionState::Attempting);
    }

    #[test]
    fn test_multiple_attempts() {
        let mut conn = create_test_connection();

        conn.start_attempt(ConnectionMethod::DirectP2P);
        conn.fail_attempt("Timeout".to_string());

        conn.start_attempt(ConnectionMethod::StunAssisted);
        conn.fail_attempt("No response".to_string());

        assert_eq!(conn.attempt_count(), 2);
        assert_eq!(conn.failed_attempts().len(), 2);
    }

    #[test]
    fn test_next_method() {
        let mut conn = create_test_connection();

        assert_eq!(conn.next_method(), Some(ConnectionMethod::DirectP2P));

        conn.start_attempt(ConnectionMethod::DirectP2P);
        conn.fail_attempt("Failed".to_string());

        assert_eq!(conn.next_method(), Some(ConnectionMethod::StunAssisted));

        conn.start_attempt(ConnectionMethod::StunAssisted);
        conn.fail_attempt("Failed".to_string());

        assert_eq!(conn.next_method(), Some(ConnectionMethod::TurnRelay));

        conn.start_attempt(ConnectionMethod::TurnRelay);
        conn.fail_attempt("Failed".to_string());

        assert_eq!(conn.next_method(), None);
        assert!(conn.all_methods_tried());
    }

    #[test]
    fn test_connection_method_properties() {
        assert_eq!(ConnectionMethod::DirectP2P.priority(), 100);
        assert_eq!(ConnectionMethod::StunAssisted.priority(), 80);
        assert_eq!(ConnectionMethod::TurnRelay.priority(), 50);

        assert!(!ConnectionMethod::DirectP2P.requires_relay());
        assert!(!ConnectionMethod::StunAssisted.requires_relay());
        assert!(ConnectionMethod::TurnRelay.requires_relay());
    }

    #[test]
    fn test_relay_address() {
        let mut conn = create_test_connection();
        let relay_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)), 3478);

        conn.start_attempt(ConnectionMethod::TurnRelay);

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::relay(addr1, "f1".to_string(), relay_addr);
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        conn.complete_attempt(candidate1, candidate2, Some(relay_addr));

        assert_eq!(conn.relay_address(), Some(relay_addr));
        assert_eq!(
            conn.successful_method(),
            Some(ConnectionMethod::TurnRelay)
        );
    }

    #[test]
    fn test_connection_close() {
        let mut conn = create_test_connection();

        conn.close();
        assert_eq!(conn.state(), ConnectionState::Closed);
        assert!(!conn.is_active());
    }

    #[test]
    fn test_connection_fail() {
        let mut conn = create_test_connection();

        conn.fail();
        assert_eq!(conn.state(), ConnectionState::Failed);
        assert!(!conn.is_active());
    }

    #[test]
    fn test_establishment_duration() {
        use std::thread;

        let mut conn = create_test_connection();

        thread::sleep(Duration::from_millis(50));

        conn.start_attempt(ConnectionMethod::DirectP2P);

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        conn.complete_attempt(candidate1, candidate2, None);

        let duration = conn.establishment_duration();
        assert!(duration.is_some());
        assert!(duration.unwrap() >= Duration::from_millis(50));
    }
}

// Made with Bob
