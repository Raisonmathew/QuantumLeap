//! Session Entity
//!
//! Represents a connection session between two peers. This is an entity in DDD terms -
//! it has identity (SessionId) and lifecycle, coordinating the connection between peers.

use super::ice_candidate::IceCandidate;
use super::peer_id::PeerId;
use super::session_id::SessionId;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session is being initialized
    Initializing,
    /// Gathering ICE candidates
    GatheringCandidates,
    /// Exchanging ICE candidates between peers
    ExchangingCandidates,
    /// Performing connectivity checks
    ConnectivityCheck,
    /// Session is established and active
    Active,
    /// Session is being closed
    Closing,
    /// Session is closed
    Closed,
    /// Session failed to establish
    Failed,
}

impl SessionState {
    /// Check if session is in an active state
    pub fn is_active(&self) -> bool {
        !matches!(self, SessionState::Closed | SessionState::Failed)
    }

    /// Check if session is established
    pub fn is_established(&self) -> bool {
        matches!(self, SessionState::Active)
    }

    /// Check if session is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Closed | SessionState::Failed)
    }

    /// Check if session is in negotiation phase
    pub fn is_negotiating(&self) -> bool {
        matches!(
            self,
            SessionState::Initializing
                | SessionState::GatheringCandidates
                | SessionState::ExchangingCandidates
                | SessionState::ConnectivityCheck
        )
    }
}

/// Session type - how peers are connected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionType {
    /// Direct peer-to-peer connection
    DirectP2P,
    /// STUN-assisted connection
    StunAssisted,
    /// Relayed through TURN server
    Relayed,
}

impl SessionType {
    /// Check if session uses relay
    pub fn uses_relay(&self) -> bool {
        matches!(self, SessionType::Relayed)
    }

    /// Check if session is direct
    pub fn is_direct(&self) -> bool {
        matches!(self, SessionType::DirectP2P)
    }

    /// Get performance score (higher = better)
    pub fn performance_score(&self) -> u8 {
        match self {
            SessionType::DirectP2P => 100,
            SessionType::StunAssisted => 80,
            SessionType::Relayed => 50,
        }
    }
}

/// Session entity - represents a connection session between two peers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    id: SessionId,
    /// Initiator peer ID
    initiator_id: PeerId,
    /// Responder peer ID
    responder_id: PeerId,
    /// Current session state
    state: SessionState,
    /// Session type (how peers are connected)
    session_type: Option<SessionType>,
    /// Initiator's ICE candidates
    initiator_candidates: Vec<IceCandidate>,
    /// Responder's ICE candidates
    responder_candidates: Vec<IceCandidate>,
    /// Selected candidate pair (if established)
    selected_pair: Option<(IceCandidate, IceCandidate)>,
    /// Time when session was created
    created_at: SystemTime,
    /// Time when session was established (if active)
    established_at: Option<SystemTime>,
    /// Time when session was closed (if closed)
    closed_at: Option<SystemTime>,
    /// Time of last activity
    last_activity: SystemTime,
    /// Total bytes transferred in this session
    bytes_transferred: u64,
    /// Number of connection attempts
    connection_attempts: u32,
    /// Error message (if failed)
    error_message: Option<String>,
}

impl Session {
    /// Create a new session
    pub fn new(initiator_id: PeerId, responder_id: PeerId) -> Self {
        let now = SystemTime::now();
        Self {
            id: SessionId::new(),
            initiator_id,
            responder_id,
            state: SessionState::Initializing,
            session_type: None,
            initiator_candidates: Vec::new(),
            responder_candidates: Vec::new(),
            selected_pair: None,
            created_at: now,
            established_at: None,
            closed_at: None,
            last_activity: now,
            bytes_transferred: 0,
            connection_attempts: 0,
            error_message: None,
        }
    }

    /// Get session ID
    pub fn id(&self) -> &SessionId {
        &self.id
    }

    /// Get initiator peer ID
    pub fn initiator_id(&self) -> &PeerId {
        &self.initiator_id
    }

    /// Get responder peer ID
    pub fn responder_id(&self) -> &PeerId {
        &self.responder_id
    }

    /// Get current state
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get session type
    pub fn session_type(&self) -> Option<SessionType> {
        self.session_type
    }

    /// Get initiator candidates
    pub fn initiator_candidates(&self) -> &[IceCandidate] {
        &self.initiator_candidates
    }

    /// Get responder candidates
    pub fn responder_candidates(&self) -> &[IceCandidate] {
        &self.responder_candidates
    }

    /// Get selected candidate pair
    pub fn selected_pair(&self) -> Option<&(IceCandidate, IceCandidate)> {
        self.selected_pair.as_ref()
    }

    /// Get creation time
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Get establishment time
    pub fn established_at(&self) -> Option<SystemTime> {
        self.established_at
    }

    /// Get close time
    pub fn closed_at(&self) -> Option<SystemTime> {
        self.closed_at
    }

    /// Get last activity time
    pub fn last_activity(&self) -> SystemTime {
        self.last_activity
    }

    /// Get bytes transferred
    pub fn bytes_transferred(&self) -> u64 {
        self.bytes_transferred
    }

    /// Get connection attempts
    pub fn connection_attempts(&self) -> u32 {
        self.connection_attempts
    }

    /// Get error message
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Check if session is established
    pub fn is_established(&self) -> bool {
        self.state.is_established()
    }

    /// Check if peer is part of this session
    pub fn involves_peer(&self, peer_id: &PeerId) -> bool {
        &self.initiator_id == peer_id || &self.responder_id == peer_id
    }

    /// Get the other peer ID in the session
    pub fn other_peer(&self, peer_id: &PeerId) -> Option<&PeerId> {
        if &self.initiator_id == peer_id {
            Some(&self.responder_id)
        } else if &self.responder_id == peer_id {
            Some(&self.initiator_id)
        } else {
            None
        }
    }

    /// Check if session has timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        if let Ok(elapsed) = self.last_activity.elapsed() {
            elapsed > timeout
        } else {
            false
        }
    }

    /// Get session duration
    pub fn duration(&self) -> Option<Duration> {
        self.created_at.elapsed().ok()
    }

    /// Get establishment duration (time to establish connection)
    pub fn establishment_duration(&self) -> Option<Duration> {
        self.established_at
            .and_then(|est| est.duration_since(self.created_at).ok())
    }

    /// Add initiator ICE candidate
    pub fn add_initiator_candidate(&mut self, candidate: IceCandidate) {
        if !self.initiator_candidates.contains(&candidate) {
            self.initiator_candidates.push(candidate);
            self.update_activity();
        }
    }

    /// Add responder ICE candidate
    pub fn add_responder_candidate(&mut self, candidate: IceCandidate) {
        if !self.responder_candidates.contains(&candidate) {
            self.responder_candidates.push(candidate);
            self.update_activity();
        }
    }

    /// Start gathering candidates
    pub fn start_gathering(&mut self) {
        if self.state == SessionState::Initializing {
            self.state = SessionState::GatheringCandidates;
            self.update_activity();
        }
    }

    /// Start exchanging candidates
    pub fn start_exchanging(&mut self) {
        if self.state == SessionState::GatheringCandidates {
            self.state = SessionState::ExchangingCandidates;
            self.update_activity();
        }
    }

    /// Start connectivity check
    pub fn start_connectivity_check(&mut self) {
        if self.state == SessionState::ExchangingCandidates {
            self.state = SessionState::ConnectivityCheck;
            self.connection_attempts += 1;
            self.update_activity();
        }
    }

    /// Establish session with selected candidate pair
    pub fn establish(
        &mut self,
        session_type: SessionType,
        initiator_candidate: IceCandidate,
        responder_candidate: IceCandidate,
    ) {
        if self.state.is_negotiating() {
            self.state = SessionState::Active;
            self.session_type = Some(session_type);
            self.selected_pair = Some((initiator_candidate, responder_candidate));
            self.established_at = Some(SystemTime::now());
            self.update_activity();
        }
    }

    /// Close session
    pub fn close(&mut self) {
        if self.state.is_active() {
            self.state = SessionState::Closing;
            self.update_activity();
        }
    }

    /// Mark session as closed
    pub fn closed(&mut self) {
        self.state = SessionState::Closed;
        self.closed_at = Some(SystemTime::now());
        self.update_activity();
    }

    /// Mark session as failed
    pub fn fail(&mut self, error: String) {
        self.state = SessionState::Failed;
        self.error_message = Some(error);
        self.closed_at = Some(SystemTime::now());
        self.update_activity();
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Add bytes transferred
    pub fn add_bytes_transferred(&mut self, bytes: u64) {
        self.bytes_transferred = self.bytes_transferred.saturating_add(bytes);
        self.update_activity();
    }

    /// Increment connection attempts
    pub fn increment_attempts(&mut self) {
        self.connection_attempts += 1;
        self.update_activity();
    }

    /// Get session summary
    pub fn summary(&self) -> String {
        format!(
            "Session {} - {} <-> {} - State: {:?}, Type: {:?}, Bytes: {}",
            self.id,
            self.initiator_id,
            self.responder_id,
            self.state,
            self.session_type,
            self.bytes_transferred
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::thread;

    fn create_test_session() -> Session {
        let initiator = PeerId::new();
        let responder = PeerId::new();
        Session::new(initiator, responder)
    }

    #[test]
    fn test_new_session() {
        let session = create_test_session();

        assert_eq!(session.state(), SessionState::Initializing);
        assert!(session.session_type().is_none());
        assert_eq!(session.bytes_transferred(), 0);
        assert_eq!(session.connection_attempts(), 0);
        assert!(session.is_active());
        assert!(!session.is_established());
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = create_test_session();

        session.start_gathering();
        assert_eq!(session.state(), SessionState::GatheringCandidates);

        session.start_exchanging();
        assert_eq!(session.state(), SessionState::ExchangingCandidates);

        session.start_connectivity_check();
        assert_eq!(session.state(), SessionState::ConnectivityCheck);
        assert_eq!(session.connection_attempts(), 1);

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        session.establish(SessionType::DirectP2P, candidate1, candidate2);
        assert_eq!(session.state(), SessionState::Active);
        assert!(session.is_established());
        assert_eq!(session.session_type(), Some(SessionType::DirectP2P));
        assert!(session.established_at().is_some());
    }

    #[test]
    fn test_session_close() {
        let mut session = create_test_session();
        session.start_gathering();

        session.close();
        assert_eq!(session.state(), SessionState::Closing);

        session.closed();
        assert_eq!(session.state(), SessionState::Closed);
        assert!(!session.is_active());
        assert!(session.closed_at().is_some());
    }

    #[test]
    fn test_session_fail() {
        let mut session = create_test_session();

        session.fail("Connection timeout".to_string());
        assert_eq!(session.state(), SessionState::Failed);
        assert_eq!(session.error_message(), Some("Connection timeout"));
        assert!(!session.is_active());
    }

    #[test]
    fn test_ice_candidates() {
        let mut session = create_test_session();

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        session.add_initiator_candidate(candidate1.clone());
        session.add_responder_candidate(candidate2.clone());

        assert_eq!(session.initiator_candidates().len(), 1);
        assert_eq!(session.responder_candidates().len(), 1);

        // Adding duplicate should not increase count
        session.add_initiator_candidate(candidate1);
        assert_eq!(session.initiator_candidates().len(), 1);
    }

    #[test]
    fn test_involves_peer() {
        let initiator = PeerId::new();
        let responder = PeerId::new();
        let other = PeerId::new();
        let session = Session::new(initiator.clone(), responder.clone());

        assert!(session.involves_peer(&initiator));
        assert!(session.involves_peer(&responder));
        assert!(!session.involves_peer(&other));
    }

    #[test]
    fn test_other_peer() {
        let initiator = PeerId::new();
        let responder = PeerId::new();
        let session = Session::new(initiator.clone(), responder.clone());

        assert_eq!(session.other_peer(&initiator), Some(&responder));
        assert_eq!(session.other_peer(&responder), Some(&initiator));

        let other = PeerId::new();
        assert_eq!(session.other_peer(&other), None);
    }

    #[test]
    fn test_bytes_transferred() {
        let mut session = create_test_session();

        session.add_bytes_transferred(1000);
        assert_eq!(session.bytes_transferred(), 1000);

        session.add_bytes_transferred(500);
        assert_eq!(session.bytes_transferred(), 1500);
    }

    #[test]
    fn test_timeout() {
        let mut session = create_test_session();

        assert!(!session.is_timed_out(Duration::from_secs(1)));

        thread::sleep(Duration::from_millis(100));
        assert!(session.is_timed_out(Duration::from_millis(50)));

        session.update_activity();
        assert!(!session.is_timed_out(Duration::from_secs(1)));
    }

    #[test]
    fn test_session_type_properties() {
        assert!(SessionType::DirectP2P.is_direct());
        assert!(!SessionType::DirectP2P.uses_relay());
        assert_eq!(SessionType::DirectP2P.performance_score(), 100);

        assert!(!SessionType::StunAssisted.is_direct());
        assert!(!SessionType::StunAssisted.uses_relay());
        assert_eq!(SessionType::StunAssisted.performance_score(), 80);

        assert!(!SessionType::Relayed.is_direct());
        assert!(SessionType::Relayed.uses_relay());
        assert_eq!(SessionType::Relayed.performance_score(), 50);
    }

    #[test]
    fn test_establishment_duration() {
        let mut session = create_test_session();

        assert!(session.establishment_duration().is_none());

        thread::sleep(Duration::from_millis(50));

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        session.establish(SessionType::DirectP2P, candidate1, candidate2);

        let duration = session.establishment_duration();
        assert!(duration.is_some());
        assert!(duration.unwrap() >= Duration::from_millis(50));
    }

    #[test]
    fn test_session_summary() {
        let session = create_test_session();
        let summary = session.summary();

        assert!(summary.contains("Session"));
        assert!(summary.contains("Initializing"));
    }
}

// Made with Bob
