//! Backend Monitor - Runtime monitoring and health checks
//!
//! Tracks backend performance metrics, health status, and provides
//! real-time monitoring capabilities for transport backends.

use crate::domain::TransportType;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Backend health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Backend is healthy and operational
    Healthy,
    /// Backend is degraded but functional
    Degraded,
    /// Backend is unhealthy and should not be used
    Unhealthy,
    /// Backend status is unknown (not yet checked)
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "Healthy"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Unhealthy => write!(f, "Unhealthy"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Performance metrics for a backend
#[derive(Debug, Clone)]
pub struct BackendMetrics {
    /// Transport type
    pub transport_type: TransportType,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Average latency in microseconds
    pub avg_latency_us: u64,
    /// Peak latency in microseconds
    pub peak_latency_us: u64,
    /// Current throughput in bytes/second
    pub current_throughput_bps: u64,
    /// Peak throughput in bytes/second
    pub peak_throughput_bps: u64,
    /// Number of active connections
    pub active_connections: usize,
    /// Last update timestamp
    pub last_updated: Instant,
}

impl BackendMetrics {
    /// Create new metrics for a backend
    pub fn new(transport_type: TransportType) -> Self {
        Self {
            transport_type,
            bytes_sent: 0,
            bytes_received: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_us: 0,
            peak_latency_us: 0,
            current_throughput_bps: 0,
            peak_throughput_bps: 0,
            active_connections: 0,
            last_updated: Instant::now(),
        }
    }

    /// Record a successful send operation
    pub fn record_send(&mut self, bytes: u64, latency: Duration) {
        self.bytes_sent += bytes;
        self.successful_operations += 1;
        self.update_latency(latency);
        self.update_throughput(bytes, latency);
        self.last_updated = Instant::now();
    }

    /// Record a successful receive operation
    pub fn record_receive(&mut self, bytes: u64, latency: Duration) {
        self.bytes_received += bytes;
        self.successful_operations += 1;
        self.update_latency(latency);
        self.update_throughput(bytes, latency);
        self.last_updated = Instant::now();
    }

    /// Record a failed operation
    pub fn record_failure(&mut self) {
        self.failed_operations += 1;
        self.last_updated = Instant::now();
    }

    /// Update latency statistics
    fn update_latency(&mut self, latency: Duration) {
        let latency_us = latency.as_micros() as u64;
        
        // Update average (simple moving average)
        if self.successful_operations == 1 {
            self.avg_latency_us = latency_us;
        } else {
            self.avg_latency_us = (self.avg_latency_us * 9 + latency_us) / 10;
        }
        
        // Update peak
        if latency_us > self.peak_latency_us {
            self.peak_latency_us = latency_us;
        }
    }

    /// Update throughput statistics
    fn update_throughput(&mut self, bytes: u64, duration: Duration) {
        if duration.as_secs_f64() > 0.0 {
            let throughput = (bytes as f64 / duration.as_secs_f64()) as u64;
            self.current_throughput_bps = throughput;
            
            if throughput > self.peak_throughput_bps {
                self.peak_throughput_bps = throughput;
            }
        }
    }

    /// Get success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.successful_operations + self.failed_operations;
        if total == 0 {
            1.0
        } else {
            self.successful_operations as f64 / total as f64
        }
    }

    /// Get total operations
    pub fn total_operations(&self) -> u64 {
        self.successful_operations + self.failed_operations
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Backend transport type
    pub transport_type: TransportType,
    /// Health status
    pub status: HealthStatus,
    /// Check timestamp
    pub checked_at: Instant,
    /// Response time in microseconds
    pub response_time_us: u64,
    /// Error message if unhealthy
    pub error: Option<String>,
}

/// Backend monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Enable automatic health checks
    pub enable_health_checks: bool,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Unhealthy threshold (failure rate)
    pub unhealthy_threshold: f64,
    /// Degraded threshold (failure rate)
    pub degraded_threshold: f64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            enable_health_checks: true,
            health_check_interval: Duration::from_secs(30),
            metrics_interval: Duration::from_secs(5),
            unhealthy_threshold: 0.5, // 50% failure rate
            degraded_threshold: 0.2,  // 20% failure rate
        }
    }
}

/// Backend monitor state
struct MonitorState {
    /// Metrics per backend
    metrics: HashMap<TransportType, BackendMetrics>,
    /// Health status per backend
    health: HashMap<TransportType, HealthCheckResult>,
    /// Configuration
    config: MonitorConfig,
}

/// Backend monitor
///
/// Monitors backend performance and health in real-time
pub struct BackendMonitor {
    state: Arc<RwLock<MonitorState>>,
}

impl BackendMonitor {
    /// Create a new backend monitor
    pub fn new(config: MonitorConfig) -> Self {
        info!("Backend Monitor initialized");
        Self {
            state: Arc::new(RwLock::new(MonitorState {
                metrics: HashMap::new(),
                health: HashMap::new(),
                config,
            })),
        }
    }

    /// Register a backend for monitoring
    pub async fn register_backend(&self, transport_type: TransportType) {
        let mut state = self.state.write().await;
        
        if !state.metrics.contains_key(&transport_type) {
            state.metrics.insert(transport_type, BackendMetrics::new(transport_type));
            state.health.insert(
                transport_type,
                HealthCheckResult {
                    transport_type,
                    status: HealthStatus::Unknown,
                    checked_at: Instant::now(),
                    response_time_us: 0,
                    error: None,
                },
            );
            info!("Registered backend for monitoring: {}", transport_type);
        }
    }

    /// Record a send operation
    pub async fn record_send(&self, transport_type: TransportType, bytes: u64, latency: Duration) {
        let mut state = self.state.write().await;
        
        if let Some(metrics) = state.metrics.get_mut(&transport_type) {
            metrics.record_send(bytes, latency);
        }
    }

    /// Record a receive operation
    pub async fn record_receive(&self, transport_type: TransportType, bytes: u64, latency: Duration) {
        let mut state = self.state.write().await;
        
        if let Some(metrics) = state.metrics.get_mut(&transport_type) {
            metrics.record_receive(bytes, latency);
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self, transport_type: TransportType) {
        let mut state = self.state.write().await;
        
        if let Some(metrics) = state.metrics.get_mut(&transport_type) {
            metrics.record_failure();
        }
    }

    /// Get metrics for a backend
    pub async fn get_metrics(&self, transport_type: TransportType) -> Option<BackendMetrics> {
        let state = self.state.read().await;
        state.metrics.get(&transport_type).cloned()
    }

    /// Get all metrics
    pub async fn get_all_metrics(&self) -> HashMap<TransportType, BackendMetrics> {
        let state = self.state.read().await;
        state.metrics.clone()
    }

    /// Perform health check on a backend
    pub async fn health_check(&self, transport_type: TransportType) -> Result<HealthCheckResult> {
        let start = Instant::now();
        
        // Get current metrics
        let metrics = {
            let state = self.state.read().await;
            state.metrics.get(&transport_type).cloned()
        };

        let result = if let Some(metrics) = metrics {
            let success_rate = metrics.success_rate();
            let config = {
                let state = self.state.read().await;
                state.config.clone()
            };

            let status = if success_rate < (1.0 - config.unhealthy_threshold) {
                HealthStatus::Unhealthy
            } else if success_rate < (1.0 - config.degraded_threshold) {
                HealthStatus::Degraded
            } else {
                HealthStatus::Healthy
            };

            let error = if status != HealthStatus::Healthy {
                Some(format!(
                    "Success rate: {:.1}%, Failed: {}/{}",
                    success_rate * 100.0,
                    metrics.failed_operations,
                    metrics.total_operations()
                ))
            } else {
                None
            };

            HealthCheckResult {
                transport_type,
                status,
                checked_at: Instant::now(),
                response_time_us: start.elapsed().as_micros() as u64,
                error,
            }
        } else {
            HealthCheckResult {
                transport_type,
                status: HealthStatus::Unknown,
                checked_at: Instant::now(),
                response_time_us: start.elapsed().as_micros() as u64,
                error: Some("Backend not registered".to_string()),
            }
        };

        // Update health status
        {
            let mut state = self.state.write().await;
            state.health.insert(transport_type, result.clone());
        }

        debug!(
            "Health check for {}: {} ({}µs)",
            transport_type, result.status, result.response_time_us
        );

        Ok(result)
    }

    /// Get health status for a backend
    pub async fn get_health(&self, transport_type: TransportType) -> Option<HealthCheckResult> {
        let state = self.state.read().await;
        state.health.get(&transport_type).cloned()
    }

    /// Get all health statuses
    pub async fn get_all_health(&self) -> HashMap<TransportType, HealthCheckResult> {
        let state = self.state.read().await;
        state.health.clone()
    }

    /// Check if backend is healthy
    pub async fn is_healthy(&self, transport_type: TransportType) -> bool {
        let state = self.state.read().await;
        state
            .health
            .get(&transport_type)
            .map(|h| h.status == HealthStatus::Healthy)
            .unwrap_or(false)
    }

    /// Get monitor configuration
    pub async fn config(&self) -> MonitorConfig {
        let state = self.state.read().await;
        state.config.clone()
    }

    /// Update monitor configuration
    pub async fn set_config(&self, config: MonitorConfig) {
        let mut state = self.state.write().await;
        state.config = config;
        info!("Monitor configuration updated");
    }

    /// Reset metrics for a backend
    pub async fn reset_metrics(&self, transport_type: TransportType) {
        let mut state = self.state.write().await;
        if state.metrics.contains_key(&transport_type) {
            state.metrics.insert(transport_type, BackendMetrics::new(transport_type));
            info!("Reset metrics for backend: {}", transport_type);
        }
    }

    /// Get summary statistics
    pub async fn get_summary(&self) -> MonitorSummary {
        let state = self.state.read().await;
        
        let total_bytes_sent: u64 = state.metrics.values().map(|m| m.bytes_sent).sum();
        let total_bytes_received: u64 = state.metrics.values().map(|m| m.bytes_received).sum();
        let total_operations: u64 = state.metrics.values().map(|m| m.total_operations()).sum();
        
        let healthy_backends = state
            .health
            .values()
            .filter(|h| h.status == HealthStatus::Healthy)
            .count();
        
        let degraded_backends = state
            .health
            .values()
            .filter(|h| h.status == HealthStatus::Degraded)
            .count();
        
        let unhealthy_backends = state
            .health
            .values()
            .filter(|h| h.status == HealthStatus::Unhealthy)
            .count();

        MonitorSummary {
            total_backends: state.metrics.len(),
            healthy_backends,
            degraded_backends,
            unhealthy_backends,
            total_bytes_sent,
            total_bytes_received,
            total_operations,
        }
    }
}

impl Default for BackendMonitor {
    fn default() -> Self {
        Self::new(MonitorConfig::default())
    }
}

/// Monitor summary statistics
#[derive(Debug, Clone)]
pub struct MonitorSummary {
    /// Total number of monitored backends
    pub total_backends: usize,
    /// Number of healthy backends
    pub healthy_backends: usize,
    /// Number of degraded backends
    pub degraded_backends: usize,
    /// Number of unhealthy backends
    pub unhealthy_backends: usize,
    /// Total bytes sent across all backends
    pub total_bytes_sent: u64,
    /// Total bytes received across all backends
    pub total_bytes_received: u64,
    /// Total operations across all backends
    pub total_operations: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "Healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "Degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "Unhealthy");
        assert_eq!(HealthStatus::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_backend_metrics_new() {
        let metrics = BackendMetrics::new(TransportType::Tcp);
        assert_eq!(metrics.transport_type, TransportType::Tcp);
        assert_eq!(metrics.bytes_sent, 0);
        assert_eq!(metrics.bytes_received, 0);
        assert_eq!(metrics.successful_operations, 0);
        assert_eq!(metrics.failed_operations, 0);
    }

    #[test]
    fn test_metrics_record_send() {
        let mut metrics = BackendMetrics::new(TransportType::Tcp);
        metrics.record_send(1000, Duration::from_millis(10));
        
        assert_eq!(metrics.bytes_sent, 1000);
        assert_eq!(metrics.successful_operations, 1);
        assert!(metrics.avg_latency_us > 0);
    }

    #[test]
    fn test_metrics_success_rate() {
        let mut metrics = BackendMetrics::new(TransportType::Tcp);
        
        // 100% success rate initially
        assert_eq!(metrics.success_rate(), 1.0);
        
        // Add some operations
        metrics.record_send(100, Duration::from_millis(1));
        metrics.record_send(100, Duration::from_millis(1));
        metrics.record_failure();
        
        // 2 success, 1 failure = 66.67%
        assert!((metrics.success_rate() - 0.6667).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = BackendMonitor::new(config);
        
        let summary = monitor.get_summary().await;
        assert_eq!(summary.total_backends, 0);
    }

    #[tokio::test]
    async fn test_register_backend() {
        let monitor = BackendMonitor::default();
        monitor.register_backend(TransportType::Tcp).await;
        
        let summary = monitor.get_summary().await;
        assert_eq!(summary.total_backends, 1);
    }

    #[tokio::test]
    async fn test_record_operations() {
        let monitor = BackendMonitor::default();
        monitor.register_backend(TransportType::Tcp).await;
        
        monitor.record_send(TransportType::Tcp, 1000, Duration::from_millis(10)).await;
        monitor.record_receive(TransportType::Tcp, 500, Duration::from_millis(5)).await;
        
        let metrics = monitor.get_metrics(TransportType::Tcp).await.unwrap();
        assert_eq!(metrics.bytes_sent, 1000);
        assert_eq!(metrics.bytes_received, 500);
        assert_eq!(metrics.successful_operations, 2);
    }

    #[tokio::test]
    async fn test_health_check() {
        let monitor = BackendMonitor::default();
        monitor.register_backend(TransportType::Tcp).await;
        
        // Add successful operations
        for _ in 0..10 {
            monitor.record_send(TransportType::Tcp, 100, Duration::from_millis(1)).await;
        }
        
        let result = monitor.health_check(TransportType::Tcp).await.unwrap();
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_unhealthy_detection() {
        let monitor = BackendMonitor::default();
        monitor.register_backend(TransportType::Tcp).await;
        
        // Add mostly failures
        for _ in 0..10 {
            monitor.record_failure(TransportType::Tcp).await;
        }
        monitor.record_send(TransportType::Tcp, 100, Duration::from_millis(1)).await;
        
        let result = monitor.health_check(TransportType::Tcp).await.unwrap();
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_reset_metrics() {
        let monitor = BackendMonitor::default();
        monitor.register_backend(TransportType::Tcp).await;
        
        monitor.record_send(TransportType::Tcp, 1000, Duration::from_millis(10)).await;
        monitor.reset_metrics(TransportType::Tcp).await;
        
        let metrics = monitor.get_metrics(TransportType::Tcp).await.unwrap();
        assert_eq!(metrics.bytes_sent, 0);
        assert_eq!(metrics.successful_operations, 0);
    }
}

// Made with Bob