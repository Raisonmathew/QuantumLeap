//! Session Service - Application Layer
//!
//! Orchestrates session-related use cases and manages session lifecycle.
//! Coordinates ICE candidate exchange and session negotiation between peers.

use crate::domain::{
    IceCandidate, PeerId, Session, SessionId, SessionState, SessionType,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Error types for session service operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum SessionServiceError {
    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),
    
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(SessionId),
    
    #[error("Invalid session state: expected {expected:?}, got {actual:?}")]
    InvalidState {
        expected: SessionState,
        actual: SessionState,
    },
    
    #[error("Session timed out: {0}")]
    SessionTimedOut(SessionId),
    
    #[error("Peer not in session: {0}")]
    PeerNotInSession(PeerId),
    
    #[error("Session establishment failed: {0}")]
    EstablishmentFailed(String),
}

pub type Result<T> = std::result::Result<T, SessionServiceError>;

/// Session creation request
#[derive(Debug, Clone)]
pub struct CreateSessionRequest {
    pub initiator_id: PeerId,
    pub responder_id: PeerId,
}

/// Session service - manages session lifecycle and ICE exchange
pub struct SessionService {
    /// In-memory session registry
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    /// Timeout duration for inactive sessions
    timeout_duration: Duration,
}

impl SessionService {
    /// Create a new session service
    pub fn new(timeout_duration: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            timeout_duration,
        }
    }

    /// Create a new session between two peers
    pub fn create_session(&self, request: CreateSessionRequest) -> Result<SessionId> {
        let mut sessions = self.sessions.write().unwrap();

        let session = Session::new(request.initiator_id, request.responder_id);
        let session_id = session.id().clone();

        sessions.insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &SessionId) -> Result<Session> {
        let sessions = self.sessions.read().unwrap();

        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))
    }

    /// Start gathering ICE candidates for a session
    pub fn start_gathering(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.start_gathering();
        Ok(())
    }

    /// Start exchanging ICE candidates
    pub fn start_exchanging(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.start_exchanging();
        Ok(())
    }

    /// Start connectivity check
    pub fn start_connectivity_check(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.start_connectivity_check();
        Ok(())
    }

    /// Add initiator ICE candidate
    pub fn add_initiator_candidate(
        &self,
        session_id: &SessionId,
        candidate: IceCandidate,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.add_initiator_candidate(candidate);
        Ok(())
    }

    /// Add responder ICE candidate
    pub fn add_responder_candidate(
        &self,
        session_id: &SessionId,
        candidate: IceCandidate,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.add_responder_candidate(candidate);
        Ok(())
    }

    /// Get initiator candidates
    pub fn get_initiator_candidates(&self, session_id: &SessionId) -> Result<Vec<IceCandidate>> {
        let sessions = self.sessions.read().unwrap();

        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        Ok(session.initiator_candidates().to_vec())
    }

    /// Get responder candidates
    pub fn get_responder_candidates(&self, session_id: &SessionId) -> Result<Vec<IceCandidate>> {
        let sessions = self.sessions.read().unwrap();

        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        Ok(session.responder_candidates().to_vec())
    }

    /// Establish session with selected candidate pair
    pub fn establish_session(
        &self,
        session_id: &SessionId,
        session_type: SessionType,
        initiator_candidate: IceCandidate,
        responder_candidate: IceCandidate,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.establish(session_type, initiator_candidate, responder_candidate);
        Ok(())
    }

    /// Close a session
    pub fn close_session(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.close();
        Ok(())
    }

    /// Mark session as closed
    pub fn mark_closed(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.closed();
        Ok(())
    }

    /// Fail a session
    pub fn fail_session(&self, session_id: &SessionId, error: String) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.fail(error);
        Ok(())
    }

    /// Remove a session
    pub fn remove_session(&self, session_id: &SessionId) -> Result<Session> {
        let mut sessions = self.sessions.write().unwrap();

        sessions
            .remove(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))
    }

    /// Update session activity
    pub fn update_activity(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.update_activity();
        Ok(())
    }

    /// Add bytes transferred
    pub fn add_bytes_transferred(&self, session_id: &SessionId, bytes: u64) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.add_bytes_transferred(bytes);
        Ok(())
    }

    /// Increment connection attempts
    pub fn increment_attempts(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().unwrap();

        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        session.increment_attempts();
        Ok(())
    }

    /// Get all active sessions
    pub fn get_active_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    /// Get all established sessions
    pub fn get_established_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.is_established())
            .cloned()
            .collect()
    }

    /// Get sessions for a specific peer
    pub fn get_peer_sessions(&self, peer_id: &PeerId) -> Vec<Session> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.involves_peer(peer_id))
            .cloned()
            .collect()
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.len()
    }

    /// Get active session count
    pub fn active_session_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.values().filter(|s| s.is_active()).count()
    }

    /// Clean up timed out sessions
    pub fn cleanup_timed_out_sessions(&self) -> Vec<SessionId> {
        let mut sessions = self.sessions.write().unwrap();
        let mut timed_out = Vec::new();

        // Find timed out sessions
        let to_remove: Vec<SessionId> = sessions
            .iter()
            .filter(|(_, session)| session.is_timed_out(self.timeout_duration))
            .map(|(id, _)| id.clone())
            .collect();

        // Remove them
        for session_id in to_remove {
            if let Some(mut session) = sessions.remove(&session_id) {
                session.fail("Session timed out".to_string());
                timed_out.push(session_id);
            }
        }

        timed_out
    }

    /// Get session statistics
    pub fn get_session_stats(&self, session_id: &SessionId) -> Result<SessionStats> {
        let sessions = self.sessions.read().unwrap();

        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionServiceError::SessionNotFound(session_id.clone()))?;

        Ok(SessionStats {
            session_id: session.id().clone(),
            initiator_id: session.initiator_id().clone(),
            responder_id: session.responder_id().clone(),
            state: session.state(),
            session_type: session.session_type(),
            bytes_transferred: session.bytes_transferred(),
            connection_attempts: session.connection_attempts(),
            establishment_duration: session.establishment_duration(),
        })
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub session_id: SessionId,
    pub initiator_id: PeerId,
    pub responder_id: PeerId,
    pub state: SessionState,
    pub session_type: Option<SessionType>,
    pub bytes_transferred: u64,
    pub connection_attempts: u32,
    pub establishment_duration: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn create_test_service() -> SessionService {
        SessionService::new(Duration::from_secs(30))
    }

    fn create_test_request() -> CreateSessionRequest {
        CreateSessionRequest {
            initiator_id: PeerId::new(),
            responder_id: PeerId::new(),
        }
    }

    #[test]
    fn test_create_session() {
        let service = create_test_service();
        let request = create_test_request();

        let result = service.create_session(request);
        assert!(result.is_ok());
        assert_eq!(service.session_count(), 1);
    }

    #[test]
    fn test_get_session() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request.clone()).unwrap();

        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.id(), &session_id);
        assert_eq!(session.initiator_id(), &request.initiator_id);
        assert_eq!(session.responder_id(), &request.responder_id);
    }

    #[test]
    fn test_session_state_transitions() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        service.start_gathering(&session_id).unwrap();
        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::GatheringCandidates);

        service.start_exchanging(&session_id).unwrap();
        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::ExchangingCandidates);

        service.start_connectivity_check(&session_id).unwrap();
        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::ConnectivityCheck);
    }

    #[test]
    fn test_add_ice_candidates() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        service
            .add_initiator_candidate(&session_id, candidate1)
            .unwrap();
        service
            .add_responder_candidate(&session_id, candidate2)
            .unwrap();

        let initiator_candidates = service.get_initiator_candidates(&session_id).unwrap();
        let responder_candidates = service.get_responder_candidates(&session_id).unwrap();

        assert_eq!(initiator_candidates.len(), 1);
        assert_eq!(responder_candidates.len(), 1);
    }

    #[test]
    fn test_establish_session() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 8080);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 8081);
        let candidate1 = IceCandidate::host(addr1, "f1".to_string());
        let candidate2 = IceCandidate::host(addr2, "f2".to_string());

        service
            .establish_session(&session_id, SessionType::DirectP2P, candidate1, candidate2)
            .unwrap();

        let session = service.get_session(&session_id).unwrap();
        assert!(session.is_established());
        assert_eq!(session.session_type(), Some(SessionType::DirectP2P));
    }

    #[test]
    fn test_close_session() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        service.close_session(&session_id).unwrap();
        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::Closing);

        service.mark_closed(&session_id).unwrap();
        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::Closed);
    }

    #[test]
    fn test_fail_session() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        service
            .fail_session(&session_id, "Connection failed".to_string())
            .unwrap();

        let session = service.get_session(&session_id).unwrap();
        assert_eq!(session.state(), SessionState::Failed);
        assert_eq!(session.error_message(), Some("Connection failed"));
    }

    #[test]
    fn test_remove_session() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request).unwrap();

        let removed = service.remove_session(&session_id);
        assert!(removed.is_ok());
        assert_eq!(service.session_count(), 0);
    }

    #[test]
    fn test_get_active_sessions() {
        let service = create_test_service();

        let request1 = create_test_request();
        service.create_session(request1).unwrap();

        let request2 = create_test_request();
        let session_id2 = service.create_session(request2).unwrap();
        service.close_session(&session_id2).unwrap();
        service.mark_closed(&session_id2).unwrap();

        let active = service.get_active_sessions();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_get_peer_sessions() {
        let service = create_test_service();
        let peer_id = PeerId::new();

        let request1 = CreateSessionRequest {
            initiator_id: peer_id.clone(),
            responder_id: PeerId::new(),
        };
        service.create_session(request1).unwrap();

        let request2 = CreateSessionRequest {
            initiator_id: PeerId::new(),
            responder_id: peer_id.clone(),
        };
        service.create_session(request2).unwrap();

        let request3 = CreateSessionRequest {
            initiator_id: PeerId::new(),
            responder_id: PeerId::new(),
        };
        service.create_session(request3).unwrap();

        let peer_sessions = service.get_peer_sessions(&peer_id);
        assert_eq!(peer_sessions.len(), 2);
    }

    #[test]
    fn test_session_stats() {
        let service = create_test_service();
        let request = create_test_request();
        let session_id = service.create_session(request.clone()).unwrap();

        service.add_bytes_transferred(&session_id, 1000).unwrap();
        service.increment_attempts(&session_id).unwrap();

        let stats = service.get_session_stats(&session_id).unwrap();
        assert_eq!(stats.session_id, session_id);
        assert_eq!(stats.initiator_id, request.initiator_id);
        assert_eq!(stats.responder_id, request.responder_id);
        assert_eq!(stats.bytes_transferred, 1000);
        assert_eq!(stats.connection_attempts, 1);
    }
}

// Made with Bob
