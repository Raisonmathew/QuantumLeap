//! Transport Type - Value Object
//!
//! Represents the type of transport backend to use

use serde::{Deserialize, Serialize};

/// Transport backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    /// io_uring - Linux kernel bypass (8-25 GB/s)
    IoUring,
    /// DPDK - Data Plane Development Kit (10 GB/s)
    Dpdk,
    /// QUIC - Modern UDP-based protocol (1 GB/s)
    Quic,
    /// TCP - Standard reliable transport (120 MB/s)
    Tcp,
}

impl TransportType {
    /// Check if this backend is available on the current system
    pub fn is_available(&self) -> bool {
        match self {
            Self::IoUring => Self::check_io_uring_available(),
            Self::Dpdk => Self::check_dpdk_available(),
            Self::Quic => true, // Always available
            Self::Tcp => true,  // Always available
        }
    }

    /// Get maximum theoretical throughput in bytes/second
    pub fn max_throughput(&self) -> u64 {
        match self {
            Self::IoUring => 8_000_000_000,  // 8 GB/s (can go up to 25 GB/s with 25GbE)
            Self::Dpdk => 10_000_000_000,    // 10 GB/s
            Self::Quic => 1_000_000_000,     // 1 GB/s
            Self::Tcp => 120_000_000,        // 120 MB/s
        }
    }

    /// Get priority for backend selection (higher = preferred)
    pub fn priority(&self) -> u8 {
        match self {
            Self::IoUring => 90,  // High priority (standard hardware)
            Self::Dpdk => 100,    // Highest priority (if available)
            Self::Quic => 70,     // Medium-high priority
            Self::Tcp => 50,      // Medium priority (fallback)
        }
    }

    /// Check if backend requires special hardware
    pub fn requires_special_hardware(&self) -> bool {
        matches!(self, Self::Dpdk)
    }

    /// Check if backend supports zero-copy
    pub fn supports_zero_copy(&self) -> bool {
        matches!(self, Self::IoUring | Self::Dpdk)
    }

    /// Check if backend supports parallel streams
    pub fn supports_parallel_streams(&self) -> bool {
        matches!(self, Self::Quic)
    }

    /// Get all available backends on current system
    pub fn available_backends() -> Vec<Self> {
        vec![Self::IoUring, Self::Dpdk, Self::Quic, Self::Tcp]
            .into_iter()
            .filter(|t| t.is_available())
            .collect()
    }

    /// Check if io_uring is available
    #[cfg(target_os = "linux")]
    fn check_io_uring_available() -> bool {
        // Check if io_uring is available
        std::path::Path::new("/proc/sys/kernel/io_uring_disabled").exists()
            && Self::get_kernel_version()
                .map(|(major, minor, _)| major >= 5 && minor >= 1)
                .unwrap_or(false)
    }

    #[cfg(not(target_os = "linux"))]
    fn check_io_uring_available() -> bool {
        false
    }

    /// Check if DPDK is available
    fn check_dpdk_available() -> bool {
        // Check for DPDK installation
        std::env::var("RTE_SDK").is_ok()
            && std::path::Path::new("/dev/uio0").exists()
    }

    /// Get Linux kernel version
    #[cfg(target_os = "linux")]
    #[allow(dead_code)] // Used in check_io_uring_available on Linux only
    fn get_kernel_version() -> Option<(u32, u32, u32)> {
        use std::fs;
        
        let version_str = fs::read_to_string("/proc/version").ok()?;
        let parts: Vec<&str> = version_str.split_whitespace().collect();
        
        if parts.len() < 3 {
            return None;
        }
        
        let version_parts: Vec<&str> = parts[2].split('.').collect();
        if version_parts.len() < 3 {
            return None;
        }
        
        Some((
            version_parts[0].parse().ok()?,
            version_parts[1].parse().ok()?,
            version_parts[2].parse().ok()?,
        ))
    }

    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    fn get_kernel_version() -> Option<(u32, u32, u32)> {
        None
    }
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoUring => write!(f, "io_uring"),
            Self::Dpdk => write!(f, "DPDK"),
            Self::Quic => write!(f, "QUIC"),
            Self::Tcp => write!(f, "TCP"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_throughput() {
        assert_eq!(TransportType::IoUring.max_throughput(), 8_000_000_000);
        assert_eq!(TransportType::Dpdk.max_throughput(), 10_000_000_000);
        assert_eq!(TransportType::Quic.max_throughput(), 1_000_000_000);
        assert_eq!(TransportType::Tcp.max_throughput(), 120_000_000);
    }

    #[test]
    fn test_transport_type_priority() {
        assert!(TransportType::Dpdk.priority() > TransportType::IoUring.priority());
        assert!(TransportType::IoUring.priority() > TransportType::Quic.priority());
        assert!(TransportType::Quic.priority() > TransportType::Tcp.priority());
    }

    #[test]
    fn test_transport_type_availability() {
        // TCP and QUIC should always be available
        assert!(TransportType::Tcp.is_available());
        assert!(TransportType::Quic.is_available());
    }

    #[test]
    fn test_special_hardware_requirement() {
        assert!(TransportType::Dpdk.requires_special_hardware());
        assert!(!TransportType::IoUring.requires_special_hardware());
        assert!(!TransportType::Quic.requires_special_hardware());
        assert!(!TransportType::Tcp.requires_special_hardware());
    }

    #[test]
    fn test_zero_copy_support() {
        assert!(TransportType::IoUring.supports_zero_copy());
        assert!(TransportType::Dpdk.supports_zero_copy());
        assert!(!TransportType::Quic.supports_zero_copy());
        assert!(!TransportType::Tcp.supports_zero_copy());
    }
}

// Made with Bob
