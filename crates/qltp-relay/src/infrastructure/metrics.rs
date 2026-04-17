//! Metrics Module
//!
//! Provides metrics collection and monitoring for relay services

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Server metrics
#[derive(Debug)]
pub struct ServerMetrics {
    // Request counters
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    rate_limited_requests: AtomicU64,
    
    // Connection metrics
    active_connections: AtomicUsize,
    total_connections: AtomicU64,
    
    // Bandwidth metrics
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    
    // Timing metrics
    start_time: Instant,
    last_request_time: Arc<RwLock<Option<Instant>>>,
}

impl ServerMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            rate_limited_requests: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            total_connections: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            start_time: Instant::now(),
            last_request_time: Arc::new(RwLock::new(None)),
        }
    }

    // Request metrics
    
    /// Increment total requests
    pub fn increment_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment successful requests
    pub fn increment_successful(&self) {
        self.successful_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment failed requests
    pub fn increment_failed(&self) {
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment rate limited requests
    pub fn increment_rate_limited(&self) {
        self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total requests
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get successful requests
    pub fn successful_requests(&self) -> u64 {
        self.successful_requests.load(Ordering::Relaxed)
    }

    /// Get failed requests
    pub fn failed_requests(&self) -> u64 {
        self.failed_requests.load(Ordering::Relaxed)
    }

    /// Get rate limited requests
    pub fn rate_limited_requests(&self) -> u64 {
        self.rate_limited_requests.load(Ordering::Relaxed)
    }

    /// Get success rate (0.0 - 1.0)
    pub fn success_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            self.successful_requests() as f64 / total as f64
        }
    }

    // Connection metrics
    
    /// Increment active connections
    pub fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections
    pub fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get active connections
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get total connections
    pub fn total_connections(&self) -> u64 {
        self.total_connections.load(Ordering::Relaxed)
    }

    // Bandwidth metrics
    
    /// Record bytes sent
    pub fn record_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record bytes received
    pub fn record_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Get total bytes sent
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    /// Get total bytes received
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get total bandwidth (sent + received)
    pub fn total_bandwidth(&self) -> u64 {
        self.bytes_sent() + self.bytes_received()
    }

    // Timing metrics
    
    /// Update last request time
    pub async fn update_last_request_time(&self) {
        let mut last_time = self.last_request_time.write().await;
        *last_time = Some(Instant::now());
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get time since last request
    pub async fn time_since_last_request(&self) -> Option<Duration> {
        let last_time = self.last_request_time.read().await;
        last_time.map(|t| t.elapsed())
    }

    /// Get requests per second
    pub fn requests_per_second(&self) -> f64 {
        let uptime_secs = self.uptime().as_secs_f64();
        if uptime_secs == 0.0 {
            0.0
        } else {
            self.total_requests() as f64 / uptime_secs
        }
    }

    /// Get bandwidth per second (bytes/sec)
    pub fn bandwidth_per_second(&self) -> f64 {
        let uptime_secs = self.uptime().as_secs_f64();
        if uptime_secs == 0.0 {
            0.0
        } else {
            self.total_bandwidth() as f64 / uptime_secs
        }
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.successful_requests.store(0, Ordering::Relaxed);
        self.failed_requests.store(0, Ordering::Relaxed);
        self.rate_limited_requests.store(0, Ordering::Relaxed);
        self.total_connections.store(0, Ordering::Relaxed);
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        
        let mut last_time = self.last_request_time.write().await;
        *last_time = None;
    }

    /// Get metrics snapshot
    pub async fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_requests: self.total_requests(),
            successful_requests: self.successful_requests(),
            failed_requests: self.failed_requests(),
            rate_limited_requests: self.rate_limited_requests(),
            success_rate: self.success_rate(),
            active_connections: self.active_connections(),
            total_connections: self.total_connections(),
            bytes_sent: self.bytes_sent(),
            bytes_received: self.bytes_received(),
            total_bandwidth: self.total_bandwidth(),
            uptime: self.uptime(),
            requests_per_second: self.requests_per_second(),
            bandwidth_per_second: self.bandwidth_per_second(),
        }
    }
}

impl Default for ServerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics snapshot for reporting
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub success_rate: f64,
    pub active_connections: usize,
    pub total_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub total_bandwidth: u64,
    pub uptime: Duration,
    pub requests_per_second: f64,
    pub bandwidth_per_second: f64,
}

impl MetricsSnapshot {
    /// Format as human-readable string
    pub fn format(&self) -> String {
        format!(
            "Metrics Snapshot:\n\
             Requests: {} total, {} successful, {} failed, {} rate-limited\n\
             Success Rate: {:.2}%\n\
             Connections: {} active, {} total\n\
             Bandwidth: {} sent, {} received, {} total\n\
             Uptime: {:.2}s\n\
             Rate: {:.2} req/s, {:.2} bytes/s",
            self.total_requests,
            self.successful_requests,
            self.failed_requests,
            self.rate_limited_requests,
            self.success_rate * 100.0,
            self.active_connections,
            self.total_connections,
            self.bytes_sent,
            self.bytes_received,
            self.total_bandwidth,
            self.uptime.as_secs_f64(),
            self.requests_per_second,
            self.bandwidth_per_second,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_metrics() {
        let metrics = ServerMetrics::new();
        
        metrics.increment_requests();
        metrics.increment_successful();
        assert_eq!(metrics.total_requests(), 1);
        assert_eq!(metrics.successful_requests(), 1);
        assert_eq!(metrics.success_rate(), 1.0);
        
        metrics.increment_requests();
        metrics.increment_failed();
        assert_eq!(metrics.total_requests(), 2);
        assert_eq!(metrics.failed_requests(), 1);
        assert_eq!(metrics.success_rate(), 0.5);
    }

    #[tokio::test]
    async fn test_connection_metrics() {
        let metrics = ServerMetrics::new();
        
        metrics.connection_opened();
        assert_eq!(metrics.active_connections(), 1);
        assert_eq!(metrics.total_connections(), 1);
        
        metrics.connection_opened();
        assert_eq!(metrics.active_connections(), 2);
        assert_eq!(metrics.total_connections(), 2);
        
        metrics.connection_closed();
        assert_eq!(metrics.active_connections(), 1);
        assert_eq!(metrics.total_connections(), 2);
    }

    #[tokio::test]
    async fn test_bandwidth_metrics() {
        let metrics = ServerMetrics::new();
        
        metrics.record_bytes_sent(1000);
        metrics.record_bytes_received(500);
        
        assert_eq!(metrics.bytes_sent(), 1000);
        assert_eq!(metrics.bytes_received(), 500);
        assert_eq!(metrics.total_bandwidth(), 1500);
    }

    #[tokio::test]
    async fn test_rate_limited() {
        let metrics = ServerMetrics::new();
        
        metrics.increment_requests();
        metrics.increment_rate_limited();
        
        assert_eq!(metrics.rate_limited_requests(), 1);
    }

    #[tokio::test]
    async fn test_snapshot() {
        let metrics = ServerMetrics::new();
        
        metrics.increment_requests();
        metrics.increment_successful();
        metrics.connection_opened();
        metrics.record_bytes_sent(1000);
        
        let snapshot = metrics.snapshot().await;
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.successful_requests, 1);
        assert_eq!(snapshot.active_connections, 1);
        assert_eq!(snapshot.bytes_sent, 1000);
    }

    #[tokio::test]
    async fn test_reset() {
        let metrics = ServerMetrics::new();
        
        metrics.increment_requests();
        metrics.increment_successful();
        metrics.connection_opened();
        
        metrics.reset().await;
        
        assert_eq!(metrics.total_requests(), 0);
        assert_eq!(metrics.successful_requests(), 0);
        assert_eq!(metrics.total_connections(), 0);
    }
}

// Made with Bob