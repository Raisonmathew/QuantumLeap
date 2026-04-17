//! Transport Stats - Value Object
//!
//! Represents statistics for a transport session

use serde::{Deserialize, Serialize};

/// Transport session statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportStats {
    /// Total bytes transferred (sent + received)
    pub bytes_transferred: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets received
    pub packets_received: u64,
    /// Total packets lost
    pub packets_lost: u64,
    /// Total errors encountered
    pub errors: u64,
    /// Current round-trip time in milliseconds
    pub rtt_ms: u64,
    /// Current throughput in bytes per second
    pub throughput_bps: u64,
    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: f32,
}

impl TransportStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful send operation
    pub fn record_send(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.bytes_transferred += bytes;
        self.packets_sent += 1;
    }

    /// Record a successful receive operation
    pub fn record_receive(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.bytes_transferred += bytes;
        self.packets_received += 1;
    }

    /// Record a packet loss
    pub fn record_packet_loss(&mut self) {
        self.packets_lost += 1;
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    /// Update round-trip time
    pub fn update_rtt(&mut self, rtt_ms: u64) {
        self.rtt_ms = rtt_ms;
    }

    /// Update throughput
    pub fn update_throughput(&mut self, throughput_bps: u64) {
        self.throughput_bps = throughput_bps;
    }

    /// Update CPU usage
    pub fn update_cpu_usage(&mut self, cpu_percent: f32) {
        self.cpu_usage_percent = cpu_percent.clamp(0.0, 100.0);
    }

    /// Calculate packet loss rate (0.0 to 1.0)
    pub fn packet_loss_rate(&self) -> f64 {
        let total_packets = self.packets_sent + self.packets_received;
        if total_packets == 0 {
            return 0.0;
        }
        self.packets_lost as f64 / total_packets as f64
    }

    /// Calculate error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        let total_operations = self.packets_sent + self.packets_received;
        if total_operations == 0 {
            return 0.0;
        }
        self.errors as f64 / total_operations as f64
    }

    /// Get throughput in MB/s
    pub fn throughput_mbps(&self) -> f64 {
        self.throughput_bps as f64 / 1_000_000.0
    }

    /// Get throughput in GB/s
    pub fn throughput_gbps(&self) -> f64 {
        self.throughput_bps as f64 / 1_000_000_000.0
    }

    /// Check if stats indicate healthy operation
    pub fn is_healthy(&self) -> bool {
        self.packet_loss_rate() < 0.01  // Less than 1% packet loss
            && self.error_rate() < 0.001  // Less than 0.1% error rate
    }

    /// Merge stats from another session
    pub fn merge(&mut self, other: &TransportStats) {
        self.bytes_transferred += other.bytes_transferred;
        self.bytes_sent += other.bytes_sent;
        self.bytes_received += other.bytes_received;
        self.packets_sent += other.packets_sent;
        self.packets_received += other.packets_received;
        self.packets_lost += other.packets_lost;
        self.errors += other.errors;
        
        // Take average for RTT and throughput
        if other.rtt_ms > 0 {
            self.rtt_ms = (self.rtt_ms + other.rtt_ms) / 2;
        }
        if other.throughput_bps > 0 {
            self.throughput_bps = (self.throughput_bps + other.throughput_bps) / 2;
        }
        if other.cpu_usage_percent > 0.0 {
            self.cpu_usage_percent = (self.cpu_usage_percent + other.cpu_usage_percent) / 2.0;
        }
    }
}

impl std::fmt::Display for TransportStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Stats {{ transferred: {} MB, sent: {}, recv: {}, loss: {:.2}%, errors: {}, throughput: {:.2} MB/s }}",
            self.bytes_transferred / 1_000_000,
            self.packets_sent,
            self.packets_received,
            self.packet_loss_rate() * 100.0,
            self.errors,
            self.throughput_mbps()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_operations() {
        let mut stats = TransportStats::new();
        
        stats.record_send(1000);
        assert_eq!(stats.bytes_sent, 1000);
        assert_eq!(stats.bytes_transferred, 1000);
        assert_eq!(stats.packets_sent, 1);
        
        stats.record_receive(2000);
        assert_eq!(stats.bytes_received, 2000);
        assert_eq!(stats.bytes_transferred, 3000);
        assert_eq!(stats.packets_received, 1);
    }

    #[test]
    fn test_packet_loss_rate() {
        let mut stats = TransportStats::new();
        stats.packets_sent = 100;
        stats.packets_received = 100;
        stats.packets_lost = 5;
        
        assert!((stats.packet_loss_rate() - 0.025).abs() < 0.001);
    }

    #[test]
    fn test_throughput_conversion() {
        let mut stats = TransportStats::new();
        stats.throughput_bps = 1_000_000_000; // 1 GB/s
        
        assert_eq!(stats.throughput_mbps(), 1000.0);
        assert_eq!(stats.throughput_gbps(), 1.0);
    }

    #[test]
    fn test_is_healthy() {
        let mut stats = TransportStats::new();
        stats.packets_sent = 1000;
        stats.packets_received = 1000;
        stats.packets_lost = 5; // 0.25% loss
        stats.errors = 1; // 0.05% error rate
        
        assert!(stats.is_healthy());
        
        stats.packets_lost = 50; // 2.5% loss
        assert!(!stats.is_healthy());
    }

    #[test]
    fn test_merge_stats() {
        let mut stats1 = TransportStats::new();
        stats1.bytes_sent = 1000;
        stats1.packets_sent = 10;
        stats1.throughput_bps = 1_000_000;
        
        let mut stats2 = TransportStats::new();
        stats2.bytes_sent = 2000;
        stats2.packets_sent = 20;
        stats2.throughput_bps = 2_000_000;
        
        stats1.merge(&stats2);
        
        assert_eq!(stats1.bytes_sent, 3000);
        assert_eq!(stats1.packets_sent, 30);
        assert_eq!(stats1.throughput_bps, 1_500_000); // Average
    }
}

// Made with Bob
