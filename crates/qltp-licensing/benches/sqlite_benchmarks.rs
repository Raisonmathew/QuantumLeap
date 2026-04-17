//! Performance benchmarks for SQLite operations
//!
//! Run with: cargo bench --features sqlite

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use qltp_licensing::{
    LicenseRepository, LicenseService, LicenseTier, SqliteLicenseStore, SqliteUsageStore,
    TransferType, UsageTracker,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn bench_license_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("license_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
                let service = LicenseService::new(store);
                
                black_box(
                    service
                        .create_license(LicenseTier::Pro, Some("bench@example.com".to_string()))
                        .await
                        .unwrap()
                );
            });
        });
    });
}

fn bench_license_lookup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Setup: Create a license
    let (store, key) = rt.block_on(async {
        let store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
        let service = LicenseService::new(store.clone());
        let license = service
            .create_license(LicenseTier::Pro, Some("bench@example.com".to_string()))
            .await
            .unwrap();
        (store, license.key().to_string())
    });
    
    c.bench_function("license_lookup_by_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let service = LicenseService::new(store.clone());
                black_box(service.get_license(&key).await.unwrap());
            });
        });
    });
}

fn bench_usage_recording(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Setup: Create a license
    let (license_store, usage_store, license_id) = rt.block_on(async {
        let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
        let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
        let service = LicenseService::new(license_store.clone());
        let license = service
            .create_license(LicenseTier::Pro, Some("bench@example.com".to_string()))
            .await
            .unwrap();
        (license_store, usage_store, license.id().clone())
    });
    
    c.bench_function("usage_record_single", |b| {
        b.iter(|| {
            rt.block_on(async {
                let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());
                black_box(
                    tracker
                        .record_transfer(
                            license_id.clone(),
                            1024 * 1024, // 1MB
                            TransferType::Upload,
                        )
                        .await
                        .unwrap()
                );
            });
        });
    });
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    
    for num_concurrent in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_concurrent),
            num_concurrent,
            |b, &num| {
                b.iter(|| {
                    rt.block_on(async {
                        let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
                        let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
                        let service = LicenseService::new(license_store.clone());
                        
                        // Create a license
                        let license = service
                            .create_license(LicenseTier::Pro, Some("bench@example.com".to_string()))
                            .await
                            .unwrap();
                        
                        let tracker = Arc::new(UsageTracker::new(
                            license_store.clone(),
                            usage_store.clone(),
                        ));
                        
                        // Spawn concurrent usage recordings
                        let mut handles = vec![];
                        for _ in 0..num {
                            let tracker = tracker.clone();
                            let license_id = license.id().clone();
                            let handle = tokio::spawn(async move {
                                tracker
                                    .record_transfer(
                                        license_id,
                                        1024 * 1024, // 1MB
                                        TransferType::Upload,
                                    )
                                    .await
                            });
                            handles.push(handle);
                        }
                        
                        // Wait for all to complete
                        for handle in handles {
                            black_box(handle.await.unwrap().unwrap());
                        }
                    });
                });
            },
        );
    }
    
    group.finish();
}

fn bench_bulk_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("bulk_operations");
    
    for num_licenses in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_licenses),
            num_licenses,
            |b, &num| {
                b.iter(|| {
                    rt.block_on(async {
                        let store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
                        let service = LicenseService::new(store.clone());
                        
                        // Create multiple licenses
                        for i in 0..num {
                            black_box(
                                service
                                    .create_license(
                                        LicenseTier::Pro,
                                        Some(format!("bench{}@example.com", i)),
                                    )
                                    .await
                                    .unwrap()
                            );
                        }
                        
                        // List all licenses
                        black_box(store.list_all().await.unwrap());
                    });
                });
            },
        );
    }
    
    group.finish();
}

fn bench_quota_check(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Setup: Create a license with some usage
    let (license_store, usage_store, license_id) = rt.block_on(async {
        let license_store = Arc::new(SqliteLicenseStore::in_memory().unwrap());
        let usage_store = Arc::new(SqliteUsageStore::in_memory().unwrap());
        let service = LicenseService::new(license_store.clone());
        let license = service
            .create_license(LicenseTier::Pro, Some("bench@example.com".to_string()))
            .await
            .unwrap();
        
        // Add some usage history
        let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());
        for _ in 0..10 {
            tracker
                .record_transfer(
                    license.id().clone(),
                    100 * 1024 * 1024, // 100MB
                    TransferType::Upload,
                )
                .await
                .unwrap();
        }
        
        (license_store, usage_store, license.id().clone())
    });
    
    c.bench_function("quota_check_with_history", |b| {
        b.iter(|| {
            rt.block_on(async {
                let tracker = UsageTracker::new(license_store.clone(), usage_store.clone());
                black_box(
                    tracker
                        .check_quota(&license_id, 1024 * 1024)
                        .await
                        .unwrap()
                );
            });
        });
    });
}

criterion_group!(
    benches,
    bench_license_creation,
    bench_license_lookup,
    bench_usage_recording,
    bench_concurrent_operations,
    bench_bulk_operations,
    bench_quota_check
);
criterion_main!(benches);

// Made with Bob
