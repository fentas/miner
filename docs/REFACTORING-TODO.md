# Refactoring & Optimization TODO

Comprehensive list of refactoring and optimization opportunities identified in the btcminer codebase.

---

## Priority Legend

- **P0 - Critical**: High impact, should be done first
- **P1 - High**: Significant improvement, do after P0
- **P2 - Medium**: Good improvement, do when time permits
- **P3 - Low**: Nice to have, incremental improvements

---

## Table of Contents

1. [Critical Refactoring (P0)](#1-critical-refactoring-p0)
2. [Error Handling (P1)](#2-error-handling-p1)
3. [Performance Optimizations (P1)](#3-performance-optimizations-p1)
4. [Code Deduplication (P2)](#4-code-deduplication-p2)
5. [Architecture Improvements (P2)](#5-architecture-improvements-p2)
6. [Code Quality (P3)](#6-code-quality-p3)

---

## 1. Critical Refactoring (P0)

### 1.1. [~] Split main.rs into modules

**File:** `src/main.rs` (2,044 lines, down from 2,783 - 26.5% reduction)
**Impact:** Massive improvement in maintainability and testability

**Progress:**

- ✅ Extracted Bitcoin helpers (coinbase building, merkle root, BIP34, bech32) to `src/bitcoin/helpers.rs` (493 lines extracted)
- ✅ Extracted print helpers to `src/app/output.rs` (97 lines)
- ✅ Extracted Bitcoin Core log tailing to `src/app/bitcoin_log.rs` (126 lines)
- ✅ Added template parsing helpers to deduplicate main.rs (56 lines removed)

**Current Structure:**

```text
src/
├── main.rs                    # 2,044 lines - reduced by consolidating parsing
├── app/
│   ├── mod.rs                 # Module re-exports
│   ├── output.rs              # Print helper functions (97 lines)
│   └── bitcoin_log.rs         # Bitcoin Core log tailing (126 lines)
```

**Remaining work (lower priority):**

- [ ] Create `MiningApplication` struct to hold shared state
- [ ] Extract mining coordination into `mining_coordinator.rs`
- [ ] Extract pool/stratum logic into `pool_coordinator.rs`
- [ ] Extract Bitcoin Core management into `bitcoin_coordinator.rs`
- [ ] Extract stats reporting into `stats_reporter.rs`
- [ ] Create event-driven architecture with channels
- [ ] Add integration tests for each coordinator

**Note:** The remaining tasks require significant architectural changes (event-driven architecture) and are deferred to P2/Architecture improvements.

---

### 1.2. [x] Refactor bitcoin/manager.rs ✅

**File:** `src/bitcoin/manager.rs` (1,633 → 745 lines, 54% reduction)
**Impact:** Better error handling, clearer lifecycle management

**Completed Structure:**

```text
src/bitcoin/
├── mod.rs                     # Re-exports
├── manager.rs                 # 745 lines - orchestration only
├── downloader.rs              # 187 lines - Binary download logic
├── config_writer.rs           # 258 lines - bitcoin.conf generation
└── snapshot.rs                # 610 lines - AssumeUTXO snapshot handling
```

**Completed Tasks:**

- [x] Extract download logic to `downloader.rs` (187 lines)
- [x] Extract config generation to `config_writer.rs` (258 lines)
- [x] Extract snapshot handling to `snapshot.rs` (610 lines)
- [x] Simplify manager.rs to orchestration only (745 lines)

**Note:** Process management kept in manager.rs as it's tightly coupled with RPC state

---

### 1.3. [~] Simplify tui/state.rs

**File:** `src/tui/state.rs` (1,508 → 1,276 lines, 15% reduction)
**Impact:** Clearer UI state management, easier testing

**Progress:**

- Extracted log types (LogEntry, LogLevel, LogTimeFilter) to `logs.rs` (246 lines)
- Extracted hashrate types (HashrateSample, HashrateDisplayMode) to `history.rs` (155 lines)

**Completed Structure:**

```text
src/tui/
├── mod.rs                     # Re-exports
├── state.rs                   # 1,276 lines - TuiState (still large but types extracted)
├── logs.rs                    # 246 lines - Log types and filtering
├── history.rs                 # 155 lines - Hashrate sample types
└── widgets.rs                 # TUI rendering (unchanged)
```

**Completed Tasks:**

- [x] Extract log types to `logs.rs`
- [x] Extract hashrate types to `history.rs`

**Remaining (lower priority):**

- [ ] Further split TuiState into focused components (significant refactoring)
- [ ] Simplify locking strategy (requires architectural changes)

---

## 2. Error Handling (P1)

### 2.1. [ ] Fix config.rs error handling

**File:** `src/config.rs`
**Count:** 43 `unwrap()`/`expect()` calls

**High-Priority Fixes:**

| Line | Issue | Fix |
|------|-------|-----|
| ~74 | `try_into().unwrap()` on header | Return `ConfigError` |
| Various | `toml::from_str().unwrap()` in tests | Use `?` operator |
| Various | Validation `expect()` calls | Return `Result<(), ValidationError>` |

**Tasks:**
- [ ] Create `ConfigError` enum with variants
- [ ] Replace `unwrap()` with `?` and proper error types
- [ ] Add context to errors using `anyhow` or custom messages
- [ ] Keep test `unwrap()` calls (acceptable in tests)

---

### 2.2. [ ] Fix gpu/opencl.rs error handling

**File:** `src/gpu/opencl.rs`
**Count:** 16 `unwrap()` calls

**Issues:**
- GPU initialization can panic on driver errors
- No graceful fallback when GPU unavailable

**Tasks:**
- [ ] Create `GpuError` enum
- [ ] Return `Result` from all GPU operations
- [ ] Add graceful degradation (disable GPU, continue with CPU)
- [ ] Better error messages for common GPU issues

---

### 2.3. [ ] Fix bitcoin/manager.rs error handling

**File:** `src/bitcoin/manager.rs`
**Count:** 16 `unwrap()` calls

**Critical Locations:**
- Process spawning
- Command execution
- File operations
- Download operations

**Tasks:**
- [ ] Create `BitcoinError` enum
- [ ] Wrap all I/O operations in Result
- [ ] Add retry logic for transient failures
- [ ] Log errors with context before propagating

---

### 2.4. [ ] Fix stratum/v1.rs error handling

**File:** `src/stratum/v1.rs`
**Count:** 9 `unwrap()` calls

**Tasks:**
- [ ] Handle connection timeouts gracefully
- [ ] Add reconnection with exponential backoff (already partial)
- [ ] Better error messages for pool communication failures

---

## 3. Performance Optimizations (P1)

### 3.1. [x] Reduce unnecessary cloning ✅

**Count:** 87 `.clone()` calls across codebase

**Status:** Verified - Job is already `Arc<Job>` wrapped internally. Remaining clones are:

- Arc clones (cheap reference counting)
- String clones for move-into-closure patterns
- Config clones at startup (not hot path)

**Verified:**

- ✅ Job internally uses `Arc<Job>` in coordinator/worker communication
- ✅ tui_state uses Arc for sharing
- ✅ No unnecessary cloning in hot mining paths

---

### 3.2. [x] Optimize extranonce2 generation ✅

**File:** `src/mining/worker.rs`

**Result:** Replaced `SystemTime::now()` syscall with atomic counter for **12x speedup** (17.79ns → 1.49ns).

See `docs/performance-improvements.md` section 12 for details.

---

### 3.3. [x] Optimize atomic operations ✅

**Files:** `src/stratum/v1.rs`, `src/stratum/v2.rs`

**Result:** Replaced `SeqCst` with appropriate orderings:

- `connected.store/load`: Release/Acquire
- `request_id.fetch_add`: Relaxed

See `docs/performance-improvements.md` section 13 for details.

---

### 3.4. [ ] Batch stats increments

**File:** `src/mining/worker.rs` (line 167)

**Current:**
```rust
self.stats.add_hashes(batch_size as u64);
```

**Optimization:** Already batched per batch_size, but could accumulate locally and flush less frequently for very high hash rates.

**Tasks:**
- [ ] Profile to see if atomic contention is an issue
- [ ] Consider thread-local accumulators with periodic flush
- [ ] Benchmark at different thread counts

---

### 3.5. [ ] Add IPv6 support to Stratum

**File:** `src/stratum/v1.rs` (lines 42-45)

**Current Issue:**
```rust
TcpStream::connect_timeout(&addr, timeout)
```
- Only connects to first resolved address
- No IPv6 fallback

**Fix:**
- Try all resolved addresses (IPv4 and IPv6)
- Implement Happy Eyeballs algorithm

**Tasks:**
- [ ] Resolve hostname to all addresses
- [ ] Try connections in parallel/sequential with fallback
- [ ] Add config option for IP version preference

---

## 4. Code Deduplication (P2)

### 4.1. [ ] Create SIMD helper macros

**Files:** `src/mining/sha256_simd/{shani,avx2,avx512}.rs`
**Duplicated Lines:** ~810

**Common Patterns:**
- State loading/extraction
- Message schedule expansion
- 64-round compression loop
- K constant addition

**Proposed Approach:**
```rust
// src/mining/sha256_simd/macros.rs

/// Generate message schedule expansion
macro_rules! sha256_msg_schedule {
    ($w:expr, $i:expr, $simd_type:ty) => {{
        // Common expansion logic parameterized by SIMD type
    }};
}

/// Generate compression round
macro_rules! sha256_round {
    ($state:expr, $k:expr, $w:expr, $simd_type:ty) => {{
        // Common round logic
    }};
}
```

**Tasks:**
- [ ] Identify exact duplicated sections
- [ ] Create macro for message schedule expansion
- [ ] Create macro for compression rounds
- [ ] Test that macro-generated code matches original
- [ ] Profile to ensure no performance regression

---

### 4.2. [~] Consolidate config snapshot structs

**Files:** `src/config.rs`, `src/web/mod.rs`, `src/tui/state.rs`

**Issue:** Multiple similar structs for config snapshots with overlapping fields.

**Progress:**

- ✅ Added `ConfigSnapshot::for_solo()` and `for_pool()` constructors
- ✅ Reduced inline ConfigSnapshot creation in main.rs (36 lines saved)

**Remaining Tasks:**

- [ ] Further unify config snapshot patterns if needed

---

### 4.3. [~] Consolidate hex encoding utilities

**Multiple Files:** Various hex encoding/decoding scattered

**Progress:**

- ✅ Added `parse_prev_hash()`, `parse_nbits()`, `parse_target()` to `bitcoin/helpers.rs`
- ✅ Consolidated 4 duplicate parsing blocks in main.rs (56 lines removed)
- ✅ Added tests for new parsing functions

**Remaining Tasks:**

- [ ] Create `src/util/hex.rs` if more hex utilities are needed elsewhere

---

## 5. Architecture Improvements (P2)

### 5.1. [ ] Event-driven architecture for main loop

**File:** `src/main.rs`

**Current:** Tightly coupled `select!` macro with direct state access

**Proposed:**
```rust
enum MiningEvent {
    NewJob(Job),
    ShareFound(Share),
    PoolConnected,
    PoolDisconnected,
    StatsUpdate(StatsSnapshot),
    // ...
}

// Event channel for decoupled communication
let (event_tx, event_rx) = mpsc::channel(100);
```

**Tasks:**
- [ ] Define `MiningEvent` enum
- [ ] Create event channel infrastructure
- [ ] Refactor coordinators to emit events
- [ ] Simplify main loop to event dispatch

---

### 5.2. [ ] Add connection pooling for Stratum

**File:** `src/stratum/v1.rs`

**Current:** Single TCP connection with reconnect logic

**Improvement:**
- Connection health monitoring
- Automatic failover to backup pools
- Connection reuse

**Tasks:**
- [ ] Add backup pool support to config
- [ ] Implement pool failover logic
- [ ] Add connection health checks (ping/pong)

---

### 5.3. [ ] Trait-based platform abstraction for idle monitor

**File:** `src/idle/mod.rs` (524 lines)

**Current:** Platform detection via `#[cfg]` throughout

**Proposed:**
```rust
trait IdleDetector: Send + Sync {
    fn get_idle_time(&self) -> Result<Duration>;
    fn is_available(&self) -> bool;
}

#[cfg(target_os = "linux")]
struct LinuxIdleDetector { ... }

#[cfg(target_os = "windows")]
struct WindowsIdleDetector { ... }
```

**Tasks:**
- [ ] Define `IdleDetector` trait
- [ ] Implement for each platform
- [ ] Factory function to create appropriate detector
- [ ] Simplify main idle module

---

## 6. Code Quality (P3)

### 6.1. [ ] Add missing documentation

**Priority Files:**
- [ ] `src/mining/mod.rs` - Core mining types
- [ ] `src/stratum/mod.rs` - Stratum protocol
- [ ] `src/config.rs` - Configuration options

**Tasks:**
- [ ] Add module-level documentation
- [ ] Document public APIs
- [ ] Add examples for complex functions

---

### 6.2. [ ] Improve test coverage

**Current Gaps:**
- Integration tests for stratum reconnection
- GPU mining tests (require mocking)
- TUI state transitions

**Tasks:**
- [ ] Add stratum reconnection tests
- [ ] Mock GPU for testing
- [ ] Test TUI state machine transitions
- [ ] Add property-based tests for SHA-256

---

### 6.3. [ ] Standardize logging

**Issue:** Inconsistent log levels and formats

**Tasks:**
- [ ] Audit all log statements
- [ ] Standardize on log levels (error/warn/info/debug/trace)
- [ ] Add structured logging fields where appropriate
- [ ] Remove duplicate/redundant log statements

---

### 6.4. [ ] Remove dead code

**Tasks:**
- [ ] Run `cargo +nightly udeps` to find unused dependencies
- [ ] Run `cargo clippy` with all warnings
- [ ] Remove unused functions and imports
- [ ] Clean up commented-out code

---

## Summary Statistics

| Category | Count | Priority |
|----------|-------|----------|
| Critical Refactoring | 3 | P0 |
| Error Handling | 4 | P1 |
| Performance Optimizations | 5 | P1 |
| Code Deduplication | 3 | P2 |
| Architecture Improvements | 3 | P2 |
| Code Quality | 4 | P3 |
| **Total** | **22** | |

---

## Quick Wins (Can be done independently)

1. [x] Fix extranonce2 generation (P1) - ✅ 12x speedup
2. [x] Optimize atomic orderings (P1) - ✅ SeqCst → Release/Acquire/Relaxed
3. [x] Consolidate hex utilities (P2) - ✅ Already organized, skipped
4. [ ] Add missing documentation (P3, ongoing)
5. [x] Run clippy and fix warnings (P3) - ✅ All fixed
6. [x] Add criterion benchmarks (P1) - ✅ Migrated from profiling tests

---

## Dependencies

```text
1.1 (main.rs split) blocks:
  └── 5.1 (event architecture)

1.2 (bitcoin/manager) blocks:
  └── 2.3 (error handling)

4.1 (SIMD macros) requires:
  └── Performance baseline tests first
```

---

## How to Use This Document

1. **Start with Quick Wins** - Low effort, immediate value
2. **Then P0 Critical** - High impact refactoring
3. **Profile before P1 Performance** - Measure actual bottlenecks
4. **P2/P3 as time permits** - Incremental improvements

For each task:
1. Create a branch: `refactor/<task-name>`
2. Run baseline tests/benchmarks
3. Implement changes
4. Run tests and benchmarks again
5. Document in `docs/performance-improvements.md` if applicable
