//! Transport Backend Port - Interface for transport implementations
//!
//! This trait defines the contract that all transport backends must implement

use crate::domain::{BackendCapabilities, SessionConfig, SessionId, TransportStats};
use crate::error::Result;
use async_trait::async_trait;
use std::net::SocketAddr;

/// Transport backend interface
///
/// This trait must be implemented by all transport backends (TCP, QUIC, io_uring, DPDK)
#[async_trait]
pub trait TransportBackend: Send + Sync {
    /// Get backend capabilities
    fn capabilities(&self) -> BackendCapabilities;

    /// Initialize the backend
    async fn initialize(&mut self) -> Result<()>;

    /// Shutdown the backend
    async fn shutdown(&mut self) -> Result<()>;

    /// Create a new session
    async fn create_session(&mut self, config: SessionConfig) -> Result<SessionId>;

    /// Start a session (begin listening or connecting)
    async fn start_session(&mut self, session_id: SessionId) -> Result<()>;

    /// Stop a session
    async fn stop_session(&mut self, session_id: SessionId) -> Result<()>;

    /// Send data on a session
    async fn send(&mut self, session_id: SessionId, data: &[u8]) -> Result<usize>;

    /// Receive data from a session
    async fn receive(&mut self, session_id: SessionId, buffer: &mut [u8]) -> Result<usize>;

    /// Get session statistics
    async fn get_stats(&self, session_id: SessionId) -> Result<TransportStats>;

    /// Check if session is active
    async fn is_session_active(&self, session_id: SessionId) -> Result<bool>;

    /// Get local address for a session
    async fn local_addr(&self, session_id: SessionId) -> Result<SocketAddr>;

    /// Get remote address for a session
    async fn remote_addr(&self, session_id: SessionId) -> Result<SocketAddr>;

    /// Pause a session (stop sending/receiving but maintain connection)
    async fn pause_session(&mut self, session_id: SessionId) -> Result<()>;

    /// Resume a paused session
    async fn resume_session(&mut self, session_id: SessionId) -> Result<()>;

    /// Get current throughput in bytes per second
    async fn get_throughput(&self, session_id: SessionId) -> Result<u64>;

    /// Get current round-trip time in milliseconds
    async fn get_rtt(&self, session_id: SessionId) -> Result<u64>;

    /// Check backend health
    async fn health_check(&self) -> Result<bool>;
}

/// Transport backend factory
///
/// Creates transport backends based on configuration
pub trait TransportBackendFactory: Send + Sync {
    /// Create a new backend instance
    fn create(&self) -> Result<Box<dyn TransportBackend>>;

    /// Get backend capabilities
    fn capabilities(&self) -> BackendCapabilities;

    /// Check if backend is available on current platform
    fn is_available(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SessionState, TransportType};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock backend for testing
    struct MockBackend {
        initialized: bool,
        sessions: std::collections::HashMap<SessionId, SessionState>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                initialized: false,
                sessions: std::collections::HashMap::new(),
            }
        }
    }

    #[async_trait]
    impl TransportBackend for MockBackend {
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::for_transport(TransportType::Tcp)
        }

        async fn initialize(&mut self) -> Result<()> {
            self.initialized = true;
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            self.initialized = false;
            self.sessions.clear();
            Ok(())
        }

        async fn create_session(&mut self, _config: SessionConfig) -> Result<SessionId> {
            let id = SessionId::new();
            self.sessions.insert(id, SessionState::Initializing);
            Ok(id)
        }

        async fn start_session(&mut self, session_id: SessionId) -> Result<()> {
            if let Some(state) = self.sessions.get_mut(&session_id) {
                *state = SessionState::Active;
                Ok(())
            } else {
                Err(crate::error::Error::Domain("Session not found".to_string()))
            }
        }

        async fn stop_session(&mut self, session_id: SessionId) -> Result<()> {
            self.sessions.remove(&session_id);
            Ok(())
        }

        async fn send(&mut self, _session_id: SessionId, data: &[u8]) -> Result<usize> {
            Ok(data.len())
        }

        async fn receive(&mut self, _session_id: SessionId, buffer: &mut [u8]) -> Result<usize> {
            Ok(buffer.len())
        }

        async fn get_stats(&self, _session_id: SessionId) -> Result<TransportStats> {
            Ok(TransportStats::new())
        }

        async fn is_session_active(&self, session_id: SessionId) -> Result<bool> {
            Ok(self
                .sessions
                .get(&session_id)
                .map(|s| *s == SessionState::Active)
                .unwrap_or(false))
        }

        async fn local_addr(&self, _session_id: SessionId) -> Result<SocketAddr> {
            Ok("127.0.0.1:8080".parse().unwrap())
        }

        async fn remote_addr(&self, _session_id: SessionId) -> Result<SocketAddr> {
            Ok("127.0.0.1:9090".parse().unwrap())
        }

        async fn pause_session(&mut self, session_id: SessionId) -> Result<()> {
            if let Some(state) = self.sessions.get_mut(&session_id) {
                *state = SessionState::Paused;
                Ok(())
            } else {
                Err(crate::error::Error::Domain("Session not found".to_string()))
            }
        }

        async fn resume_session(&mut self, session_id: SessionId) -> Result<()> {
            if let Some(state) = self.sessions.get_mut(&session_id) {
                *state = SessionState::Active;
                Ok(())
            } else {
                Err(crate::error::Error::Domain("Session not found".to_string()))
            }
        }

        async fn get_throughput(&self, _session_id: SessionId) -> Result<u64> {
            Ok(1_000_000_000) // 1 GB/s
        }

        async fn get_rtt(&self, _session_id: SessionId) -> Result<u64> {
            Ok(10) // 10ms
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(self.initialized)
        }
    }

    #[tokio::test]
    async fn test_mock_backend() {
        let mut backend = MockBackend::new();

        // Initialize
        backend.initialize().await.unwrap();
        assert!(backend.health_check().await.unwrap());

        // Create session
        let config = SessionConfig::default();
        let session_id = backend.create_session(config).await.unwrap();

        // Start session
        backend.start_session(session_id).await.unwrap();
        assert!(backend.is_session_active(session_id).await.unwrap());

        // Pause and resume
        backend.pause_session(session_id).await.unwrap();
        assert!(!backend.is_session_active(session_id).await.unwrap());

        backend.resume_session(session_id).await.unwrap();
        assert!(backend.is_session_active(session_id).await.unwrap());

        // Stop session
        backend.stop_session(session_id).await.unwrap();
        assert!(!backend.is_session_active(session_id).await.unwrap());

        // Shutdown
        backend.shutdown().await.unwrap();
        assert!(!backend.health_check().await.unwrap());
    }
}

// Made with Bob
