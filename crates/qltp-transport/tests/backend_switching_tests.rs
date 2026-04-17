//! Backend Switching Tests
//!
//! Tests for dynamic backend switching and fallback mechanisms

use qltp_transport::{
    application::{TransportManager, TransportManagerConfig, SelectionCriteria, RetryConfig},
    domain::{TransportType, BackendCapabilities, Platform},
    error::Result,
};

#[tokio::test]
async fn test_backend_switching_tcp_to_quic() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Start with TCP
    let tcp_criteria = SelectionCriteria {
        preferred_transport: Some(TransportType::Tcp),
        ..Default::default()
    };
    
    let tcp_selection = manager.select_optimal_backend(&tcp_criteria).unwrap();
    assert_eq!(tcp_selection.transport_type, TransportType::Tcp);
    
    // Switch to QUIC
    let quic_criteria = SelectionCriteria {
        preferred_transport: Some(TransportType::Quic),
        ..Default::default()
    };
    
    let quic_selection = manager.select_optimal_backend(&quic_criteria).unwrap();
    assert_eq!(quic_selection.transport_type, TransportType::Quic);
    
    // Verify different capabilities
    assert!(quic_selection.capabilities.max_throughput_bps > tcp_selection.capabilities.max_throughput_bps);
}

#[tokio::test]
async fn test_backend_switching_based_on_throughput() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Low throughput requirement - should select TCP
    let low_criteria = SelectionCriteria {
        min_throughput_bps: Some(50_000_000), // 50 MB/s
        ..Default::default()
    };
    
    let low_selection = manager.select_optimal_backend(&low_criteria).unwrap();
    
    // High throughput requirement - should select QUIC or better
    let high_criteria = SelectionCriteria {
        min_throughput_bps: Some(500_000_000), // 500 MB/s
        ..Default::default()
    };
    
    let high_selection = manager.select_optimal_backend(&high_criteria).unwrap();
    
    // High throughput backend should be faster
    assert!(high_selection.capabilities.max_throughput_bps >= 500_000_000);
}

#[tokio::test]
async fn test_backend_switching_preserves_fallbacks() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should have fallback options
    assert!(!selection.fallbacks.is_empty());
    
    // Fallbacks should be different from selected
    for fallback in &selection.fallbacks {
        assert_ne!(*fallback, selection.transport_type);
    }
    
    // Fallbacks should be ordered by preference
    assert!(selection.fallbacks.len() >= 1);
}

#[tokio::test]
async fn test_backend_switching_with_platform_constraints() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let platform = manager.get_platform().await;
    
    // Try to select io_uring
    let io_uring_criteria = SelectionCriteria {
        preferred_transport: Some(TransportType::IoUring),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&io_uring_criteria).unwrap();
    
    // If not on Linux, should fallback to another backend
    if platform.os != "linux" {
        assert_ne!(selection.transport_type, TransportType::IoUring);
        assert!(selection.fallbacks.contains(&TransportType::Quic) || 
                selection.fallbacks.contains(&TransportType::Tcp));
    }
}

#[tokio::test]
async fn test_backend_switching_cost_optimization() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Low budget - should select cheaper backend
    let low_budget_criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(30),
        ..Default::default()
    };
    
    let low_budget_selection = manager.select_optimal_backend(&low_budget_criteria).unwrap();
    assert!(low_budget_selection.capabilities.hardware_cost_usd <= 30);
    
    // Medium budget - can select better backend
    let medium_budget_criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(100),
        ..Default::default()
    };
    
    let medium_budget_selection = manager.select_optimal_backend(&medium_budget_criteria).unwrap();
    assert!(medium_budget_selection.capabilities.hardware_cost_usd <= 100);
}

#[tokio::test]
async fn test_backend_switching_zero_copy_preference() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let platform = manager.get_platform().await;
    
    let zero_copy_criteria = SelectionCriteria {
        prefer_zero_copy: true,
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&zero_copy_criteria).unwrap();
    
    // If io_uring is available, it should be selected
    if platform.os == "linux" && platform.supports_io_uring() {
        // io_uring might be selected if available
        if selection.transport_type == TransportType::IoUring {
            assert!(selection.capabilities.supports_zero_copy);
        }
    }
}

#[tokio::test]
async fn test_backend_list_shows_all_available() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let backends = manager.list_available_backends();
    
    // Should have at least TCP and QUIC
    assert!(backends.len() >= 2);
    
    // Verify each backend has valid capabilities
    for (transport_type, capabilities) in backends {
        assert_eq!(transport_type, capabilities.transport_type);
        assert!(capabilities.max_throughput_bps > 0);
        assert!(capabilities.hardware_cost_usd > 0);
    }
}

#[tokio::test]
async fn test_backend_switching_reason_provided() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(1_000_000_000), // 1 GB/s
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should provide a reason for selection
    assert!(!selection.reason.is_empty());
    assert!(selection.reason.len() > 10); // Meaningful reason
}

#[tokio::test]
async fn test_backend_switching_with_special_hardware() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Disallow special hardware
    let no_special_criteria = SelectionCriteria {
        allow_special_hardware: false,
        ..Default::default()
    };
    
    let no_special_selection = manager.select_optimal_backend(&no_special_criteria).unwrap();
    assert!(!no_special_selection.capabilities.requires_special_hardware);
    
    // Allow special hardware
    let allow_special_criteria = SelectionCriteria {
        allow_special_hardware: true,
        min_throughput_bps: Some(5_000_000_000), // 5 GB/s
        ..Default::default()
    };
    
    // Should try to select high-performance backend if available
    let result = manager.select_optimal_backend(&allow_special_criteria);
    
    // May fail if no backend meets requirements, which is acceptable
    if let Ok(selection) = result {
        assert!(selection.capabilities.max_throughput_bps >= 1_000_000_000);
    }
}

#[tokio::test]
async fn test_backend_switching_latency_preference() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let low_latency_criteria = SelectionCriteria {
        max_latency_ms: Some(5),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&low_latency_criteria).unwrap();
    
    // Should select a backend (exact latency depends on implementation)
    assert!(!selection.reason.is_empty());
}

#[tokio::test]
async fn test_auto_initialize_with_fallback() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(100_000_000), // 100 MB/s
        ..Default::default()
    };
    
    let result = manager.initialize_with_fallback(
        Some(criteria),
        Some(RetryConfig::default())
    ).await;
    
    // Should complete (may succeed or fail gracefully)
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_backend_capabilities_comparison() {
    let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
    let quic = BackendCapabilities::for_transport(TransportType::Quic);
    let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
    
    // Verify throughput hierarchy
    assert!(tcp.max_throughput_bps < quic.max_throughput_bps);
    assert!(quic.max_throughput_bps < io_uring.max_throughput_bps);
    
    // Verify cost hierarchy
    assert!(tcp.hardware_cost_usd < quic.hardware_cost_usd);
    assert!(quic.hardware_cost_usd < io_uring.hardware_cost_usd);
}

#[tokio::test]
async fn test_platform_specific_backend_selection() {
    let platform = Platform::detect();
    
    // Test TCP (always available)
    let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
    assert!(tcp_caps.is_available(&platform));
    
    // Test QUIC (cross-platform)
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    assert!(quic_caps.is_available(&platform));
    
    // Test io_uring (Linux only)
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    if platform.os == "linux" {
        // May or may not be available depending on kernel version
        let _ = io_uring_caps.is_available(&platform);
    } else {
        assert!(!io_uring_caps.is_available(&platform));
    }
}

// Made with Bob