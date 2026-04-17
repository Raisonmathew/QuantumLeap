# QLTP Bugs & Issues Tracker

**Version**: 1.0  
**Date**: 2026-04-15  
**Status**: Active Tracking

---

## Critical Issues (P0) 🔴

### BUG-001: Transport Layer Not Integrated
**Priority**: P0 - Critical  
**Status**: 🔴 Open  
**Discovered**: 2026-04-15  
**Affects**: All file transfers

**Description**:
The high-performance `qltp-transport` layer (with io_uring backend achieving 8-10 GB/s) is completely disconnected from the file transfer logic. The application currently uses basic TCP from `qltp-network` crate, achieving only 120 MB/s.

**Impact**:
- **67x slower** than potential performance
- Users cannot benefit from io_uring optimization
- No automatic backend selection
- No fallback mechanism

**Root Cause**:
- `Engine` doesn't instantiate `TransportManager`
- `TransferPipeline` hardcoded to use `qltp-network`
- No integration between layers

**Reproduction**:
```bash
# Current behavior: Uses basic TCP (120 MB/s)
qltp transfer large_file.bin remote:/path/

# Expected: Should auto-select io_uring on Linux (8-10 GB/s)
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 3
**Estimated Effort**: 1 week  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

### BUG-002: Duplicate Transport Implementations
**Priority**: P0 - Critical  
**Status**: 🔴 Open  
**Discovered**: 2026-04-15  
**Affects**: Code maintainability, performance

**Description**:
Two separate, incompatible transport implementations exist:
1. `qltp-network` - Currently used, basic TCP only
2. `qltp-transport` - Complete but unused, has io_uring + TCP

**Impact**:
- Code duplication (~1,000 LOC)
- Maintenance burden (2 codebases)
- Confusion for developers
- Wasted development effort

**Root Cause**:
- `qltp-transport` was created as new abstraction layer
- Old `qltp-network` was never migrated
- No deprecation plan executed

**Evidence**:
```
qltp-network/src/connection.rs  (TCP implementation)
qltp-transport/src/adapters/tcp.rs  (TCP implementation)
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 1
**Estimated Effort**: 2 weeks  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

### BUG-003: No Automatic Backend Selection
**Priority**: P0 - Critical  
**Status**: 🔴 Open  
**Discovered**: 2026-04-15  
**Affects**: Performance, user experience

**Description**:
`TransportManagerConfig` has `auto_select_backend: bool` flag, but no implementation of the selection algorithm exists. Users cannot benefit from automatic optimization.

**Impact**:
- Manual backend selection required (not implemented in CLI)
- No fallback if preferred backend unavailable
- Poor user experience
- Suboptimal performance

**Current Behavior**:
```rust
// TransportManagerConfig has the flag
pub struct TransportManagerConfig {
    pub auto_select_backend: bool,  // ✅ Flag exists
    // ...
}

// But no implementation
impl TransportManager {
    pub fn select_backend() {
        // ❌ Not implemented
    }
}
```

**Expected Behavior**:
```rust
// Should automatically select:
// 1. io_uring (Linux 5.1+) - 8-10 GB/s
// 2. QUIC (cross-platform) - 1 GB/s
// 3. TCP (fallback) - 120 MB/s
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 2
**Estimated Effort**: 1 week  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

## High Priority Issues (P1) 🟠

### BUG-004: QUIC Backend Not Implemented
**Priority**: P1 - High  
**Status**: 🟠 Open  
**Discovered**: 2026-04-15  
**Affects**: Cross-platform performance

**Description**:
QUIC backend exists as stub implementation in `qltp-network/src/quic.rs` but is not functional. This prevents high-performance transfers on non-Linux platforms.

**Impact**:
- macOS/Windows limited to 120 MB/s (TCP)
- Cannot achieve 1 GB/s on non-Linux platforms
- Poor cross-platform experience

**Current State**:
```rust
// qltp-network/src/quic.rs
pub async fn connect(&mut self, addr: &str) -> Result<QuicConnection> {
    // Simulate QUIC connection establishment
    // In a real implementation, this would use quinn or quiche
    debug!("Establishing QUIC connection to {}", server_addr);
    // ❌ Stub only - not functional
}
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 4
**Estimated Effort**: 2 weeks  
**Assigned To**: TBD  
**Target Release**: v2.1.0

---

### BUG-005: No CLI Transport Selection
**Priority**: P1 - High  
**Status**: 🟠 Open  
**Discovered**: 2026-04-15  
**Affects**: User control, debugging

**Description**:
CLI has no flags to select or view available transport backends. Users cannot override automatic selection or debug transport issues.

**Impact**:
- No way to force specific backend
- Cannot list available backends
- Difficult to debug transport issues
- Poor developer experience

**Current CLI**:
```bash
# Only basic transfer command exists
qltp transfer file.bin remote:/path/

# Missing:
qltp transfer --transport io_uring file.bin remote:/path/
qltp transfer --list-backends
qltp transfer --transport-info
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 3, Step 3.3
**Estimated Effort**: 1 day  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

### BUG-006: No Fallback Mechanism
**Priority**: P1 - High  
**Status**: 🟠 Open  
**Discovered**: 2026-04-15  
**Affects**: Reliability, user experience

**Description**:
If a backend fails to initialize or encounters errors during transfer, there's no automatic fallback to a more reliable backend.

**Impact**:
- Transfer fails completely if backend unavailable
- No graceful degradation
- Poor reliability on diverse systems

**Expected Behavior**:
```
Attempt 1: io_uring → Failed (not available)
Attempt 2: QUIC → Failed (network issue)
Attempt 3: TCP → Success (fallback)
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 2, Step 2.2
**Estimated Effort**: 2 days  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

## Medium Priority Issues (P2) 🟡

### BUG-007: DPDK Backend Included (Unnecessary)
**Priority**: P2 - Medium  
**Status**: 🟡 Open  
**Discovered**: 2026-04-15  
**Affects**: Code complexity, maintenance

**Description**:
DPDK backend is defined in `TransportType` enum and mentioned in documentation, but:
- Not suitable for cloud deployments
- Requires expensive specialized hardware
- Adds unnecessary complexity
- Decision made to NOT use DPDK

**Impact**:
- Confusing documentation
- Wasted code space
- Misleading users about capabilities

**Current Code**:
```rust
pub enum TransportType {
    IoUring,
    Dpdk,    // ❌ Should be removed
    Quic,
    Tcp,
}
```

**Fix Plan**:
- Remove `TransportType::Dpdk` variant
- Remove DPDK references from documentation
- Update priority calculations
- Add comment explaining decision

**Estimated Effort**: 2 hours  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

### BUG-008: Error Types Not Unified
**Priority**: P2 - Medium  
**Status**: 🟡 Open  
**Discovered**: 2026-04-15  
**Affects**: Error handling, debugging

**Description**:
Separate error types in `qltp-network` and `qltp-transport` make error handling inconsistent and confusing.

**Impact**:
- Inconsistent error messages
- Difficult to handle errors uniformly
- Poor debugging experience

**Current State**:
```rust
// qltp-network/src/error.rs
pub enum NetworkError { ... }

// qltp-transport/src/error.rs
pub enum Error { ... }

// Different error types for similar operations
```

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 1, Step 1.4
**Estimated Effort**: 1 day  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

### BUG-009: No Performance Monitoring
**Priority**: P2 - Medium  
**Status**: 🟡 Open  
**Discovered**: 2026-04-15  
**Affects**: Observability, optimization

**Description**:
No runtime monitoring of transport backend performance. Cannot detect degradation or compare backends.

**Impact**:
- Cannot verify performance claims
- No visibility into backend health
- Difficult to optimize
- No metrics for debugging

**Desired Features**:
- Real-time throughput monitoring
- Backend health checks
- Performance comparison dashboard
- Automatic backend switching on degradation

**Fix Plan**: See [TRANSPORT_INTEGRATION_PLAN.md](./TRANSPORT_INTEGRATION_PLAN.md) Phase 2, Step 2.3
**Estimated Effort**: 2 days  
**Assigned To**: TBD  
**Target Release**: v2.1.0

---

## Low Priority Issues (P3) 🟢

### BUG-010: Missing Platform Detection Tests
**Priority**: P3 - Low  
**Status**: 🟢 Open  
**Discovered**: 2026-04-15  
**Affects**: Cross-platform reliability

**Description**:
Platform detection logic exists but lacks comprehensive tests across different OS versions and configurations.

**Impact**:
- Potential false positives/negatives in backend availability
- Untested edge cases
- Risk of runtime failures

**Fix Plan**: Add comprehensive test suite for platform detection
**Estimated Effort**: 1 day  
**Assigned To**: TBD  
**Target Release**: v2.1.0

---

### BUG-011: Documentation Out of Sync
**Priority**: P3 - Low  
**Status**: 🟢 Open  
**Discovered**: 2026-04-15  
**Affects**: Developer experience

**Description**:
Documentation references both `qltp-network` and `qltp-transport` inconsistently. Some examples use old API.

**Impact**:
- Confusing for new developers
- Outdated examples
- Maintenance burden

**Fix Plan**: Update all documentation after Phase 1 completion
**Estimated Effort**: 1 day  
**Assigned To**: TBD  
**Target Release**: v2.0.0

---

## Resolved Issues ✅

### BUG-012: io_uring Phase 4 Features Missing
**Priority**: P0 - Critical  
**Status**: ✅ Resolved  
**Discovered**: 2026-04-10  
**Resolved**: 2026-04-14

**Description**:
io_uring backend was missing Phase 4 optimizations (SQPOLL, linked operations, buffer selection).

**Resolution**:
- Implemented all Phase 4 features
- Added 26 comprehensive tests
- Achieved 8-10 GB/s throughput
- Documented in [IO_URING_PHASE4_COMPLETE.md](./IO_URING_PHASE4_COMPLETE.md)

**Resolved By**: Bob  
**Release**: v1.5.0

---

## Issue Statistics

### By Priority
- **P0 (Critical)**: 3 open
- **P1 (High)**: 3 open
- **P2 (Medium)**: 3 open
- **P3 (Low)**: 2 open
- **Total Open**: 11
- **Total Resolved**: 1

### By Category
- **Architecture**: 3 issues (BUG-001, BUG-002, BUG-003)
- **Performance**: 2 issues (BUG-004, BUG-009)
- **User Experience**: 2 issues (BUG-005, BUG-006)
- **Code Quality**: 3 issues (BUG-007, BUG-008, BUG-011)
- **Testing**: 1 issue (BUG-010)

### By Target Release
- **v2.0.0**: 8 issues
- **v2.1.0**: 3 issues

---

## Issue Workflow

### States
- 🔴 **Open** - Issue identified, not started
- 🟡 **In Progress** - Work has begun
- 🟢 **In Review** - Fix ready for review
- ✅ **Resolved** - Fix merged and verified
- ❌ **Closed** - Won't fix or duplicate

### Priority Levels
- **P0 (Critical)** - Blocks release, must fix immediately
- **P1 (High)** - Important for release, fix soon
- **P2 (Medium)** - Should fix, can defer if needed
- **P3 (Low)** - Nice to have, fix when time permits

---

## How to Report a Bug

1. **Check existing issues** - Avoid duplicates
2. **Create new entry** in this document
3. **Include**:
   - Clear description
   - Impact assessment
   - Reproduction steps
   - Expected vs actual behavior
   - Relevant code snippets
4. **Assign priority** based on impact
5. **Link to fix plan** if known
6. **Update status** as work progresses

---

## Related Documents

- [Transport Integration Plan](./TRANSPORT_INTEGRATION_PLAN.md) - Fix plan for BUG-001, BUG-002, BUG-003
- [io_uring Complete Summary](./IO_URING_COMPLETE_SUMMARY.md) - Context for performance issues
- [Enterprise Architecture](./ENTERPRISE_ARCHITECTURE.md) - Overall system design

---

**Last Updated**: 2026-04-15  
**Next Review**: Weekly during integration phase  
**Maintained By**: Development Team