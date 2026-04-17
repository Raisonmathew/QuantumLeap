//! Backend Capabilities - Value Object
//!
//! Represents the capabilities and requirements of a transport backend

use crate::domain::transport_type::TransportType;
use serde::{Deserialize, Serialize};

/// Platform information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    /// Operating system (Linux, Windows, macOS)
    pub os: String,
    /// OS version
    pub os_version: String,
    /// Kernel version (for Linux)
    pub kernel_version: Option<String>,
    /// CPU architecture (x86_64, aarch64, etc.)
    pub arch: String,
    /// Number of CPU cores
    pub cpu_cores: usize,
    /// Total RAM in bytes
    pub total_ram_bytes: u64,
}

impl Platform {
    /// Detect current platform
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            os_version: Self::get_os_version(),
            kernel_version: Self::get_kernel_version(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_cores: num_cpus::get(),
            total_ram_bytes: Self::get_total_ram(),
        }
    }

    fn get_os_version() -> String {
        // Simplified - in production, use platform-specific APIs
        "unknown".to_string()
    }

    fn get_kernel_version() -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            std::fs::read_to_string("/proc/version")
                .ok()
                .and_then(|v| v.split_whitespace().nth(2).map(String::from))
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    fn get_total_ram() -> u64 {
        // Simplified - in production, use platform-specific APIs
        8_000_000_000 // 8GB default
    }

    /// Check if platform supports io_uring
    pub fn supports_io_uring(&self) -> bool {
        if self.os != "linux" {
            return false;
        }

        if let Some(ref kernel) = self.kernel_version {
            // io_uring requires Linux 5.1+
            if let Some(major) = kernel.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                return major >= 5;
            }
        }

        false
    }

    /// Check if platform supports DPDK
    pub fn supports_dpdk(&self) -> bool {
        // DPDK supports Linux, FreeBSD, and Windows
        matches!(self.os.as_str(), "linux" | "freebsd" | "windows")
    }

    /// Check if platform has sufficient resources for high-performance transfer
    pub fn has_sufficient_resources(&self) -> bool {
        self.cpu_cores >= 4 && self.total_ram_bytes >= 4_000_000_000 // 4GB
    }
}

/// Backend capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendCapabilities {
    /// Transport type
    pub transport_type: TransportType,
    /// Maximum throughput in bytes per second
    pub max_throughput_bps: u64,
    /// Minimum required network bandwidth in bytes per second
    pub min_bandwidth_bps: u64,
    /// Whether zero-copy is supported
    pub supports_zero_copy: bool,
    /// Whether kernel bypass is used
    pub uses_kernel_bypass: bool,
    /// Requires special hardware
    pub requires_special_hardware: bool,
    /// Minimum CPU cores required
    pub min_cpu_cores: usize,
    /// Minimum RAM required in bytes
    pub min_ram_bytes: u64,
    /// Supported platforms
    pub supported_platforms: Vec<String>,
    /// Hardware cost estimate in USD
    pub hardware_cost_usd: u32,
}

impl BackendCapabilities {
    /// Get capabilities for a transport type
    pub fn for_transport(transport_type: TransportType) -> Self {
        match transport_type {
            TransportType::IoUring => Self {
                transport_type,
                max_throughput_bps: 20_000_000_000, // 20 GB/s with enhanced async I/O + batch optimization
                min_bandwidth_bps: 1_250_000_000,   // 10 Gbps
                supports_zero_copy: true,
                uses_kernel_bypass: true,
                requires_special_hardware: false,
                min_cpu_cores: 4,
                min_ram_bytes: 4_000_000_000, // 4GB
                supported_platforms: vec!["linux".to_string()],
                hardware_cost_usd: 300, // Standard 10GbE NIC
            },
            TransportType::Dpdk => Self {
                transport_type,
                max_throughput_bps: 10_000_000_000, // 10 GB/s
                min_bandwidth_bps: 10_000_000_000,  // 100 Gbps
                supports_zero_copy: true,
                uses_kernel_bypass: true,
                requires_special_hardware: true,
                min_cpu_cores: 8,
                min_ram_bytes: 16_000_000_000, // 16GB
                supported_platforms: vec![
                    "linux".to_string(),
                    "freebsd".to_string(),
                    "windows".to_string(),
                ],
                hardware_cost_usd: 2000, // DPDK-compatible NIC
            },
            TransportType::Quic => Self {
                transport_type,
                max_throughput_bps: 1_400_000_000, // 1.4 GB/s with dynamic window + jumbo frames
                min_bandwidth_bps: 125_000_000,    // 1 Gbps
                supports_zero_copy: false,
                uses_kernel_bypass: false,
                requires_special_hardware: false,
                min_cpu_cores: 2,
                min_ram_bytes: 2_000_000_000, // 2GB
                supported_platforms: vec![
                    "linux".to_string(),
                    "windows".to_string(),
                    "macos".to_string(),
                ],
                hardware_cost_usd: 50, // Standard NIC
            },
            TransportType::Tcp => Self {
                transport_type,
                max_throughput_bps: 250_000_000, // 250 MB/s with BBR + window scaling + jumbo frames + zero-copy
                min_bandwidth_bps: 125_000_000,  // 1 Gbps
                supports_zero_copy: cfg!(target_os = "linux"), // sendfile() on Linux only
                uses_kernel_bypass: false,
                requires_special_hardware: false,
                min_cpu_cores: 1,
                min_ram_bytes: 1_000_000_000, // 1GB
                supported_platforms: vec![
                    "linux".to_string(),
                    "windows".to_string(),
                    "macos".to_string(),
                ],
                hardware_cost_usd: 20, // Any NIC
            },
        }
    }

    /// Check if backend is available on current platform
    pub fn is_available(&self, platform: &Platform) -> bool {
        // Check platform support
        if !self.supported_platforms.contains(&platform.os) {
            return false;
        }

        // Check resource requirements
        if platform.cpu_cores < self.min_cpu_cores {
            return false;
        }

        if platform.total_ram_bytes < self.min_ram_bytes {
            return false;
        }

        // Check transport-specific requirements
        match self.transport_type {
            TransportType::IoUring => platform.supports_io_uring(),
            TransportType::Dpdk => platform.supports_dpdk(),
            TransportType::Quic | TransportType::Tcp => true,
        }
    }

    /// Get throughput in MB/s
    pub fn max_throughput_mbps(&self) -> f64 {
        self.max_throughput_bps as f64 / 1_000_000.0
    }

    /// Get throughput in GB/s
    pub fn max_throughput_gbps(&self) -> f64 {
        self.max_throughput_bps as f64 / 1_000_000_000.0
    }

    /// Get performance tier
    pub fn performance_tier(&self) -> &str {
        match self.max_throughput_bps {
            x if x >= 10_000_000_000 => "Enterprise",
            x if x >= 1_000_000_000 => "Professional",
            x if x >= 500_000_000 => "Standard",
            _ => "Basic",
        }
    }
}

impl std::fmt::Display for BackendCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - Max: {:.2} GB/s, Zero-copy: {}, Kernel bypass: {}, Cost: ${}",
            self.transport_type,
            self.performance_tier(),
            self.max_throughput_gbps(),
            self.supports_zero_copy,
            self.uses_kernel_bypass,
            self.hardware_cost_usd
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();
        assert!(!platform.os.is_empty());
        assert!(!platform.arch.is_empty());
        assert!(platform.cpu_cores > 0);
        assert!(platform.total_ram_bytes > 0);
    }

    #[test]
    fn test_io_uring_capabilities() {
        let caps = BackendCapabilities::for_transport(TransportType::IoUring);
        assert_eq!(caps.max_throughput_bps, 20_000_000_000); // 20 GB/s with enhanced async I/O
        assert!(caps.supports_zero_copy);
        assert!(caps.uses_kernel_bypass);
        assert!(!caps.requires_special_hardware);
        assert_eq!(caps.performance_tier(), "Enterprise");
    }

    #[test]
    fn test_dpdk_capabilities() {
        let caps = BackendCapabilities::for_transport(TransportType::Dpdk);
        assert_eq!(caps.max_throughput_bps, 10_000_000_000);
        assert!(caps.requires_special_hardware);
        assert_eq!(caps.performance_tier(), "Enterprise");
    }

    #[test]
    fn test_throughput_conversion() {
        let caps = BackendCapabilities::for_transport(TransportType::IoUring);
        assert_eq!(caps.max_throughput_mbps(), 20000.0); // 20 GB/s = 20000 MB/s
        assert_eq!(caps.max_throughput_gbps(), 20.0);
    }

    #[test]
    fn test_availability_check() {
        let caps = BackendCapabilities::for_transport(TransportType::Tcp);
        let platform = Platform::detect();
        
        // TCP should be available on all platforms
        assert!(caps.is_available(&platform));
    }
}

// Made with Bob
