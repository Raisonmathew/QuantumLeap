# QLTP Transport Integration & Refactoring Plan

**Version**: 1.0  
**Date**: 2026-04-15  
**Status**: Planning Phase  
**Priority**: 🔥 CRITICAL

---

## Executive Summary

This document outlines the plan to integrate the high-performance `qltp-transport` layer with the existing file transfer logic, and refactor/consolidate the `qltp-network` crate to eliminate duplication and create a unified transport architecture.

### Key Decisions

✅ **Use `qltp-transport` as the primary transport layer**  
✅ **Migrate protocol/transfer logic from `qltp-network` to `qltp-transport`**  
✅ **Remove DPDK backend** (not suitable for cloud deployments)  
✅ **Focus on io_uring (Linux) + QUIC (cross-platform) + TCP (fallback)**  
❌ **Deprecate `qltp-network` crate** (merge into `qltp-transport`)

---

## Current Architecture Analysis

### qltp-network (10 files, ~3,000 LOC)

**Purpose**: High-level networking and file transfer protocol

| Module | Lines | Purpose | Status |
|--------|-------|---------|--------|
| `codec.rs` | ~200 | Message encoding/decoding | ✅ Keep - Move to transport |
| `connection.rs` | ~400 | TCP connection management | ⚠️ Duplicate - Use transport backends |
| `error.rs` | ~100 | Network errors | ✅ Keep - Merge with transport errors |
| `parallel.rs` | ~300 | Parallel stream transfers | ✅ Keep - Move to transport |
| `protocol.rs` | ~500 | QLTP protocol messages | ✅ Keep - Move to transport |
| `quic.rs` | ~200 | QUIC stub implementation | ❌ Remove - Implement properly |
| `resume.rs` | ~300 | Transfer resume logic | ✅ Keep - Move to transport |
| `tls.rs` | ~200 | TLS encryption | ✅ Keep - Move to transport |
| `transfer.rs` | ~800 | File transfer client/server | ✅ Keep - Refactor to use backends |
| `lib.rs` | ~100 | Module exports | ❌ Remove - Merge into transport |

**Total**: ~3,100 LOC to migrate/refactor

### qltp-transport (9 files, ~1,500 LOC)

**Purpose**: Low-level transport backend abstraction

| Module | Lines | Purpose | Status |
|--------|-------|---------|--------|
| `adapters/tcp.rs` | ~350 | TCP backend | ✅ Complete |
| `adapters/io_uring.rs` | ~1,145 | io_uring backend (4 phases) | ✅ Complete |
| `application/transport_manager.rs` | ~250 | Backend orchestration | ⚠️ Needs integration |
| `domain/transport_type.rs` | ~150 | Backend types | ✅ Complete |
| `domain/session.rs` | ~200 | Session management | ✅ Complete |
| `ports/transport_backend.rs` | ~100 | Backend interface | ✅ Complete |

**Total**: ~2,195 LOC (well-structured, clean architecture)

---

## Integration Gaps & Issues

### 🔴 Critical Issues

1. **No Connection Between Layers**
   - `Engine` → `TransferPipeline` → `qltp-network` (hardcoded TCP)
   - `TransportManager` exists but is never instantiated
   - io_uring backend is complete but unused

2. **Duplicate Implementations**
   - TCP: Both `qltp-network/connection.rs` and `qltp-transport/adapters/tcp.rs`
   - QUIC: Stub in `qltp-network`, planned in `qltp-transport`
   - Error types: Separate in both crates

3. **Missing Auto-Selection**
   - No logic to choose optimal backend
   - No fallback mechanism
   - No runtime switching

4. **Protocol Layer Confusion**
   - High-level protocol (messages, codec) in `qltp-network`
   - Low-level transport (send/recv) in `qltp-transport`
   - No clear separation of concerns

---

## Refactoring Strategy

### Phase 1: Merge & Consolidate (Week 1-2)

**Goal**: Merge `qltp-network` into `qltp-transport` to create a unified transport layer

#### Step 1.1: Move Protocol Layer (3 days)
- [ ] Move `protocol.rs` → `qltp-transport/src/protocol/`
- [ ] Move `codec.rs` → `qltp-transport/src/protocol/codec.rs`
- [ ] Update imports across codebase
- [ ] Run tests to verify no breakage

**Files to create**:
```
qltp-transport/src/protocol/
├── mod.rs           (new)
├── messages.rs      (from protocol.rs)
├── codec.rs         (from codec.rs)
└── types.rs         (new - common types)
```

#### Step 1.2: Move Transfer Logic (3 days)
- [ ] Move `transfer.rs` → `qltp-transport/src/application/transfer.rs`
- [ ] Refactor `TransferClient`/`TransferServer` to use `TransportManager`
- [ ] Update to use backend abstraction instead of direct TCP
- [ ] Add backend selection logic

**Files to create**:
```
qltp-transport/src/application/
├── mod.rs
├── transport_manager.rs  (existing)
├── transfer_client.rs    (new - from transfer.rs)
└── transfer_server.rs    (new - from transfer.rs)
```

#### Step 1.3: Move Supporting Features (2 days)
- [ ] Move `resume.rs` → `qltp-transport/src/features/resume.rs`
- [ ] Move `parallel.rs` → `qltp-transport/src/features/parallel.rs`
- [ ] Move `tls.rs` → `qltp-transport/src/features/tls.rs`
- [ ] Update feature flags in `Cargo.toml`

**Files to create**:
```
qltp-transport/src/features/
├── mod.rs           (new)
├── resume.rs        (from qltp-network)
├── parallel.rs      (from qltp-network)
└── tls.rs           (from qltp-network)
```

#### Step 1.4: Merge Error Types (1 day)
- [ ] Merge `qltp-network/error.rs` into `qltp-transport/error.rs`
- [ ] Create unified error hierarchy
- [ ] Update all error handling

#### Step 1.5: Remove qltp-network Crate (1 day)
- [ ] Update all dependencies to use `qltp-transport`
- [ ] Remove `qltp-network` from workspace
- [ ] Update documentation
- [ ] Archive old code (git tag: `pre-transport-merge`)

---

### Phase 2: Implement Auto-Selection (Week 3)

**Goal**: Add intelligent backend selection and fallback logic

#### Step 2.1: Backend Selection Algorithm (2 days)
- [ ] Implement `TransportManager::select_optimal_backend()`
- [ ] Add platform detection (Linux kernel version, etc.)
- [ ] Add capability checking (io_uring available, etc.)
- [ ] Implement priority-based selection

**Algorithm**:
```rust
fn select_optimal_backend() -> TransportType {
    let available = TransportType::available_backends();
    
    // Priority order (no DPDK):
    // 1. io_uring (Linux 5.1+, 8-10 GB/s)
    // 2. QUIC (cross-platform, 1 GB/s)
    // 3. TCP (universal fallback, 120 MB/s)
    
    for backend in [IoUring, Quic, Tcp] {
        if available.contains(&backend) {
            return backend;
        }
    }
    
    Tcp // Always available
}
```

#### Step 2.2: Fallback Mechanism (2 days)
- [ ] Add `TransportManager::try_with_fallback()`
- [ ] Implement automatic fallback on backend failure
- [ ] Add retry logic with exponential backoff
- [ ] Log backend selection decisions

#### Step 2.3: Runtime Monitoring (2 days)
- [ ] Add performance metrics per backend
- [ ] Implement health checks
- [ ] Add ability to switch backends mid-transfer (optional)
- [ ] Create dashboard/logging for backend status

---

### Phase 3: Engine Integration (Week 4)

**Goal**: Connect `TransportManager` to `Engine` and `TransferPipeline`

#### Step 3.1: Update Engine (2 days)
- [ ] Add `TransportManager` field to `Engine`
- [ ] Initialize `TransportManager` in `Engine::new()`
- [ ] Add `Engine::with_transport_config()` constructor
- [ ] Update `Engine::transfer_file()` to use transport sessions

**Changes to `qltp-core/src/lib.rs`**:
```rust
pub struct Engine {
    config: EngineConfig,
    pipeline: Arc<TransferPipeline>,
    transport: Arc<TransportManager>,  // NEW
    storage_dir: PathBuf,
}

impl Engine {
    pub async fn new() -> Result<Self> {
        let transport_config = TransportManagerConfig {
            auto_select_backend: true,
            preferred_transport: None,  // Auto-select
            ..Default::default()
        };
        
        let transport = TransportManager::new(transport_config);
        
        // Auto-select and initialize backend
        let backend = transport.select_optimal_backend().await?;
        transport.initialize(backend).await?;
        
        // ... rest of initialization
    }
}
```

#### Step 3.2: Update TransferPipeline (2 days)
- [ ] Add `TransportManager` parameter to `TransferPipeline::new()`
- [ ] Update `execute()` to create transport sessions
- [ ] Replace direct file I/O with transport send/receive
- [ ] Add progress reporting via transport stats

#### Step 3.3: Update CLI (1 day)
- [ ] Add `--transport` flag (auto|io_uring|quic|tcp)
- [ ] Add `--no-auto-select` flag
- [ ] Display selected backend in output
- [ ] Add `--list-backends` command

**CLI changes**:
```bash
# Auto-select (default)
qltp transfer file.bin remote:/path/

# Force specific backend
qltp transfer --transport io_uring file.bin remote:/path/

# List available backends
qltp list-backends
```

---

### Phase 4: QUIC Backend Implementation (Week 5-6)

**Goal**: Implement proper QUIC backend for cross-platform support

#### Step 4.1: QUIC Backend Structure (3 days)
- [ ] Create `qltp-transport/src/adapters/quic.rs`
- [ ] Implement `TransportBackend` trait for QUIC
- [ ] Add quinn or quiche dependency
- [ ] Implement basic send/receive

#### Step 4.2: QUIC Features (4 days)
- [ ] Implement 0-RTT connection establishment
- [ ] Add stream multiplexing
- [ ] Implement congestion control
- [ ] Add built-in TLS 1.3 support

#### Step 4.3: QUIC Testing (3 days)
- [ ] Unit tests for QUIC backend
- [ ] Integration tests with Engine
- [ ] Performance benchmarks
- [ ] Cross-platform testing (Linux, macOS, Windows)

---

### Phase 5: Testing & Validation (Week 7)

**Goal**: Comprehensive testing of integrated system

#### Step 5.1: Unit Tests (2 days)
- [ ] Test backend selection logic
- [ ] Test fallback mechanism
- [ ] Test each backend independently
- [ ] Test error handling

#### Step 5.2: Integration Tests (2 days)
- [ ] Test Engine → TransportManager → Backend flow
- [ ] Test file transfers with each backend
- [ ] Test resume functionality
- [ ] Test parallel transfers

#### Step 5.3: Performance Benchmarks (2 days)
- [ ] Benchmark each backend (io_uring, QUIC, TCP)
- [ ] Compare against old implementation
- [ ] Measure CPU usage and memory
- [ ] Document performance improvements

#### Step 5.4: End-to-End Tests (1 day)
- [ ] Test CLI with all backends
- [ ] Test large file transfers (>1GB)
- [ ] Test network interruption scenarios
- [ ] Test cross-platform compatibility

---

## File Structure After Refactoring

```
qltp-project/
├── crates/
│   ├── qltp-transport/              (UNIFIED TRANSPORT LAYER)
│   │   ├── src/
│   │   │   ├── adapters/            (Backend implementations)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── tcp.rs           (TCP backend)
│   │   │   │   ├── io_uring.rs      (io_uring backend - Linux)
│   │   │   │   └── quic.rs          (QUIC backend - NEW)
│   │   │   ├── application/         (High-level services)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── transport_manager.rs
│   │   │   │   ├── transfer_client.rs   (NEW - from qltp-network)
│   │   │   │   └── transfer_server.rs   (NEW - from qltp-network)
│   │   │   ├── domain/              (Domain models)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── transport_type.rs
│   │   │   │   ├── session.rs
│   │   │   │   └── ...
│   │   │   ├── protocol/            (NEW - from qltp-network)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── messages.rs
│   │   │   │   ├── codec.rs
│   │   │   │   └── types.rs
│   │   │   ├── features/            (NEW - optional features)
│   │   │   │   ├── mod.rs
│   │   │   │   ├── resume.rs        (from qltp-network)
│   │   │   │   ├── parallel.rs      (from qltp-network)
│   │   │   │   └── tls.rs           (from qltp-network)
│   │   │   ├── ports/               (Interfaces)
│   │   │   │   ├── mod.rs
│   │   │   │   └── transport_backend.rs
│   │   │   ├── error.rs             (Unified errors)
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── qltp-core/                   (File transfer engine)
│   ├── qltp-storage/                (Storage & dedup)
│   ├── qltp-auth/                   (Authentication)
│   └── qltp-licensing/              (Licensing)
└── apps/
    └── cli/                         (Command-line interface)
```

---

## Migration Checklist

### Pre-Migration
- [x] Analyze current architecture
- [x] Identify all dependencies on `qltp-network`
- [x] Create backup branch: `git checkout -b pre-transport-merge`
- [ ] Document all public APIs that will change
- [ ] Notify team of upcoming changes

### Phase 1: Merge (Week 1-2)
- [ ] Move protocol layer
- [ ] Move transfer logic
- [ ] Move supporting features
- [ ] Merge error types
- [ ] Remove `qltp-network` crate
- [ ] Update all imports
- [ ] Run full test suite

### Phase 2: Auto-Selection (Week 3)
- [ ] Implement selection algorithm
- [ ] Add fallback mechanism
- [ ] Add runtime monitoring
- [ ] Test on multiple platforms

### Phase 3: Integration (Week 4)
- [ ] Update Engine
- [ ] Update TransferPipeline
- [ ] Update CLI
- [ ] Update documentation

### Phase 4: QUIC (Week 5-6)
- [ ] Implement QUIC backend
- [ ] Add QUIC features
- [ ] Test QUIC backend
- [ ] Benchmark QUIC performance

### Phase 5: Testing (Week 7)
- [ ] Unit tests
- [ ] Integration tests
- [ ] Performance benchmarks
- [ ] End-to-end tests

### Post-Migration
- [ ] Update all documentation
- [ ] Create migration guide for users
- [ ] Tag release: `v2.0.0-transport-unified`
- [ ] Archive old `qltp-network` code
- [ ] Celebrate! 🎉

---

## Risk Assessment

### High Risk
1. **Breaking Changes**: Public API will change significantly
   - **Mitigation**: Provide compatibility layer for 1 release cycle
   - **Timeline**: 2 weeks for compatibility layer

2. **Performance Regression**: Integration might slow down transfers
   - **Mitigation**: Extensive benchmarking before/after
   - **Rollback Plan**: Keep old code in separate branch

### Medium Risk
3. **Platform Compatibility**: io_uring only works on Linux 5.1+
   - **Mitigation**: Automatic fallback to QUIC/TCP
   - **Testing**: Test on Linux, macOS, Windows

4. **QUIC Implementation Complexity**: QUIC is complex to implement correctly
   - **Mitigation**: Use battle-tested library (quinn)
   - **Timeline**: 2 weeks for QUIC backend

### Low Risk
5. **Test Coverage**: Might miss edge cases during refactoring
   - **Mitigation**: Maintain >80% test coverage
   - **Review**: Code review for all changes

---

## Success Metrics

### Performance
- ✅ **8-10x faster** transfers on Linux (io_uring vs old TCP)
- ✅ **8-10x faster** transfers on other platforms (QUIC vs old TCP)
- ✅ **<15% CPU** overhead (vs 80% before)
- ✅ **1GB in <0.25s** on 10GbE network

### Code Quality
- ✅ **Single transport crate** (vs 2 separate crates)
- ✅ **<5,000 LOC** total (vs ~5,300 before)
- ✅ **>80% test coverage**
- ✅ **Zero compiler warnings**

### User Experience
- ✅ **Automatic backend selection** (no user config needed)
- ✅ **Graceful fallback** on unsupported platforms
- ✅ **Clear error messages** for transport issues
- ✅ **CLI flags** for manual backend selection

---

## Timeline Summary

| Phase | Duration | Effort | Priority |
|-------|----------|--------|----------|
| **Phase 1: Merge & Consolidate** | 2 weeks | 10 days | 🔥 Critical |
| **Phase 2: Auto-Selection** | 1 week | 5 days | 🔥 Critical |
| **Phase 3: Engine Integration** | 1 week | 4 days | 🔥 Critical |
| **Phase 4: QUIC Backend** | 2 weeks | 10 days | ⚠️ High |
| **Phase 5: Testing & Validation** | 1 week | 5 days | ⚠️ High |
| **Total** | **7 weeks** | **34 days** | |

**Start Date**: 2026-04-15  
**Target Completion**: 2026-06-03  
**Buffer**: 1 week for unexpected issues

---

## Dependencies

### External Libraries
- `tokio` - Async runtime (already used)
- `quinn` or `quiche` - QUIC implementation (NEW)
- `io-uring` - io_uring bindings (already used)
- `tokio-rustls` - TLS support (already used)

### Internal Crates
- `qltp-core` - Will depend on new `qltp-transport`
- `qltp-auth` - No changes needed
- `qltp-storage` - No changes needed
- `qltp-licensing` - No changes needed

---

## Open Questions

1. **QUIC Library Choice**: quinn vs quiche?
   - **Recommendation**: quinn (better Rust integration, active development)

2. **Backward Compatibility**: Support old protocol?
   - **Recommendation**: No - clean break with v2.0.0

3. **Feature Flags**: Keep TLS optional?
   - **Recommendation**: Yes - keep `tls` feature flag

4. **DPDK Removal**: Remove all DPDK code?
   - **Recommendation**: Yes - not suitable for cloud, adds complexity

---

## Next Steps

1. **Review this plan** with team
2. **Create GitHub issues** for each phase
3. **Set up project board** for tracking
4. **Start Phase 1** (Merge & Consolidate)
5. **Weekly progress reviews**

---

## References

- [io_uring Implementation](./IO_URING_COMPLETE_SUMMARY.md)
- [Enterprise Architecture](./ENTERPRISE_ARCHITECTURE.md)
- [DPDK Analysis](./DPDK_CLOUD_ANALYSIS.md) (decision: not using)
- [Original Transport Design](./TRANSPORT_ABSTRACTION_DESIGN.md)

---

**Document Status**: ✅ Ready for Review  
**Last Updated**: 2026-04-15  
**Next Review**: After Phase 1 completion