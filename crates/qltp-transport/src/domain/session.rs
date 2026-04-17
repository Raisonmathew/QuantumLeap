//! Transport Session - Aggregate Root
//!
//! Represents a complete transport session with state management

use crate::domain::{
    session_state::SessionState,
    transport_stats::TransportStats,
    transport_type::TransportType,
};
use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

/// Session identifier (Value Object)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Create a new random session ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to string
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    /// Parse from string
    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Transport session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Transport type to use
    pub transport_type: TransportType,
    /// Local address to bind to
    pub local_addr: SocketAddr,
    /// Remote address to connect to
    pub remote_addr: SocketAddr,
    /// Maximum transfer size in bytes
    pub max_transfer_size: u64,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Enable compression
    pub enable_compression: bool,
    /// Enable encryption
    pub enable_encryption: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            transport_type: TransportType::Tcp,
            local_addr: "0.0.0.0:0".parse().unwrap(),
            remote_addr: "127.0.0.1:8080".parse().unwrap(),
            max_transfer_size: 10_000_000_000, // 10GB
            connection_timeout_secs: 30,
            enable_compression: true,
            enable_encryption: true,
        }
    }
}

/// Transport session (Aggregate Root)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportSession {
    /// Unique session identifier
    id: SessionId,
    /// Session configuration
    config: SessionConfig,
    /// Current session state
    state: SessionState,
    /// Session statistics
    stats: TransportStats,
    /// Session creation time
    created_at: DateTime<Utc>,
    /// Last state change time
    updated_at: DateTime<Utc>,
    /// Session completion time
    completed_at: Option<DateTime<Utc>>,
    /// Error message if failed
    error_message: Option<String>,
}

impl TransportSession {
    /// Create a new transport session
    pub fn new(config: SessionConfig) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            config,
            state: SessionState::Initializing,
            stats: TransportStats::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            error_message: None,
        }
    }

    /// Get session ID
    pub fn id(&self) -> SessionId {
        self.id
    }

    /// Get session configuration
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Get current state
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get session statistics
    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    /// Get mutable statistics (for internal updates)
    pub fn stats_mut(&mut self) -> &mut TransportStats {
        &mut self.stats
    }

    /// Get creation time
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Get last update time
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Get completion time
    pub fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }

    /// Get error message
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    /// Get session duration
    pub fn duration(&self) -> chrono::Duration {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        end - self.created_at
    }

    /// Transition to a new state
    pub fn transition_to(&mut self, new_state: SessionState) -> Result<()> {
        if !self.state.can_transition_to(new_state) {
            return Err(crate::error::Error::InvalidStateTransition {
                from: self.state,
                to: new_state,
            });
        }
        
        self.state = new_state;
        self.updated_at = Utc::now();

        // Set completion time for terminal states
        if new_state.is_terminal() {
            self.completed_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Start the session (transition to Active)
    pub fn start(&mut self) -> Result<()> {
        self.transition_to(SessionState::Active)
    }

    /// Pause the session
    pub fn pause(&mut self) -> Result<()> {
        self.transition_to(SessionState::Paused)
    }

    /// Resume the session
    pub fn resume(&mut self) -> Result<()> {
        self.transition_to(SessionState::Active)
    }

    /// Complete the session successfully
    pub fn complete(&mut self) -> Result<()> {
        self.transition_to(SessionState::Completed)
    }

    /// Fail the session with an error message
    pub fn fail(&mut self, error: String) -> Result<()> {
        self.error_message = Some(error);
        self.transition_to(SessionState::Failed)
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    /// Check if session is completed
    pub fn is_completed(&self) -> bool {
        self.state == SessionState::Completed
    }

    /// Check if session has failed
    pub fn has_failed(&self) -> bool {
        self.state == SessionState::Failed
    }

    /// Check if session is in a terminal state
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Record a send operation
    pub fn record_send(&mut self, bytes: u64) {
        self.stats.record_send(bytes);
        self.updated_at = Utc::now();
    }

    /// Record a receive operation
    pub fn record_receive(&mut self, bytes: u64) {
        self.stats.record_receive(bytes);
        self.updated_at = Utc::now();
    }

    /// Record a packet loss
    pub fn record_packet_loss(&mut self) {
        self.stats.record_packet_loss();
        self.updated_at = Utc::now();
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.stats.record_error();
        self.updated_at = Utc::now();
    }

    /// Update performance metrics
    pub fn update_metrics(&mut self, rtt_ms: u64, throughput_bps: u64, cpu_percent: f32) {
        self.stats.update_rtt(rtt_ms);
        self.stats.update_throughput(throughput_bps);
        self.stats.update_cpu_usage(cpu_percent);
        self.updated_at = Utc::now();
    }

    /// Check if session is healthy
    pub fn is_healthy(&self) -> bool {
        self.is_active() && self.stats.is_healthy()
    }
}

impl std::fmt::Display for TransportSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session {} [{}] - {} - Duration: {}s",
            self.id,
            self.config.transport_type,
            self.state,
            self.duration().num_seconds()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_creation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_string_conversion() {
        let id = SessionId::new();
        let s = id.to_string();
        let parsed = SessionId::from_string(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = TransportSession::new(config);
        
        assert_eq!(session.state(), SessionState::Initializing);
        assert!(!session.is_active());
        assert!(!session.is_completed());
    }

    #[test]
    fn test_session_state_transitions() {
        let config = SessionConfig::default();
        let mut session = TransportSession::new(config);
        
        // Start session
        session.start().unwrap();
        assert!(session.is_active());
        
        // Pause session
        session.pause().unwrap();
        assert_eq!(session.state(), SessionState::Paused);
        
        // Resume session
        session.resume().unwrap();
        assert!(session.is_active());
        
        // Complete session
        session.complete().unwrap();
        assert!(session.is_completed());
        assert!(session.is_terminal());
        assert!(session.completed_at().is_some());
    }

    #[test]
    fn test_session_failure() {
        let config = SessionConfig::default();
        let mut session = TransportSession::new(config);
        
        session.start().unwrap();
        session.fail("Connection lost".to_string()).unwrap();
        
        assert!(session.has_failed());
        assert_eq!(session.error_message(), Some("Connection lost"));
    }

    #[test]
    fn test_session_statistics() {
        let config = SessionConfig::default();
        let mut session = TransportSession::new(config);
        
        session.record_send(1000);
        session.record_receive(2000);
        
        assert_eq!(session.stats().bytes_sent, 1000);
        assert_eq!(session.stats().bytes_received, 2000);
        assert_eq!(session.stats().bytes_transferred, 3000);
    }

    #[test]
    fn test_session_health_check() {
        let config = SessionConfig::default();
        let mut session = TransportSession::new(config);
        
        session.start().unwrap();
        
        // Healthy session
        session.record_send(1000);
        assert!(session.is_healthy());
        
        // Unhealthy session (too many errors)
        for _ in 0..100 {
            session.record_error();
        }
        assert!(!session.is_healthy());
    }
}

// Made with Bob
