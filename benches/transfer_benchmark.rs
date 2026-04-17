//! Performance benchmarks for QLTP file transfer
//!
//! Run with: cargo bench --bench transfer_benchmark

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use qltp_compression::{compress, Algorithm, CompressionLevel};
use qltp_core::hash::compute_hash;
use std::io::Write;
use tempfile::NamedTempFile;

/// Generate test data of specified size
fn generate_test_data(size: usize, pattern: u8) -> Vec<u8> {
    vec![pattern; size]
}

/// Generate random test data
fn generate_random_data(size: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen()).collect()
}

/// Benchmark compression algorithms
fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");
    
    // Test different data sizes
    for size in [4096, 65536, 1048576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        // Compressible data (repeated pattern)
        let compressible_data = generate_test_data(*size, 0x42);
        
        group.bench_with_input(
            BenchmarkId::new("lz4_compressible", size),
            &compressible_data,
            |b, data| {
                b.iter(|| {
                    compress(black_box(data), Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap()
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("zstd_compressible", size),
            &compressible_data,
            |b, data| {
                b.iter(|| {
                    compress(black_box(data), Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap()
                });
            },
        );
        
        // Random data (incompressible)
        let random_data = generate_random_data(*size);
        
        group.bench_with_input(
            BenchmarkId::new("lz4_random", size),
            &random_data,
            |b, data| {
                b.iter(|| {
                    compress(black_box(data), Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap()
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("zstd_random", size),
            &random_data,
            |b, data| {
                b.iter(|| {
                    compress(black_box(data), Algorithm::Zstd, CompressionLevel::DEFAULT).unwrap()
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark chunking strategies
fn bench_chunking(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("chunking");
    
    // Create test file
    let mut temp_file = NamedTempFile::new().unwrap();
    let test_data = generate_test_data(10 * 1024 * 1024, 0x42); // 10MB
    temp_file.write_all(&test_data).unwrap();
    temp_file.flush().unwrap();
    
    group.throughput(Throughput::Bytes(test_data.len() as u64));
    
    // Fixed-size chunking
    group.bench_function("fixed_size_64k", |b| {
        b.iter(|| {
            runtime.block_on(async {
                qltp_core::chunking::chunk_file(black_box(temp_file.path()), 65536).await.unwrap()
            })
        });
    });
    
    // Content-defined chunking
    group.bench_function("content_defined_64k", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let chunker = qltp_core::chunking::ContentDefinedChunker::new(65536);
                chunker.chunk_file(black_box(temp_file.path())).await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark storage operations
fn bench_storage(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage");
    
    let temp_dir = tempfile::tempdir().unwrap();
    let mut store = runtime.block_on(async {
        qltp_storage::ContentStore::new(temp_dir.path()).await.unwrap()
    });
    
    let chunk_data = generate_test_data(65536, 0x42);
    let chunk_hash = compute_hash(&chunk_data);
    let chunk_id = hex::encode(chunk_hash);
    
    group.throughput(Throughput::Bytes(chunk_data.len() as u64));
    
    group.bench_function("store_chunk", |b| {
        b.iter(|| {
            runtime.block_on(async {
                store.store(black_box(&chunk_id), black_box(&chunk_data)).await.unwrap()
            })
        });
    });
    
    // Store chunk first for retrieval benchmark
    runtime.block_on(async {
        store.store(&chunk_id, &chunk_data).await.unwrap()
    });
    
    group.bench_function("retrieve_chunk", |b| {
        b.iter(|| {
            runtime.block_on(async {
                store.retrieve(black_box(&chunk_id)).await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark hash calculations
fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashing");
    
    for size in [4096, 65536, 1048576].iter() {
        let data = generate_test_data(*size, 0x42);
        
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("sha256", size),
            &data,
            |b, data| {
                b.iter(|| {
                    compute_hash(black_box(data))
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark deduplication
fn bench_deduplication(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("deduplication");
    
    let temp_dir = tempfile::tempdir().unwrap();
    let mut engine = runtime.block_on(async {
        qltp_storage::DeduplicationEngine::new(temp_dir.path()).await.unwrap()
    });
    
    // Create duplicate chunks
    let chunk_data = generate_test_data(65536, 0x42);
    let chunk_hash = compute_hash(&chunk_data);
    let chunk_id = hex::encode(chunk_hash);
    
    group.throughput(Throughput::Bytes(chunk_data.len() as u64));
    
    // First store (new chunk)
    group.bench_function("store_new_chunk", |b| {
        b.iter(|| {
            runtime.block_on(async {
                engine.store_mut().store(black_box(&chunk_id), black_box(&chunk_data)).await.unwrap()
            })
        });
    });
    
    // Store the chunk once
    runtime.block_on(async {
        engine.store_mut().store(&chunk_id, &chunk_data).await.unwrap()
    });
    
    // Subsequent stores (deduplicated)
    group.bench_function("store_duplicate_chunk", |b| {
        b.iter(|| {
            runtime.block_on(async {
                engine.store_mut().store(black_box(&chunk_id), black_box(&chunk_data)).await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark end-to-end pipeline
fn bench_pipeline(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("pipeline");
    
    // Create test file
    let mut temp_file = NamedTempFile::new().unwrap();
    let test_data = generate_test_data(1024 * 1024, 0x42); // 1MB
    temp_file.write_all(&test_data).unwrap();
    temp_file.flush().unwrap();
    
    group.throughput(Throughput::Bytes(test_data.len() as u64));
    
    let temp_dir = tempfile::tempdir().unwrap();
    
    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            runtime.block_on(async {
                // Chunk
                let chunks = qltp_core::chunking::chunk_file(black_box(temp_file.path()), 65536).await.unwrap();
                
                // Store
                let mut store = qltp_storage::ContentStore::new(temp_dir.path()).await.unwrap();
                for chunk in &chunks {
                    let chunk_data = qltp_core::chunking::read_chunk(temp_file.path(), chunk).await.unwrap();
                    
                    // Compress
                    let compressed = compress(&chunk_data, Algorithm::Lz4, CompressionLevel::DEFAULT).unwrap();
                    
                    // Store
                    let chunk_id = chunk.id.to_hex();
                    store.store(&chunk_id, &compressed).await.unwrap();
                }
            })
        });
    });
    
    group.finish();
}

/// Benchmark transport layer performance
fn bench_transport(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("transport");
    
    // Test different data sizes
    for size in [65536, 1048576, 10485760].iter() {  // 64KB, 1MB, 10MB
        group.throughput(Throughput::Bytes(*size as u64));
        
        let test_data = generate_test_data(*size, 0x42);
        
        // Benchmark transport manager creation and backend selection
        group.bench_with_input(
            BenchmarkId::new("backend_selection", size),
            &test_data,
            |b, _data| {
                b.iter(|| {
                    runtime.block_on(async {
                        use qltp_transport::application::{TransportManager, TransportManagerConfig};
                        let manager = TransportManager::new(TransportManagerConfig::default());
                        
                        // Auto-select backend
                        let _ = manager.auto_initialize(None).await;
                        
                        black_box(manager)
                    })
                });
            },
        );
        
        // Benchmark session creation
        group.bench_with_input(
            BenchmarkId::new("session_creation", size),
            &test_data,
            |b, _data| {
                b.iter(|| {
                    runtime.block_on(async {
                        use qltp_transport::application::{TransportManager, TransportManagerConfig};
                        use qltp_transport::domain::SessionConfig;
                        
                        let manager = TransportManager::new(TransportManagerConfig::default());
                        let _ = manager.auto_initialize(None).await;
                        
                        // Create session (will fail without backend, but measures overhead)
                        let _ = manager.create_session(SessionConfig::default()).await;
                        
                        black_box(manager)
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark end-to-end transfer with transport
fn bench_end_to_end_transfer(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("end_to_end_transfer");
    
    // Configure for longer running benchmarks
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));
    
    // Test with 10MB file (representative size)
    let file_size = 10 * 1024 * 1024;
    group.throughput(Throughput::Bytes(file_size as u64));
    
    let mut temp_file = NamedTempFile::new().unwrap();
    let test_data = generate_test_data(file_size, 0x42);
    temp_file.write_all(&test_data).unwrap();
    temp_file.flush().unwrap();
    
    let _temp_dir = tempfile::tempdir().unwrap();
    
    group.bench_function("local_transfer_with_compression", |b| {
        b.iter(|| {
            runtime.block_on(async {
                use qltp_core::{Engine, TransferOptions, TransferMode};
                
                let engine = Engine::new().await.unwrap();
                
                let options = TransferOptions {
                    compression: true,
                    deduplication: true,
                    ..Default::default()
                };
                
                // Local mode transfer (storage only, no network)
                let result = engine.transfer_file_with_mode(
                    black_box(temp_file.path()),
                    "local:/output",
                    options,
                    TransferMode::Local
                ).await.unwrap();
                
                black_box(result)
            })
        });
    });
    
    group.bench_function("local_transfer_no_compression", |b| {
        b.iter(|| {
            runtime.block_on(async {
                use qltp_core::{Engine, TransferOptions, TransferMode};
                
                let engine = Engine::new().await.unwrap();
                
                let options = TransferOptions {
                    compression: false,
                    deduplication: false,
                    ..Default::default()
                };
                
                let result = engine.transfer_file_with_mode(
                    black_box(temp_file.path()),
                    "local:/output",
                    options,
                    TransferMode::Local
                ).await.unwrap();
                
                black_box(result)
            })
        });
    });
    
    group.finish();
}

/// Benchmark transport backend capabilities
fn bench_backend_capabilities(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("backend_capabilities");
    
    group.bench_function("list_available_backends", |b| {
        b.iter(|| {
            use qltp_transport::application::{TransportManager, TransportManagerConfig};
            
            let manager = TransportManager::new(TransportManagerConfig::default());
            let backends = manager.list_available_backends();
            
            black_box(backends)
        });
    });
    
    group.bench_function("backend_selection_with_criteria", |b| {
        b.iter(|| {
            runtime.block_on(async {
                use qltp_transport::application::{TransportManager, TransportManagerConfig, SelectionCriteria};
                
                let manager = TransportManager::new(TransportManagerConfig::default());
                
                let criteria = SelectionCriteria {
                    min_throughput_bps: Some(500_000_000),  // Minimum 500 MB/s
                    ..Default::default()
                };
                
                let selection = manager.select_optimal_backend(&criteria).unwrap();
                
                black_box(selection)
            })
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_compression,
    bench_chunking,
    bench_storage,
    bench_hashing,
    bench_deduplication,
    bench_pipeline,
    bench_transport,
    bench_end_to_end_transfer,
    bench_backend_capabilities
);
criterion_main!(benches);

// Made with Bob
