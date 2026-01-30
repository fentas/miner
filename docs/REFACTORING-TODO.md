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

**File:** `src/main.rs` (2,290 lines, down from 2,783 - 17.7% reduction)
**Impact:** Massive improvement in maintainability and testability

**Progress:**

- ✅ Extracted Bitcoin helpers (coinbase building, merkle root, BIP34, bech32) to `src/bitcoin/helpers.rs` (493 lines extracted)

**Current State:**
- Single monolithic async block containing all application logic
- Mixed concerns: CLI, config, mining, stratum, TUI, web API
- Deeply nested `select!` macro with 6+ branches
- State mutations scattered throughout

**Proposed Structure:**

```text
src/
├── main.rs                    # ~100 lines - entry point only
├── app/
│   ├── mod.rs                 # MiningApplication struct
│   ├── mining_coordinator.rs  # CPU/GPU mining orchestration
│   ├── pool_coordinator.rs    # Stratum connection management
│   ├── bitcoin_coordinator.rs # Bitcoin Core lifecycle
│   ├── stats_reporter.rs      # Stats collection and reporting
│   └── event_loop.rs          # Main select! loop
```

**Tasks:**
- [ ] Create `MiningApplication` struct to hold shared state
- [ ] Extract mining coordination into `mining_coordinator.rs`
- [ ] Extract pool/stratum logic into `pool_coordinator.rs`
- [ ] Extract Bitcoin Core management into `bitcoin_coordinator.rs`
- [ ] Extract stats reporting into `stats_reporter.rs`
- [ ] Create event-driven architecture with channels
- [ ] Add integration tests for each coordinator

---

### 1.2. [ ] Refactor bitcoin/manager.rs

**File:** `src/bitcoin/manager.rs` (1,633 lines)
**Impact:** Better error handling, clearer lifecycle management

**Current Issues:**
- 16 `unwrap()` calls on process/command operations
- Complex state machine for Bitcoin Core lifecycle
- Mixed concerns: download, verification, process management

**Proposed Structure:**

```text
src/bitcoin/
├── mod.rs                     # Re-exports
├── manager.rs                 # ~400 lines - orchestration only
├── downloader.rs              # Binary download logic
├── process.rs                 # Process lifecycle management
├── config_writer.rs           # bitcoin.conf generation
└── snapshot.rs                # AssumeUTXO snapshot handling
```

**Tasks:**
- [ ] Extract download logic to `downloader.rs`
- [ ] Extract process management to `process.rs`
- [ ] Extract config generation to `config_writer.rs`
- [ ] Extract snapshot handling to `snapshot.rs`
- [ ] Replace all `unwrap()` with proper `Result` handling
- [ ] Add proper error types with `thiserror`

---

### 1.3. [ ] Simplify tui/state.rs

**File:** `src/tui/state.rs` (1,508 lines)
**Impact:** Clearer UI state management, easier testing

**Current Issues:**
- Single monolithic struct managing all UI state
- Tight coupling between log filtering, hashrate history, display modes
- Multiple nested `Arc<RwLock<>>` structures

**Proposed Structure:**

```text
src/tui/
├── state/
│   ├── mod.rs                 # TuiState composition
│   ├── display.rs             # Display mode state
│   ├── logs.rs                # Log buffer and filtering
│   ├── history.rs             # Hashrate history
│   └── mining.rs              # Mining stats state
```

**Tasks:**
- [ ] Split into focused state modules
- [ ] Use composition instead of one large struct
- [ ] Simplify locking strategy (reduce nesting)
- [ ] Add unit tests for each state component

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

### 3.1. [ ] Reduce unnecessary cloning

**Count:** 87 `.clone()` calls across codebase

**High-Impact Locations:**

| File | Line | Issue | Fix |
|------|------|-------|-----|
| main.rs | ~1063 | `job.clone()` in pool mining | Use `Arc<Job>` |
| worker.rs | Various | Job struct cloning | Already uses Arc (verify) |
| tui/state.rs | Multiple | Stats snapshot cloning | Use references where possible |

**Tasks:**
- [ ] Audit all `.clone()` calls in hot paths
- [ ] Replace Job cloning with Arc where not already done
- [ ] Use `Cow<'_, str>` for string parameters
- [ ] Profile to measure actual impact

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

### 4.2. [ ] Consolidate config snapshot structs

**Files:** `src/config.rs`, `src/web/mod.rs`, `src/tui/state.rs`

**Issue:** Multiple similar structs for config snapshots with overlapping fields.

**Tasks:**
- [ ] Create unified `ConfigSnapshot` in config.rs
- [ ] Re-export and use in web and TUI modules
- [ ] Remove duplicate struct definitions

---

### 4.3. [ ] Consolidate hex encoding utilities

**Multiple Files:** Various hex encoding/decoding scattered

**Tasks:**
- [ ] Create `src/util/hex.rs` with common functions
- [ ] Consolidate hash reversal logic
- [ ] Add proper error handling for hex operations

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
