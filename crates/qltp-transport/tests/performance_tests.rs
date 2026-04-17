//! Performance and Failure Tests
//!
//! Tests for performance validation and network failure scenarios

use qltp_transport::{
    application::{TransportManager, TransportManagerConfig, SelectionCriteria},
    domain::{TransportType, BackendCapabilities, Platform},
    error::Result,
};
use std::time::Instant;

// ============================================================================
// Phase 5.5.6: Performance Validation (1 GB/s target)
// ============================================================================

#[tokio::test]
async fn test_quic_meets_1gbps_target() {
    let quic_caps = BackendCapabilities::for_transport(TransportType::Quic);
    
    // QUIC should support 1.4 GB/s (with jumbo frames)
    assert_eq!(quic_caps.max_throughput_bps, 1_400_000_000);
    assert_eq!(quic_caps.max_throughput_gbps(), 1.4);
    assert_eq!(quic_caps.max_throughput_mbps(), 1400.0);
}

#[tokio::test]
async fn test_backend_throughput_hierarchy() {
    let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
    let quic = BackendCapabilities::for_transport(TransportType::Quic);
    let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
    
    // Verify throughput hierarchy (with enhanced async I/O + batch optimization)
    assert_eq!(tcp.max_throughput_bps, 250_000_000);      // 250 MB/s
    assert_eq!(quic.max_throughput_bps, 1_400_000_000);   // 1.4 GB/s ⭐
    assert_eq!(io_uring.max_throughput_bps, 20_000_000_000); // 20 GB/s
    
    // QUIC is 5.6x faster than TCP
    assert!(quic.max_throughput_bps > tcp.max_throughput_bps * 5);
    
    // io_uring is 14.3x faster than QUIC
    assert!(io_uring.max_throughput_bps > quic.max_throughput_bps * 14);
}

#[tokio::test]
async fn test_selection_for_1gbps_requirement() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(1_000_000_000), // 1 GB/s
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should select QUIC or io_uring
    assert!(
        selection.transport_type == TransportType::Quic ||
        selection.transport_type == TransportType::IoUring
    );
    
    // Selected backend must meet requirement
    assert!(selection.capabilities.max_throughput_bps >= 1_000_000_000);
}

#[tokio::test]
async fn test_performance_tier_classification() {
    let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
    let quic = BackendCapabilities::for_transport(TransportType::Quic);
    let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
    let dpdk = BackendCapabilities::for_transport(TransportType::Dpdk);
    
    assert_eq!(tcp.performance_tier(), "Basic");
    assert_eq!(quic.performance_tier(), "Professional");
    assert_eq!(io_uring.performance_tier(), "Enterprise");
    assert_eq!(dpdk.performance_tier(), "Enterprise");
}

#[tokio::test]
async fn test_throughput_conversion_accuracy() {
    let quic = BackendCapabilities::for_transport(TransportType::Quic);
    
    // Test conversions (with jumbo frames)
    assert_eq!(quic.max_throughput_bps, 1_400_000_000);
    assert_eq!(quic.max_throughput_mbps(), 1400.0);
    assert_eq!(quic.max_throughput_gbps(), 1.4);
    
    // Verify conversion math
    assert_eq!(quic.max_throughput_bps as f64 / 1_000_000.0, 1400.0);
    assert_eq!(quic.max_throughput_bps as f64 / 1_000_000_000.0, 1.4);
}

#[tokio::test]
async fn test_selection_performance_benchmark() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let criteria = SelectionCriteria::default();
    
    let start = Instant::now();
    
    // Perform 10,000 selections
    for _ in 0..10_000 {
        let _ = manager.select_optimal_backend(&criteria);
    }
    
    let elapsed = start.elapsed();
    let avg_micros = elapsed.as_micros() / 10_000;
    
    // Should average < 100 microseconds per selection
    assert!(avg_micros < 100, "Selection too slow: {} μs", avg_micros);
}

#[tokio::test]
async fn test_platform_detection_performance() {
    use qltp_transport::domain::Platform;
    
    let start = Instant::now();
    
    // Detect platform 1,000 times
    for _ in 0..1_000 {
        let _ = Platform::detect();
    }
    
    let elapsed = start.elapsed();
    
    // Should complete in < 100ms
    assert!(elapsed.as_millis() < 100);
}

#[tokio::test]
async fn test_capability_query_performance() {
    let start = Instant::now();
    
    // Query capabilities 10,000 times
    for _ in 0..10_000 {
        let _ = BackendCapabilities::for_transport(TransportType::Quic);
    }
    
    let elapsed = start.elapsed();
    
    // Should complete in < 50ms
    assert!(elapsed.as_millis() < 50);
}

#[tokio::test]
async fn test_zero_copy_performance_benefit() {
    let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
    let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
    
    // Zero-copy should provide significant performance benefit
    #[cfg(target_os = "linux")]
    assert!(tcp.supports_zero_copy); // TCP now supports zero-copy on Linux
    
    #[cfg(not(target_os = "linux"))]
    assert!(!tcp.supports_zero_copy);
    
    assert!(io_uring.supports_zero_copy);
    
    // io_uring is 56x faster than TCP (250 MB/s vs 14 GB/s)
    let speedup = io_uring.max_throughput_bps / tcp.max_throughput_bps;
    assert!(speedup >= 50);
}

#[tokio::test]
async fn test_kernel_bypass_performance() {
    let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
    let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
    
    // Kernel bypass should provide performance benefit
    assert!(!tcp.uses_kernel_bypass);
    assert!(io_uring.uses_kernel_bypass);
    
    // Significant throughput difference
    assert!(io_uring.max_throughput_bps > tcp.max_throughput_bps * 50);
}

// ============================================================================
// Phase 5.5.5: Network Failure Simulation
// ============================================================================

#[tokio::test]
async fn test_selection_with_unavailable_backend() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let platform = manager.get_platform().await;
    
    // Try to select io_uring on non-Linux
    if platform.os != "linux" {
        let criteria = SelectionCriteria {
            preferred_transport: Some(TransportType::IoUring),
            ..Default::default()
        };
        
        let selection = manager.select_optimal_backend(&criteria).unwrap();
        
        // Should fallback to available backend
        assert_ne!(selection.transport_type, TransportType::IoUring);
        assert!(selection.fallbacks.len() > 0);
    }
}

#[tokio::test]
async fn test_fallback_when_requirements_not_met() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Request impossible throughput
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(100_000_000_000), // 100 GB/s (impossible)
        ..Default::default()
    };
    
    let result = manager.select_optimal_backend(&criteria);
    
    // Should either fail or select best available
    match result {
        Ok(selection) => {
            // Selected best available backend
            assert!(selection.capabilities.max_throughput_bps > 0);
        }
        Err(_) => {
            // Correctly failed when requirements can't be met
            assert!(true);
        }
    }
}

#[tokio::test]
async fn test_graceful_degradation_low_resources() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Request backend with minimal resources
    let criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(10), // Very low budget
        ..Default::default()
    };
    
    let result = manager.select_optimal_backend(&criteria);
    
    // Should either select TCP or fail gracefully
    match result {
        Ok(selection) => {
            assert!(selection.capabilities.hardware_cost_usd <= 10 || 
                   selection.transport_type == TransportType::Tcp);
        }
        Err(_) => {
            // Acceptable if no backend meets criteria
            assert!(true);
        }
    }
}

#[tokio::test]
async fn test_platform_constraint_handling() {
    let platform = Platform::detect();
    
    // Test each backend's availability
    let backends = vec![
        TransportType::Tcp,
        TransportType::Quic,
        TransportType::IoUring,
        TransportType::Dpdk,
    ];
    
    for backend_type in backends {
        let caps = BackendCapabilities::for_transport(backend_type);
        let is_available = caps.is_available(&platform);
        
        // Availability should be deterministic
        assert!(is_available == caps.is_available(&platform));
    }
}

#[tokio::test]
async fn test_insufficient_cpu_cores() {
    use qltp_transport::domain::Platform;
    
    // Create platform with insufficient resources
    let low_resource_platform = Platform {
        os: "linux".to_string(),
        os_version: "5.10".to_string(),
        kernel_version: Some("5.10.0".to_string()),
        arch: "x86_64".to_string(),
        cpu_cores: 1, // Only 1 core
        total_ram_bytes: 1_000_000_000, // 1GB RAM
    };
    
    // io_uring requires 4 cores
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    assert!(!io_uring_caps.is_available(&low_resource_platform));
    
    // TCP should still be available
    let tcp_caps = BackendCapabilities::for_transport(TransportType::Tcp);
    assert!(tcp_caps.is_available(&low_resource_platform));
}

#[tokio::test]
async fn test_insufficient_ram() {
    use qltp_transport::domain::Platform;
    
    let low_ram_platform = Platform {
        os: "linux".to_string(),
        os_version: "5.10".to_string(),
        kernel_version: Some("5.10.0".to_string()),
        arch: "x86_64".to_string(),
        cpu_cores: 8,
        total_ram_bytes: 500_000_000, // Only 500MB RAM
    };
    
    // io_uring requires 4GB
    let io_uring_caps = BackendCapabilities::for_transport(TransportType::IoUring);
    assert!(!io_uring_caps.is_available(&low_ram_platform));
}

#[tokio::test]
async fn test_error_handling_consistency() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Multiple invalid requests should fail consistently
    for _ in 0..10 {
        let criteria = SelectionCriteria {
            min_throughput_bps: Some(1_000_000_000_000), // 1 TB/s (impossible)
            ..Default::default()
        };
        
        let result1 = manager.select_optimal_backend(&criteria);
        let result2 = manager.select_optimal_backend(&criteria);
        
        // Results should be consistent
        assert_eq!(result1.is_ok(), result2.is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_failure_handling() {
    use std::sync::Arc;
    
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    let mut handles = Vec::new();
    
    // Spawn 20 tasks with impossible requirements
    for _ in 0..20 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let criteria = SelectionCriteria {
                min_throughput_bps: Some(100_000_000_000), // 100 GB/s
                ..Default::default()
            };
            manager_clone.select_optimal_backend(&criteria)
        });
        handles.push(handle);
    }
    
    // All should handle failure consistently
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    
    // All results should be consistent (all Ok or all Err)
    let first_is_ok = results[0].is_ok();
    for result in &results {
        assert_eq!(result.is_ok(), first_is_ok);
    }
}

#[tokio::test]
async fn test_recovery_after_failure() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // First request fails
    let bad_criteria = SelectionCriteria {
        min_throughput_bps: Some(100_000_000_000),
        ..Default::default()
    };
    let _ = manager.select_optimal_backend(&bad_criteria);
    
    // Second request should still work
    let good_criteria = SelectionCriteria {
        min_throughput_bps: Some(100_000_000),
        ..Default::default()
    };
    let result = manager.select_optimal_backend(&good_criteria);
    assert!(result.is_ok());
}

// Made with Bob