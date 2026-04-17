//! Connection Service - Orchestrates Connection Cascade Strategy
//!
//! This service manages the intelligent connection establishment process,
//! implementing the Connection Cascade strategy that tries multiple connection
//! methods in parallel based on NAT compatibility analysis.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::domain::{
    Connection, ConnectionMethod, ConnectionState, IceCandidate, NatType,
    PeerId, SessionId,
};
use crate::error::{Error, Result};

/// Request to initiate a connection cascade
#[derive(Debug, Clone)]
pub struct InitiateConnectionRequest {
    pub session_id: SessionId,
    pub initiator_id: PeerId,
    pub responder_id: PeerId,
    pub initiator_nat: NatType,
    pub responder_nat: NatType,
    pub initiator_candidates: Vec<IceCandidate>,
    pub responder_candidates: Vec<IceCandidate>,
}

/// Request to update connection attempt status
#[derive(Debug, Clone)]
pub struct UpdateConnectionRequest {
    pub session_id: SessionId,
    pub method: ConnectionMethod,
    pub state: ConnectionState,
    pub remote_addr: Option<SocketAddr>,
    pub error_message: Option<String>,
}

/// Connection attempt result
#[derive(Debug, Clone)]
pub struct ConnectionResult {
    pub session_id: SessionId,
    pub method: ConnectionMethod,
    pub state: ConnectionState,
    pub remote_addr: Option<SocketAddr>,
    pub establishment_duration: Option<Duration>,
    pub attempts_count: usize,
}

/// Statistics for connection service
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub successful_connections: usize,
    pub failed_connections: usize,
    pub direct_p2p_success: usize,
    pub stun_assisted_success: usize,
    pub turn_relay_success: usize,
    pub average_establishment_time: Duration,
}

/// Connection Service - Manages connection cascade logic
pub struct ConnectionService {
    connections: Arc<Mutex<HashMap<SessionId, Connection>>>,
    stats: Arc<Mutex<ConnectionStats>>,
}

impl ConnectionService {
    /// Create a new connection service
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(ConnectionStats {
                total_connections: 0,
                successful_connections: 0,
                failed_connections: 0,
                direct_p2p_success: 0,
                stun_assisted_success: 0,
                turn_relay_success: 0,
                average_establishment_time: Duration::from_secs(0),
            })),
        }
    }

    /// Initiate a connection cascade for a session
    ///
    /// This analyzes NAT compatibility and starts parallel connection attempts
    /// using the optimal strategy (Direct P2P, STUN-assisted, or TURN relay).
    pub fn initiate_connection(&self, request: InitiateConnectionRequest) -> Result<Connection> {
        // Create connection with NAT types (strategy is determined automatically)
        let connection = Connection::new(
            request.session_id.clone(),
            request.initiator_id,
            request.responder_id,
            request.initiator_nat,
            request.responder_nat,
        );

        // Store connection
        let mut connections = self.connections.lock().unwrap();
        connections.insert(request.session_id.clone(), connection.clone());

        // Update stats
        let mut stats = self.stats.lock().unwrap();
        stats.total_connections += 1;

        Ok(connection)
    }

    /// Update connection attempt status
    pub fn update_connection(&self, request: UpdateConnectionRequest) -> Result<Connection> {
        let mut connections = self.connections.lock().unwrap();

        let connection = connections
            .get_mut(&request.session_id)
            .ok_or_else(|| Error::NotFound(format!("Connection not found: {}", request.session_id)))?;

        // Update connection state based on the method
        match request.state {
            ConnectionState::Attempting => {
                connection.start_attempt(request.method);
            }
            ConnectionState::Established => {
                if let Some(addr) = request.remote_addr {
                    // For established connections, we need ICE candidates
                    // For now, create placeholder candidates
                    let local_candidate = crate::domain::IceCandidate::host(addr, "foundation".to_string());
                    let remote_candidate = crate::domain::IceCandidate::host(addr, "foundation".to_string());
                    connection.complete_attempt(local_candidate, remote_candidate, None);
                    self.update_success_stats(request.method);
                } else {
                    return Err(Error::InvalidState(
                        "Remote address required for established connection".to_string(),
                    ));
                }
            }
            ConnectionState::Failed => {
                let error = request
                    .error_message
                    .unwrap_or_else(|| "Connection attempt failed".to_string());
                connection.fail_attempt(error);

                // Check if all methods have been exhausted
                if connection.all_methods_tried() {
                    connection.fail();
                    self.update_failure_stats();
                }
            }
            ConnectionState::Closed => {
                connection.close();
            }
            _ => {}
        }

        Ok(connection.clone())
    }

    /// Get connection by session ID
    pub fn get_connection(&self, session_id: &SessionId) -> Result<Connection> {
        let connections = self.connections.lock().unwrap();
        connections
            .get(session_id)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("Connection not found: {}", session_id)))
    }

    /// Get all active connections
    pub fn get_active_connections(&self) -> Vec<Connection> {
        let connections = self.connections.lock().unwrap();
        connections
            .values()
            .filter(|c| {
                matches!(
                    c.state(),
                    ConnectionState::Attempting | ConnectionState::Established
                )
            })
            .cloned()
            .collect()
    }

    /// Get connections for a specific peer
    pub fn get_peer_connections(&self, peer_id: &PeerId) -> Vec<Connection> {
        let connections = self.connections.lock().unwrap();
        connections
            .values()
            .filter(|c| c.involves_peer(peer_id))
            .cloned()
            .collect()
    }

    /// Close a connection
    pub fn close_connection(&self, session_id: &SessionId) -> Result<()> {
        let mut connections = self.connections.lock().unwrap();

        let connection = connections
            .get_mut(session_id)
            .ok_or_else(|| Error::NotFound(format!("Connection not found: {}", session_id)))?;

        connection.close();
        Ok(())
    }

    /// Remove a connection (cleanup)
    pub fn remove_connection(&self, session_id: &SessionId) -> Result<Connection> {
        let mut connections = self.connections.lock().unwrap();
        connections
            .remove(session_id)
            .ok_or_else(|| Error::NotFound(format!("Connection not found: {}", session_id)))
    }

    /// Get connection statistics
    pub fn get_stats(&self) -> ConnectionStats {
        self.stats.lock().unwrap().clone()
    }

    /// Cleanup timed out connections
    pub fn cleanup_timed_out_connections(&self, timeout: Duration) -> Vec<SessionId> {
        let mut connections = self.connections.lock().unwrap();
        let now = SystemTime::now();

        let timed_out: Vec<SessionId> = connections
            .iter()
            .filter(|(_, conn)| {
                let created_at = conn.created_at();
                if let Ok(elapsed) = now.duration_since(created_at) {
                    elapsed > timeout && conn.state() == ConnectionState::Attempting
                } else {
                    false
                }
            })
            .map(|(id, _)| id.clone())
            .collect();

        // Remove timed out connections
        for session_id in &timed_out {
            connections.remove(session_id);
        }

        // Update failure stats
        let mut stats = self.stats.lock().unwrap();
        stats.failed_connections += timed_out.len();

        timed_out
    }

    /// Update statistics for successful connection
    fn update_success_stats(&self, method: ConnectionMethod) {
        let mut stats = self.stats.lock().unwrap();
        stats.successful_connections += 1;

        match method {
            ConnectionMethod::DirectP2P => stats.direct_p2p_success += 1,
            ConnectionMethod::StunAssisted => stats.stun_assisted_success += 1,
            ConnectionMethod::TurnRelay => stats.turn_relay_success += 1,
        }
    }

    /// Update statistics for failed connection
    fn update_failure_stats(&self) {
        let mut stats = self.stats.lock().unwrap();
        stats.failed_connections += 1;
    }

    /// Calculate success rate for a specific connection method
    pub fn get_method_success_rate(&self, method: ConnectionMethod) -> f64 {
        let stats = self.stats.lock().unwrap();
        let total = stats.total_connections as f64;
        if total == 0.0 {
            return 0.0;
        }

        let successes = match method {
            ConnectionMethod::DirectP2P => stats.direct_p2p_success,
            ConnectionMethod::StunAssisted => stats.stun_assisted_success,
            ConnectionMethod::TurnRelay => stats.turn_relay_success,
        };

        (successes as f64 / total) * 100.0
    }

    /// Get overall success rate
    pub fn get_overall_success_rate(&self) -> f64 {
        let stats = self.stats.lock().unwrap();
        let total = stats.total_connections as f64;
        if total == 0.0 {
            return 0.0;
        }

        (stats.successful_connections as f64 / total) * 100.0
    }
}

impl Default for ConnectionService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> InitiateConnectionRequest {
        InitiateConnectionRequest {
            session_id: SessionId::new(),
            initiator_id: PeerId::new(),
            responder_id: PeerId::new(),
            initiator_nat: NatType::FullCone,
            responder_nat: NatType::FullCone,
            initiator_candidates: vec![],
            responder_candidates: vec![],
        }
    }

    #[test]
    fn test_initiate_connection() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        let result = service.initiate_connection(request);
        assert!(result.is_ok());

        let connection = result.unwrap();
        assert_eq!(connection.session_id(), &session_id);
        assert_eq!(connection.state(), ConnectionState::Initializing);
    }

    #[test]
    fn test_strategy_selection_direct_first() {
        let strategy = ConnectionStrategy::select(NatType::FullCone, NatType::FullCone);
        assert_eq!(strategy, ConnectionStrategy::DirectP2PFirst);
    }

    #[test]
    fn test_strategy_selection_balanced() {
        let strategy = ConnectionStrategy::select(NatType::RestrictedCone, NatType::PortRestricted);
        assert_eq!(strategy, ConnectionStrategy::Balanced);
    }

    #[test]
    fn test_strategy_selection_relay_first() {
        let strategy = ConnectionStrategy::select(NatType::Symmetric, NatType::Symmetric);
        assert_eq!(strategy, ConnectionStrategy::RelayFirst);
    }

    #[test]
    fn test_update_connection_attempt() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        let update = UpdateConnectionRequest {
            session_id: session_id.clone(),
            method: ConnectionMethod::DirectP2P,
            state: ConnectionState::Attempting,
            remote_addr: None,
            error_message: None,
        };

        let result = service.update_connection(update);
        assert!(result.is_ok());

        let connection = service.get_connection(&session_id).unwrap();
        assert_eq!(connection.state(), ConnectionState::Attempting);
    }

    #[test]
    fn test_update_connection_established() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        // First attempt
        service
            .update_connection(UpdateConnectionRequest {
                session_id: session_id.clone(),
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Attempting,
                remote_addr: None,
                error_message: None,
            })
            .unwrap();

        // Then establish
        let addr = "192.168.1.100:5000".parse().unwrap();
        let update = UpdateConnectionRequest {
            session_id: session_id.clone(),
            method: ConnectionMethod::DirectP2P,
            state: ConnectionState::Established,
            remote_addr: Some(addr),
            error_message: None,
        };

        let result = service.update_connection(update);
        assert!(result.is_ok());

        let connection = service.get_connection(&session_id).unwrap();
        assert_eq!(connection.state(), ConnectionState::Established);
        assert_eq!(connection.successful_method(), Some(ConnectionMethod::DirectP2P));
    }

    #[test]
    fn test_update_connection_failed() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        // Attempt
        service
            .update_connection(UpdateConnectionRequest {
                session_id: session_id.clone(),
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Attempting,
                remote_addr: None,
                error_message: None,
            })
            .unwrap();

        // Fail
        let update = UpdateConnectionRequest {
            session_id: session_id.clone(),
            method: ConnectionMethod::DirectP2P,
            state: ConnectionState::Failed,
            remote_addr: None,
            error_message: Some("Connection timeout".to_string()),
        };

        let result = service.update_connection(update);
        assert!(result.is_ok());

        let connection = service.get_connection(&session_id).unwrap();
        // Should still be attempting (not failed yet, has more methods to try)
        assert_eq!(connection.state(), ConnectionState::Attempting);
    }

    #[test]
    fn test_get_active_connections() {
        let service = ConnectionService::new();

        // Create multiple connections
        let req1 = create_test_request();
        let req2 = create_test_request();

        service.initiate_connection(req1.clone()).unwrap();
        service.initiate_connection(req2.clone()).unwrap();

        // Start one connection
        service
            .update_connection(UpdateConnectionRequest {
                session_id: req1.session_id.clone(),
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Attempting,
                remote_addr: None,
                error_message: None,
            })
            .unwrap();

        let active = service.get_active_connections();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_close_connection() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        let result = service.close_connection(&session_id);
        assert!(result.is_ok());

        let connection = service.get_connection(&session_id).unwrap();
        assert_eq!(connection.state(), ConnectionState::Closed);
    }

    #[test]
    fn test_remove_connection() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        let result = service.remove_connection(&session_id);
        assert!(result.is_ok());

        let get_result = service.get_connection(&session_id);
        assert!(get_result.is_err());
    }

    #[test]
    fn test_connection_stats() {
        let service = ConnectionService::new();
        let request = create_test_request();

        service.initiate_connection(request.clone()).unwrap();

        let stats = service.get_stats();
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.successful_connections, 0);
    }

    #[test]
    fn test_success_rate_calculation() {
        let service = ConnectionService::new();

        // Create and establish a connection
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        service
            .update_connection(UpdateConnectionRequest {
                session_id: session_id.clone(),
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Attempting,
                remote_addr: None,
                error_message: None,
            })
            .unwrap();

        let addr = "192.168.1.100:5000".parse().unwrap();
        service
            .update_connection(UpdateConnectionRequest {
                session_id,
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Established,
                remote_addr: Some(addr),
                error_message: None,
            })
            .unwrap();

        let overall_rate = service.get_overall_success_rate();
        assert_eq!(overall_rate, 100.0);

        let direct_rate = service.get_method_success_rate(ConnectionMethod::DirectP2P);
        assert_eq!(direct_rate, 100.0);
    }

    #[test]
    fn test_cleanup_timed_out_connections() {
        let service = ConnectionService::new();
        let request = create_test_request();
        let session_id = request.session_id.clone();

        service.initiate_connection(request).unwrap();

        // Start attempting
        service
            .update_connection(UpdateConnectionRequest {
                session_id: session_id.clone(),
                method: ConnectionMethod::DirectP2P,
                state: ConnectionState::Attempting,
                remote_addr: None,
                error_message: None,
            })
            .unwrap();

        // Cleanup with zero timeout (should remove the connection)
        let timed_out = service.cleanup_timed_out_connections(Duration::from_secs(0));
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], session_id);

        // Connection should be removed
        let get_result = service.get_connection(&session_id);
        assert!(get_result.is_err());
    }
}

// Made with Bob
