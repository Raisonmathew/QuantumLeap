//! Integration Tests for Transport Layer
//!
//! Tests transport manager and backend selection without requiring actual network backends

use qltp_transport::{
    application::{TransportManager, TransportManagerConfig, SelectionCriteria},
    domain::{SessionConfig, SessionState, TransportType, Platform, BackendCapabilities},
    error::Result,
};
use std::net::SocketAddr;

/// Helper to create test manager
fn create_test_manager() -> TransportManager {
    TransportManager::new(TransportManagerConfig::default())
}

/// Helper to create test session config
fn create_test_config() -> SessionConfig {
    SessionConfig {
        transport_type: TransportType::Tcp,
        local_addr: "0.0.0.0:0".parse().unwrap(),
        remote_addr: "127.0.0.1:8080".parse().unwrap(),
        max_transfer_size: 10_000_000_000,
        connection_timeout_secs: 30,
        enable_compression: true,
        enable_encryption: true,
    }
}

#[tokio::test]
async fn test_manager_creation() {
    let manager = create_test_manager();
    
    // Manager should be created successfully
    let platform = manager.get_platform().await;
    assert!(!platform.os.is_empty());
    assert!(platform.cpu_cores > 0);
}

#[tokio::test]
async fn test_backend_selection_tcp() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        preferred_transport: Some(TransportType::Tcp),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    assert_eq!(selection.transport_type, TransportType::Tcp);
    assert!(!selection.reason.is_empty());
}

#[tokio::test]
async fn test_backend_selection_quic() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        preferred_transport: Some(TransportType::Quic),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    assert_eq!(selection.transport_type, TransportType::Quic);
}

#[tokio::test]
async fn test_backend_selection_high_throughput() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(1_000_000_000), // 1 GB/s
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should select QUIC or io_uring (if available)
    assert!(
        selection.transport_type == TransportType::Quic ||
        selection.transport_type == TransportType::IoUring
    );
}

#[tokio::test]
async fn test_backend_selection_zero_copy() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        prefer_zero_copy: true,
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // If io_uring is available, it should be selected
    if selection.transport_type == TransportType::IoUring {
        assert!(selection.capabilities.supports_zero_copy);
    }
}

#[tokio::test]
async fn test_list_available_backends() {
    let manager = create_test_manager();
    
    let backends = manager.list_available_backends();
    
    // Should have at least TCP and QUIC
    assert!(backends.len() >= 2);
    
    // TCP should always be available
    assert!(backends.iter().any(|(t, _)| *t == TransportType::Tcp));
    
    // QUIC should be available on all platforms
    assert!(backends.iter().any(|(t, _)| *t == TransportType::Quic));
}

#[tokio::test]
async fn test_platform_detection() {
    let platform = Platform::detect();
    
    assert!(!platform.os.is_empty());
    assert!(!platform.arch.is_empty());
    assert!(platform.cpu_cores > 0);
    assert!(platform.total_ram_bytes > 0);
}

#[tokio::test]
async fn test_backend_capabilities_tcp() {
    let caps = BackendCapabilities::for_transport(TransportType::Tcp);
    
    assert_eq!(caps.transport_type, TransportType::Tcp);
    assert_eq!(caps.max_throughput_bps, 250_000_000); // 250 MB/s (with BBR + window scaling + jumbo frames + zero-copy)
    
    // Zero-copy is only supported on Linux
    #[cfg(target_os = "linux")]
    assert!(caps.supports_zero_copy);
    
    #[cfg(not(target_os = "linux"))]
    assert!(!caps.supports_zero_copy);
    
    assert!(!caps.uses_kernel_bypass);
    assert!(!caps.requires_special_hardware);
}

#[tokio::test]
async fn test_backend_capabilities_quic() {
    let caps = BackendCapabilities::for_transport(TransportType::Quic);
    
    assert_eq!(caps.transport_type, TransportType::Quic);
    assert_eq!(caps.max_throughput_bps, 1_400_000_000); // 1.4 GB/s (with BBR + jumbo frames)
    assert!(!caps.supports_zero_copy);
    assert!(!caps.uses_kernel_bypass);
    assert!(!caps.requires_special_hardware);
    assert_eq!(caps.max_throughput_gbps(), 1.4);
}

#[tokio::test]
async fn test_backend_capabilities_io_uring() {
    let caps = BackendCapabilities::for_transport(TransportType::IoUring);
    
    assert_eq!(caps.transport_type, TransportType::IoUring);
    assert_eq!(caps.max_throughput_bps, 20_000_000_000); // 20 GB/s (with enhanced async I/O + batch optimization)
    assert!(caps.supports_zero_copy);
    assert!(caps.uses_kernel_bypass);
    assert!(!caps.requires_special_hardware);
    assert_eq!(caps.max_throughput_gbps(), 20.0);
}

#[tokio::test]
async fn test_backend_availability() {
    let platform = Platform::detect();
    
    // TCP should always be available
    let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
    assert!(tcp_caps.is_available(&platform));
    
    // QUIC should be available on all platforms
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    assert!(quic_caps.is_available(&platform));
    
    // io_uring only on Linux 5.1+
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    if platform.os == "linux" && platform.supports_io_uring() {
        assert!(io_uring_caps.is_available(&platform));
    }
}

#[tokio::test]
async fn test_session_config_creation() {
    let config = create_test_config();
    
    assert_eq!(config.transport_type, TransportType::Tcp);
    assert_eq!(config.max_transfer_size, 10_000_000_000);
    assert_eq!(config.connection_timeout_secs, 30);
    assert!(config.enable_compression);
    assert!(config.enable_encryption);
}

#[tokio::test]
async fn test_session_config_with_socket_addr() {
    let remote: SocketAddr = "192.168.1.100:9000".parse().unwrap();
    
    let config = SessionConfig {
        transport_type: TransportType::Quic,
        local_addr: "0.0.0.0:0".parse().unwrap(),
        remote_addr: remote,
        max_transfer_size: 1_000_000_000,
        connection_timeout_secs: 60,
        enable_compression: false,
        enable_encryption: true,
    };
    
    assert_eq!(config.remote_addr, remote);
    assert_eq!(config.transport_type, TransportType::Quic);
    assert!(!config.enable_compression);
}

#[tokio::test]
async fn test_auto_selection_with_criteria() {
    let manager = create_test_manager();
    
    // Test with different criteria
    let criteria_sets = vec![
        SelectionCriteria {
            min_throughput_bps: Some(100_000_000), // 100 MB/s
            ..Default::default()
        },
        SelectionCriteria {
            max_latency_ms: Some(10),
            ..Default::default()
        },
        SelectionCriteria {
            prefer_zero_copy: true,
            ..Default::default()
        },
        SelectionCriteria {
            allow_special_hardware: false,
            ..Default::default()
        },
    ];
    
    for criteria in criteria_sets {
        let result = manager.select_optimal_backend(&criteria);
        assert!(result.is_ok());
        let selection = result.unwrap();
        assert!(!selection.reason.is_empty());
        assert!(!selection.fallbacks.is_empty());
    }
}

#[tokio::test]
async fn test_fallback_chain() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should have fallback options
    assert!(!selection.fallbacks.is_empty());
    
    // Fallbacks should not include the selected backend
    assert!(!selection.fallbacks.contains(&selection.transport_type));
}

#[tokio::test]
async fn test_performance_tiers() {
    let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    
    assert_eq!(tcp_caps.performance_tier(), "Basic");
    assert_eq!(quic_caps.performance_tier(), "Professional");
    assert_eq!(io_uring_caps.performance_tier(), "Enterprise");
}

#[tokio::test]
async fn test_throughput_conversions() {
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    
    assert_eq!(quic_caps.max_throughput_bps, 1_400_000_000);
    assert_eq!(quic_caps.max_throughput_mbps(), 1400.0);
    assert_eq!(quic_caps.max_throughput_gbps(), 1.4);
}

#[tokio::test]
async fn test_hardware_cost_estimates() {
    let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    let dpdk_caps = BackendCapabilities::for_transport(TransportType::Dpdk);
    
    assert_eq!(tcp_caps.hardware_cost_usd, 20);
    assert_eq!(quic_caps.hardware_cost_usd, 50);
    assert_eq!(io_uring_caps.hardware_cost_usd, 300);
    assert_eq!(dpdk_caps.hardware_cost_usd, 2000);
}

#[tokio::test]
async fn test_platform_resource_check() {
    let platform = Platform::detect();
    
    // Should have sufficient resources for basic operations
    assert!(platform.has_sufficient_resources());
}

#[tokio::test]
async fn test_io_uring_platform_support() {
    let platform = Platform::detect();
    
    if platform.os == "linux" {
        // Check if io_uring is supported
        let supports = platform.supports_io_uring();
        
        // If kernel version is available, verify logic
        if let Some(ref kernel) = platform.kernel_version {
            if let Some(major) = kernel.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
                if major >= 5 {
                    assert!(supports);
                }
            }
        }
    } else {
        // Non-Linux platforms don't support io_uring
        assert!(!platform.supports_io_uring());
    }
}

#[tokio::test]
async fn test_dpdk_platform_support() {
    let platform = Platform::detect();
    
    // DPDK supports Linux, FreeBSD, and Windows
    let expected_support = matches!(platform.os.as_str(), "linux" | "freebsd" | "windows");
    assert_eq!(platform.supports_dpdk(), expected_support);
}

#[tokio::test]
async fn test_selection_with_budget_constraint() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(100), // Low budget
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should select TCP or QUIC (both under $100)
    assert!(
        selection.transport_type == TransportType::Tcp ||
        selection.transport_type == TransportType::Quic
    );
    assert!(selection.capabilities.hardware_cost_usd <= 100);
}

#[tokio::test]
async fn test_selection_with_high_budget() {
    let manager = create_test_manager();
    
    let criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(5000), // High budget
        allow_special_hardware: true,
        min_throughput_bps: Some(1_000_000_000), // 1 GB/s (more realistic)
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should select high-performance backend (QUIC or better)
    assert!(selection.capabilities.max_throughput_bps >= 1_000_000_000);
}

#[tokio::test]
async fn test_manager_config_defaults() {
    let config = TransportManagerConfig::default();
    
    assert!(config.auto_select_backend);
    assert_eq!(config.max_concurrent_sessions, 100);
    assert!(config.enable_monitoring);
    assert!(config.preferred_transport.is_none());
}

#[tokio::test]
async fn test_manager_with_custom_config() {
    let config = TransportManagerConfig {
        preferred_transport: Some(TransportType::Quic),
        auto_select_backend: false,
        max_concurrent_sessions: 50,
        enable_monitoring: false,
    };
    
    let manager = TransportManager::new(config);
    let platform = manager.get_platform().await;
    assert!(!platform.os.is_empty());
}

// Made with Bob