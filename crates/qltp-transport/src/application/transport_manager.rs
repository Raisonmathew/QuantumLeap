//! Transport Manager - Application Service
//!
//! Orchestrates transport operations and manages backend selection

use crate::application::{
    BackendMonitor, BackendSelector, FallbackResult, MonitorConfig, RetryConfig,
    SelectionCriteria, SelectionResult,
};
use crate::domain::{
    BackendCapabilities, Platform, SessionConfig, SessionId, TransportSession, TransportStats,
    TransportType,
};
use crate::error::{Error, Result};
use crate::ports::TransportBackend;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Transport manager configuration
#[derive(Debug, Clone)]
pub struct TransportManagerConfig {
    /// Preferred transport type (None = auto-select)
    pub preferred_transport: Option<TransportType>,
    /// Enable automatic backend selection
    pub auto_select_backend: bool,
    /// Maximum concurrent sessions
    pub max_concurrent_sessions: usize,
    /// Enable performance monitoring
    pub enable_monitoring: bool,
}

impl Default for TransportManagerConfig {
    fn default() -> Self {
        Self {
            preferred_transport: None,
            auto_select_backend: true,
            max_concurrent_sessions: 100,
            enable_monitoring: true,
        }
    }
}

/// Transport manager state
struct TransportManagerState {
    /// Active sessions
    sessions: HashMap<SessionId, TransportSession>,
    /// Current backend
    backend: Option<Box<dyn TransportBackend>>,
    /// Platform information
    platform: Platform,
    /// Configuration
    config: TransportManagerConfig,
    /// Backend monitor
    monitor: Arc<BackendMonitor>,
    /// Current backend type
    current_backend_type: Option<TransportType>,
}

/// Transport manager
///
/// Main application service for managing transport operations
pub struct TransportManager {
    state: Arc<RwLock<TransportManagerState>>,
}

impl TransportManager {
    /// Create a new transport manager
    pub fn new(config: TransportManagerConfig) -> Self {
        let platform = Platform::detect();
        info!(
            "Transport Manager initialized on {} {} ({})",
            platform.os, platform.os_version, platform.arch
        );

        let monitor = Arc::new(BackendMonitor::new(MonitorConfig::default()));

        Self {
            state: Arc::new(RwLock::new(TransportManagerState {
                sessions: HashMap::new(),
                backend: None,
                platform,
                config,
                monitor,
                current_backend_type: None,
            })),
        }
    }

    /// Initialize the manager with a backend
    pub async fn initialize(&self, mut backend: Box<dyn TransportBackend>) -> Result<()> {
        let mut state = self.state.write().await;

        // Check if backend is available on current platform
        let caps = backend.capabilities();
        if !caps.is_available(&state.platform) {
            return Err(Error::Configuration(format!(
                "Backend {} is not available on this platform",
                caps.transport_type
            )));
        }

        // Initialize backend
        backend.initialize().await?;
        info!(
            "Initialized backend: {} (max throughput: {:.2} GB/s)",
            caps.transport_type,
            caps.max_throughput_gbps()
        );

        state.backend = Some(backend);
        Ok(())
    }

    /// Shutdown the manager
    pub async fn shutdown(&self) -> Result<()> {
        let mut state = self.state.write().await;

        // Stop all active sessions
        let session_ids: Vec<SessionId> = state.sessions.keys().copied().collect();
        for session_id in session_ids {
            if let Some(backend) = &mut state.backend {
                if let Err(e) = backend.stop_session(session_id).await {
                    warn!("Error stopping session {}: {}", session_id, e);
                }
            }
        }
        state.sessions.clear();

        // Shutdown backend
        if let Some(mut backend) = state.backend.take() {
            backend.shutdown().await?;
        }

        info!("Transport Manager shutdown complete");
        Ok(())
    }

    /// Create a new transport session
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        let mut state = self.state.write().await;

        // Check session limit
        if state.sessions.len() >= state.config.max_concurrent_sessions {
            return Err(Error::Configuration(
                "Maximum concurrent sessions reached".to_string(),
            ));
        }

        // Get backend
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        // Create session in backend
        let session_id = backend.create_session(config.clone()).await?;

        // Create domain session
        let session = TransportSession::new(config);
        state.sessions.insert(session_id, session);

        debug!("Created session: {}", session_id);
        Ok(session_id)
    }

    /// Start a session
    pub async fn start_session(&self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        // Start session in backend first
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        backend.start_session(session_id).await?;

        // Update domain session
        let session = state
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        session.start()?;

        info!("Started session: {}", session_id);
        Ok(())
    }

    /// Stop a session
    pub async fn stop_session(&self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        // Stop session in backend
        if let Some(backend) = &mut state.backend {
            backend.stop_session(session_id).await?;
        }

        // Remove domain session
        if let Some(mut session) = state.sessions.remove(&session_id) {
            session.complete()?;
            info!("Stopped session: {}", session_id);
        }

        Ok(())
    }

    /// Send data on a session
    pub async fn send(&self, session_id: SessionId, data: &[u8]) -> Result<usize> {
        let start = Instant::now();
        let mut state = self.state.write().await;

        // Get backend
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        // Send data
        let result = backend.send(session_id, data).await;
        let latency = start.elapsed();

        match result {
            Ok(bytes_sent) => {
                // Update session stats
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    session.record_send(bytes_sent as u64);
                }

                // Record metrics
                if let Some(backend_type) = state.current_backend_type {
                    state.monitor.record_send(backend_type, bytes_sent as u64, latency).await;
                }

                Ok(bytes_sent)
            }
            Err(e) => {
                // Record failure
                if let Some(backend_type) = state.current_backend_type {
                    state.monitor.record_failure(backend_type).await;
                }
                Err(e)
            }
        }
    }

    /// Receive data from a session
    pub async fn receive(&self, session_id: SessionId, buffer: &mut [u8]) -> Result<usize> {
        let start = Instant::now();
        let mut state = self.state.write().await;

        // Get backend
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        // Receive data
        let result = backend.receive(session_id, buffer).await;
        let latency = start.elapsed();

        match result {
            Ok(bytes_received) => {
                // Update session stats
                if let Some(session) = state.sessions.get_mut(&session_id) {
                    session.record_receive(bytes_received as u64);
                }

                // Record metrics
                if let Some(backend_type) = state.current_backend_type {
                    state.monitor.record_receive(backend_type, bytes_received as u64, latency).await;
                }

                Ok(bytes_received)
            }
            Err(e) => {
                // Record failure
                if let Some(backend_type) = state.current_backend_type {
                    state.monitor.record_failure(backend_type).await;
                }
                Err(e)
            }
        }
    }

    /// Get session statistics
    pub async fn get_session_stats(&self, session_id: SessionId) -> Result<TransportStats> {
        let state = self.state.read().await;

        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;

        Ok(session.stats().clone())
    }

    /// Get session information
    pub async fn get_session(&self, session_id: SessionId) -> Result<TransportSession> {
        let state = self.state.read().await;

        state
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or_else(|| Error::Domain("Session not found".to_string()))
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<SessionId>> {
        let state = self.state.read().await;
        Ok(state.sessions.keys().copied().collect())
    }

    /// Get backend capabilities
    pub async fn get_capabilities(&self) -> Result<BackendCapabilities> {
        let state = self.state.read().await;

        let backend = state
            .backend
            .as_ref()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        Ok(backend.capabilities())
    }

    /// Get platform information
    pub async fn get_platform(&self) -> Platform {
        let state = self.state.read().await;
        state.platform.clone()
    }

    /// Pause a session
    pub async fn pause_session(&self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        // Pause in backend
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        backend.pause_session(session_id).await?;

        // Update domain session
        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.pause()?;
        }

        debug!("Paused session: {}", session_id);
        Ok(())
    }

    /// Resume a session
    pub async fn resume_session(&self, session_id: SessionId) -> Result<()> {
        let mut state = self.state.write().await;

        // Resume in backend
        let backend = state
            .backend
            .as_mut()
            .ok_or_else(|| Error::Configuration("Backend not initialized".to_string()))?;

        backend.resume_session(session_id).await?;

        // Update domain session
        if let Some(session) = state.sessions.get_mut(&session_id) {
            session.resume()?;
        }

        debug!("Resumed session: {}", session_id);
        Ok(())
    }

    /// Get session state
    pub async fn get_session_state(&self, session_id: SessionId) -> Result<crate::domain::SessionState> {
        let state = self.state.read().await;
        
        let session = state
            .sessions
            .get(&session_id)
            .ok_or_else(|| Error::Domain("Session not found".to_string()))?;
        
        Ok(session.state())
    }

    /// Get aggregate transport statistics
    pub async fn get_transport_stats(&self) -> Result<TransportStats> {
        let state = self.state.read().await;
        
        // Aggregate stats from all sessions
        let mut total_stats = TransportStats::new();
        for session in state.sessions.values() {
            let session_stats = session.stats();
            total_stats.bytes_sent += session_stats.bytes_sent;
            total_stats.bytes_received += session_stats.bytes_received;
            total_stats.packets_sent += session_stats.packets_sent;
            total_stats.packets_received += session_stats.packets_received;
            total_stats.errors += session_stats.errors;
            total_stats.packets_lost += session_stats.packets_lost;
        }
        
        Ok(total_stats)
    }

    /// Check manager health
    pub async fn health_check(&self) -> Result<bool> {
        let state = self.state.read().await;

        if let Some(backend) = &state.backend {
            backend.health_check().await
        } else {
            Ok(false)
        }
    }

    /// Get active session count
    pub async fn active_session_count(&self) -> usize {
        let state = self.state.read().await;
        state.sessions.len()
    }

    /// Select optimal backend based on criteria
    pub fn select_optimal_backend(&self, criteria: &SelectionCriteria) -> Result<SelectionResult> {
        let selector = BackendSelector::new();
        selector.select_optimal(criteria)
    }

    /// Auto-select and initialize backend
    pub async fn auto_initialize(&self, criteria: Option<SelectionCriteria>) -> Result<SelectionResult> {
        let criteria = criteria.unwrap_or_default();
        let selection = self.select_optimal_backend(&criteria)?;
        
        info!(
            "Auto-selected backend: {} - {}",
            selection.transport_type, selection.reason
        );
        
        // Note: Backend initialization requires a concrete backend instance
        // This will be implemented when we integrate with the adapters
        
        Ok(selection)
    }

    /// List all available backends on current platform
    pub fn list_available_backends(&self) -> Vec<(TransportType, BackendCapabilities)> {
        let selector = BackendSelector::new();
        selector.list_available_backends()
    }

    /// Initialize with automatic fallback
    ///
    /// Tries to initialize the optimal backend, automatically falling back
    /// to alternative transports if the primary fails.
    pub async fn initialize_with_fallback(
        &self,
        criteria: Option<SelectionCriteria>,
        retry_config: Option<RetryConfig>,
    ) -> Result<FallbackResult> {
        use crate::application::FallbackManager;
        
        let fallback_manager = FallbackManager::new(retry_config.unwrap_or_default());
        let criteria = criteria.unwrap_or_default();

        // Create initialization function
        let state = self.state.clone();
        let init_fn = move |transport_type: TransportType| {
            let state = state.clone();
            async move {
                // Note: This is a placeholder. Actual backend initialization
                // will be implemented when we integrate with concrete adapters.
                info!("Initializing backend: {}", transport_type);
                
                // For now, just validate that the backend is available
                let platform = {
                    let s = state.read().await;
                    s.platform.clone()
                };
                
                let caps = BackendCapabilities::for_transport(transport_type);
                
                if !caps.is_available(&platform) {
                    return Err(Error::Configuration(format!(
                        "Backend {} not available on this platform",
                        transport_type
                    )));
                }
                
                Ok(())
            }
        };

        fallback_manager.try_with_fallback(&criteria, init_fn).await
    }

    /// Get backend monitor
    pub async fn monitor(&self) -> Arc<BackendMonitor> {
        let state = self.state.read().await;
        state.monitor.clone()
    }
    /// Get current backend type
    pub async fn current_backend_type(&self) -> Option<TransportType> {
        let state = self.state.read().await;
        state.current_backend_type
    }


    /// Get current backend metrics
    pub async fn get_backend_metrics(&self) -> Option<crate::application::BackendMetrics> {
        let state = self.state.read().await;
        if let Some(backend_type) = state.current_backend_type {
            state.monitor.get_metrics(backend_type).await
        } else {
            None
        }
    }

    /// Perform health check on current backend
    pub async fn health_check_backend(&self) -> Result<crate::application::HealthCheckResult> {
        let state = self.state.read().await;
        let backend_type = state.current_backend_type.ok_or_else(|| {
            Error::Configuration("No backend initialized".to_string())
        })?;
        
        state.monitor.health_check(backend_type).await
    }

    /// Get monitoring summary
    pub async fn get_monitor_summary(&self) -> crate::application::MonitorSummary {
        let state = self.state.read().await;
        state.monitor.get_summary().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TransportType;

    // Note: Full integration tests require actual backend implementations
    // These are basic unit tests for the manager structure

    #[tokio::test]
    async fn test_manager_creation() {
        let config = TransportManagerConfig::default();
        let manager = TransportManager::new(config);

        let platform = manager.get_platform().await;
        assert!(!platform.os.is_empty());
    }

    #[tokio::test]
    async fn test_manager_without_backend() {
        let config = TransportManagerConfig::default();
        let manager = TransportManager::new(config);

        // Should fail without backend
        let session_config = SessionConfig::default();
        let result = manager.create_session(session_config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_count() {
        let config = TransportManagerConfig::default();
        let manager = TransportManager::new(config);

        assert_eq!(manager.active_session_count().await, 0);
    }
}

// Made with Bob
