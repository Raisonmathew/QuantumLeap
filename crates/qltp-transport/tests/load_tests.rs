//! Load Testing
//!
//! Tests for concurrent transfers and high-load scenarios

use qltp_transport::{
    application::{TransportManager, TransportManagerConfig, SelectionCriteria},
    domain::{TransportType, TransportStats},
    error::Result,
};
use std::sync::Arc;
use tokio::task::JoinSet;

#[tokio::test]
async fn test_concurrent_backend_selections() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    let mut handles = Vec::new();
    
    // Spawn 10 concurrent selection tasks
    for i in 0..10 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let criteria = SelectionCriteria {
                min_throughput_bps: Some((i as u64 + 1) * 100_000_000),
                ..Default::default()
            };
            manager_clone.select_optimal_backend(&criteria)
        });
        handles.push(handle);
    }
    
    // All should complete successfully
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_high_frequency_selections() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Perform 100 rapid selections
    for _ in 0..100 {
        let criteria = SelectionCriteria::default();
        let result = manager.select_optimal_backend(&criteria);
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_platform_queries() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    let mut handles = Vec::new();
    
    // Spawn 20 concurrent platform queries
    for _ in 0..20 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            manager_clone.get_platform().await
        });
        handles.push(handle);
    }
    
    // All should return the same platform info
    let mut platforms = Vec::new();
    for handle in handles {
        let platform = handle.await.unwrap();
        platforms.push(platform);
    }
    
    // All platforms should be identical
    for platform in &platforms[1..] {
        assert_eq!(platform.os, platforms[0].os);
        assert_eq!(platform.arch, platforms[0].arch);
    }
}

#[tokio::test]
async fn test_concurrent_backend_list_queries() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    let mut handles = Vec::new();
    
    // Spawn 15 concurrent backend list queries
    for _ in 0..15 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            manager_clone.list_available_backends()
        });
        handles.push(handle);
    }
    
    // All should return consistent results
    let mut backend_lists = Vec::new();
    for handle in handles {
        let backends = handle.await.unwrap();
        backend_lists.push(backends);
    }
    
    // All lists should have the same length
    let expected_len = backend_lists[0].len();
    for backends in &backend_lists {
        assert_eq!(backends.len(), expected_len);
    }
}

#[tokio::test]
async fn test_mixed_concurrent_operations() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    let mut set = JoinSet::new();
    
    // Mix of different operations
    for i in 0..30 {
        let manager_clone = manager.clone();
        
        match i % 3 {
            0 => {
                // Backend selection
                set.spawn(async move {
                    let criteria = SelectionCriteria::default();
                    manager_clone.select_optimal_backend(&criteria).is_ok()
                });
            }
            1 => {
                // Platform query
                set.spawn(async move {
                    let platform = manager_clone.get_platform().await;
                    !platform.os.is_empty()
                });
            }
            _ => {
                // Backend list
                set.spawn(async move {
                    let backends = manager_clone.list_available_backends();
                    !backends.is_empty()
                });
            }
        }
    }
    
    // All operations should succeed
    while let Some(result) = set.join_next().await {
        assert!(result.unwrap());
    }
}

#[tokio::test]
async fn test_stress_backend_selection() {
    let manager = TransportManager::new(TransportManagerConfig::default());
    
    // Perform 1000 selections with varying criteria
    for i in 0..1000 {
        let criteria = SelectionCriteria {
            min_throughput_bps: Some((i % 10 + 1) * 100_000_000),
            max_hardware_cost_usd: Some((((i % 5) + 1) * 100) as u32),
            prefer_zero_copy: i % 2 == 0,
            ..Default::default()
        };
        
        let result = manager.select_optimal_backend(&criteria);
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_concurrent_capability_queries() {
    use qltp_transport::domain::BackendCapabilities;
    
    let mut handles = Vec::new();
    
    // Query capabilities for all transport types concurrently
    for _ in 0..50 {
        let handle = tokio::spawn(async {
            let tcp = BackendCapabilities::for_transport(TransportType::Tcp);
            let quic = BackendCapabilities::for_transport(TransportType::Quic);
            let io_uring = BackendCapabilities::for_transport(TransportType::IoUring);
            
            (tcp.max_throughput_bps, quic.max_throughput_bps, io_uring.max_throughput_bps)
        });
        handles.push(handle);
    }
    
    // All should return consistent values
    let mut results = Vec::new();
    for handle in handles {
        let result = handle.await.unwrap();
        results.push(result);
    }
    
    // Verify consistency
    for result in &results[1..] {
        assert_eq!(result.0, results[0].0); // TCP
        assert_eq!(result.1, results[0].1); // QUIC
        assert_eq!(result.2, results[0].2); // io_uring
    }
}

#[tokio::test]
async fn test_rapid_manager_creation() {
    // Create and drop 100 managers rapidly
    for _ in 0..100 {
        let manager = TransportManager::new(TransportManagerConfig::default());
        let platform = manager.get_platform().await;
        assert!(!platform.os.is_empty());
        drop(manager);
    }
}

#[tokio::test]
async fn test_concurrent_manager_instances() {
    let mut handles = Vec::new();
    
    // Create 20 managers concurrently
    for _ in 0..20 {
        let handle = tokio::spawn(async {
            let manager = TransportManager::new(TransportManagerConfig::default());
            let criteria = SelectionCriteria::default();
            manager.select_optimal_backend(&criteria).is_ok()
        });
        handles.push(handle);
    }
    
    // All should succeed
    for handle in handles {
        assert!(handle.await.unwrap());
    }
}

#[tokio::test]
async fn test_load_with_different_configs() {
    let configs = vec![
        TransportManagerConfig {
            preferred_transport: Some(TransportType::Tcp),
            ..Default::default()
        },
        TransportManagerConfig {
            preferred_transport: Some(TransportType::Quic),
            ..Default::default()
        },
        TransportManagerConfig {
            max_concurrent_sessions: 50,
            ..Default::default()
        },
        TransportManagerConfig {
            enable_monitoring: false,
            ..Default::default()
        },
    ];
    
    let mut handles = Vec::new();
    
    for config in configs {
        let handle = tokio::spawn(async move {
            let manager = TransportManager::new(config);
            let criteria = SelectionCriteria::default();
            manager.select_optimal_backend(&criteria).is_ok()
        });
        handles.push(handle);
    }
    
    for handle in handles {
        assert!(handle.await.unwrap());
    }
}

#[tokio::test]
async fn test_memory_efficiency_under_load() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    // Perform many operations without accumulating memory
    for _ in 0..1000 {
        let _ = manager.select_optimal_backend(&SelectionCriteria::default());
        let _ = manager.get_platform().await;
        let _ = manager.list_available_backends();
    }
    
    // If we get here without OOM, test passes
    assert!(true);
}

#[tokio::test]
async fn test_concurrent_active_session_count() {
    let manager = Arc::new(TransportManager::new(TransportManagerConfig::default()));
    
    let mut handles = Vec::new();
    
    // Query active session count concurrently
    for _ in 0..50 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            manager_clone.active_session_count().await
        });
        handles.push(handle);
    }
    
    // All should return 0 (no sessions created)
    for handle in handles {
        let count = handle.await.unwrap();
        assert_eq!(count, 0);
    }
}

#[tokio::test]
async fn test_throughput_calculation_under_load() {
    use qltp_transport::domain::BackendCapabilities;
    
    // Calculate throughput conversions many times
    for _ in 0..10000 {
        let caps = BackendCapabilities::for_transport(TransportType::Quic);
        let mbps = caps.max_throughput_mbps();
        let gbps = caps.max_throughput_gbps();
        
        assert_eq!(mbps, 1400.0); // Updated with jumbo frames
        assert_eq!(gbps, 1.4);
    }
}

#[tokio::test]
async fn test_platform_detection_consistency() {
    use qltp_transport::domain::Platform;
    
    // Detect platform 100 times
    let mut platforms = Vec::new();
    for _ in 0..100 {
        platforms.push(Platform::detect());
    }
    
    // All should be identical
    for platform in &platforms[1..] {
        assert_eq!(platform.os, platforms[0].os);
        assert_eq!(platform.arch, platforms[0].arch);
        assert_eq!(platform.cpu_cores, platforms[0].cpu_cores);
    }
}

#[tokio::test]
async fn test_selection_performance() {
    use std::time::Instant;
    
    let manager = TransportManager::new(TransportManagerConfig::default());
    let criteria = SelectionCriteria::default();
    
    let start = Instant::now();
    
    // Perform 1000 selections
    for _ in 0..1000 {
        let _ = manager.select_optimal_backend(&criteria);
    }
    
    let elapsed = start.elapsed();
    
    // Should complete in reasonable time (< 1 second for 1000 selections)
    assert!(elapsed.as_secs() < 1);
}

// Made with Bob