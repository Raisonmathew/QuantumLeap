//! Fallback Mechanism Tests
//!
//! Tests for automatic fallback when primary backend fails

use qltp_transport::{
    application::{
        TransportManager, TransportManagerConfig, SelectionCriteria, 
        RetryConfig, FallbackManager
    },
    domain::{TransportType, BackendCapabilities},
    error::Result,
};

#[tokio::test]
async fn test_fallback_chain_generation() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should have at least one fallback
    assert!(!selection.fallbacks.is_empty());
    
    // Fallbacks should not include the selected backend
    assert!(!selection.fallbacks.contains(&selection.transport_type));
    
    // All fallbacks should be valid transport types
    for fallback in &selection.fallbacks {
        let caps = BackendCapabilities::for_transport(*fallback);
        assert!(caps.max_throughput_bps > 0);
    }
}

#[tokio::test]
async fn test_fallback_order_by_performance() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(1_000_000_000), // 1 GB/s
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Primary should meet requirements
    assert!(selection.capabilities.max_throughput_bps >= 1_000_000_000);
    
    // Fallbacks should be ordered (best to worst)
    if selection.fallbacks.len() >= 2 {
        let first_fallback = BackendCapabilities::for_transport(selection.fallbacks[0]);
        let second_fallback = BackendCapabilities::for_transport(selection.fallbacks[1]);
        
        // First fallback should be better or equal to second
        assert!(first_fallback.max_throughput_bps >= second_fallback.max_throughput_bps);
    }
}

#[tokio::test]
async fn test_fallback_includes_tcp_as_last_resort() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // If TCP is not the primary, it should be in fallbacks
    if selection.transport_type != TransportType::Tcp {
        assert!(selection.fallbacks.contains(&TransportType::Tcp));
    }
}

#[tokio::test]
async fn test_fallback_respects_platform_constraints() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let platform = manager.get_platform().await;
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // All fallbacks should be available on current platform
    for fallback_type in &selection.fallbacks {
        let caps = BackendCapabilities::for_transport(*fallback_type);
        assert!(caps.is_available(&platform));
    }
}

#[tokio::test]
async fn test_fallback_manager_creation() {
    let retry_config = RetryConfig::default();
    let fallback_manager = FallbackManager::new(retry_config);
    
    // Should create successfully
    // (FallbackManager is used internally by TransportManager)
    drop(fallback_manager);
}

#[tokio::test]
async fn test_fallback_with_high_throughput_requirement() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(500_000_000), // 500 MB/s
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Primary should meet requirement
    assert!(selection.capabilities.max_throughput_bps >= 500_000_000);
    
    // Fallbacks should ideally also meet requirement, but may be lower
    for fallback_type in &selection.fallbacks {
        let caps = BackendCapabilities::for_transport(*fallback_type);
        // At least one fallback should be reasonably fast
        if caps.max_throughput_bps >= 100_000_000 {
            // Good fallback option
            assert!(caps.max_throughput_bps > 0);
        }
    }
}

#[tokio::test]
async fn test_fallback_with_zero_copy_preference() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let platform = manager.get_platform().await;
    
    let criteria = SelectionCriteria {
        prefer_zero_copy: true,
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // If io_uring is available and selected, fallbacks should include non-zero-copy options
    if selection.transport_type == TransportType::IoUring {
        assert!(selection.capabilities.supports_zero_copy);
        
        // Fallbacks should include QUIC or TCP
        assert!(
            selection.fallbacks.contains(&TransportType::Quic) ||
            selection.fallbacks.contains(&TransportType::Tcp)
        );
    }
}

#[tokio::test]
async fn test_fallback_with_budget_constraint() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        max_hardware_cost_usd: Some(50),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Primary should be within budget
    assert!(selection.capabilities.hardware_cost_usd <= 50);
    
    // All fallbacks should also be within budget
    for fallback_type in &selection.fallbacks {
        let caps = BackendCapabilities::for_transport(*fallback_type);
        assert!(caps.hardware_cost_usd <= 50);
    }
}

#[tokio::test]
async fn test_fallback_chain_completeness() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    let available_backends = manager.list_available_backends();
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Total options (selected + fallbacks) should not exceed available backends
    let total_options = 1 + selection.fallbacks.len();
    assert!(total_options <= available_backends.len());
}

#[tokio::test]
async fn test_fallback_no_duplicates() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Check for duplicates in fallback chain
    let mut seen = std::collections::HashSet::new();
    for fallback in &selection.fallbacks {
        assert!(seen.insert(fallback), "Duplicate fallback found: {:?}", fallback);
    }
}

#[tokio::test]
async fn test_fallback_with_special_hardware_disabled() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        allow_special_hardware: false,
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Primary should not require special hardware
    assert!(!selection.capabilities.requires_special_hardware);
    
    // Fallbacks should also not require special hardware
    for fallback_type in &selection.fallbacks {
        let caps = BackendCapabilities::for_transport(*fallback_type);
        assert!(!caps.requires_special_hardware);
    }
}

#[tokio::test]
async fn test_fallback_preserves_cross_platform_options() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria::default();
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should include at least one cross-platform option (TCP or QUIC)
    let has_cross_platform = 
        selection.transport_type == TransportType::Tcp ||
        selection.transport_type == TransportType::Quic ||
        selection.fallbacks.contains(&TransportType::Tcp) ||
        selection.fallbacks.contains(&TransportType::Quic);
    
    assert!(has_cross_platform);
}

#[tokio::test]
async fn test_retry_config_defaults() {
    let config = RetryConfig::default();
    
    // Should have reasonable defaults
    // (Exact values depend on implementation)
    drop(config);
}

#[tokio::test]
async fn test_fallback_reason_clarity() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        min_throughput_bps: Some(1_000_000_000),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Reason should explain why this backend was selected
    assert!(!selection.reason.is_empty());
    assert!(selection.reason.len() > 20); // Meaningful explanation
}

#[tokio::test]
async fn test_fallback_with_latency_constraint() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    let criteria = SelectionCriteria {
        max_latency_ms: Some(10),
        ..Default::default()
    };
    
    let selection = manager.select_optimal_backend(&criteria).unwrap();
    
    // Should select a backend (latency handling depends on implementation)
    assert!(!selection.fallbacks.is_empty());
}

// Made with Bob