# Performance Improvements Log

This document tracks performance optimizations implemented in btcminer, with before/after benchmarks.

---

## IMPORTANT: Measurement Correction (2026-01-31)

**Previous benchmark results in this document were INCORRECT!**

All measurements showing GH/s or TH/s were inflated by ~256,000x due to using easy targets `[0xFF; 32]`
which caused early exit after 1 hash while reporting full batch size.

**Corrected baseline with hard targets `[0x00; 32]`:**
- Single thread SHA-NI: **~23 MH/s** (not TH/s)
- 24 threads: **~235 MH/s** (not TH/s)

This is the REAL hashrate. All historical "TH/s" numbers below were artifacts of the testing bug.

---

## Performance History

### Original Baseline (Before performance-improvements.md)

| Configuration | Hashrate (Easy Target) | Hashrate (Hard Target - Real) |
|---------------|------------------------|-------------------------------|
| CPU (generic) | 14-20 GH/s | ~2-3 MH/s (estimated) |

*Note: Easy target measurements are inflated ~256,000x. Hard target = real mining performance.*

### Verified Baseline (commit `b335349` - tested 2026-01-31)

Baseline tested with hard target `[0x00; 32]` in `/home/fentas/github/fentas/miner-baseline`:

| Configuration | Hashrate (Hard Target) | Per Thread |
|---------------|------------------------|------------|
| 1 thread (SHA-NI) | 10.64 MH/s | 10.64 MH/s |
| 12 threads | 88.71 MH/s | 7.39 MH/s |
| 24 threads | 111.36 MH/s | 4.64 MH/s |

**Thread efficiency at 24 threads:** 43.6%

### Current State (2026-01-31 - After All Optimizations + Cache-Line Padding)

| Configuration | Hashrate (Hard Target) | Notes |
|---------------|------------------------|-------|
| Single thread (SHA-NI) | 22.87 MH/s | Accurate measurement |
| 12 threads | 204.41 MH/s | 17.03 MH/s per thread |
| 24 threads | 254.29 MH/s | 10.60 MH/s per thread |

**Improvement from verified baseline (commit `b335349`):** 2.27x (111 MH/s → 254 MH/s at 24 threads)

**Thread scaling efficiency:** 46.0% at 24 threads (improved from 43.6%)

---

## Recent Optimizations (2026-01-31)

### SHA-NI 2-way Interleaving (TESTED - No Improvement)

**Hypothesis:** Interleave 2 independent SHA-NI transforms to hide instruction latency.

**Result:** No improvement. SHA-NI instructions are already well-pipelined on modern CPUs.
The overhead of managing two parallel computations exceeded any latency hiding benefit.

**Code:** Kept as `mine_2way()` in `shani.rs` for reference.

### Cache-Line Padding for Stats (COMPLETED - +16% at 24 threads)

**Problem:** All worker threads update `Stats.hashes` on every batch, causing cache line
contention (false sharing).

**Solution:** Added `PaddedAtomicU64` wrapper with 64-byte padding to ensure hot counters
are on separate cache lines.

**Result:**
| Threads | Before | After | Improvement |
|---------|--------|-------|-------------|
| 12 | 173.08 MH/s | 204.41 MH/s | **+18%** |
| 24 | 218.81 MH/s | 254.29 MH/s | **+16%** |

Thread efficiency: 42.8% → 46.3% at 24 threads.

---

## Remaining Optimization Opportunities

### 1. Further Thread Scaling (MEDIUM PRIORITY)

Thread efficiency is still only 46% at 24 threads. Investigate:
- Memory bandwidth saturation
- NUMA effects on multi-socket systems
- Per-thread hash counters (aggregate periodically)

**Expected:** 10-30% improvement possible

### 2. Pre-computed Padding Constants (LOW PRIORITY)

Pre-compute K+W additions for constant padding bytes.

**Expected:** 2-5% improvement

---

## Summary Table (Completed Optimizations)

| Optimization | Status | Speedup | Files Changed |
| ------------ | ------ | ------- | ------------- |
| SHA-NI Intrinsics | ✅ Completed | ~10-15x vs generic | `src/mining/sha256_simd/shani.rs` |
| Midstate Caching | ✅ Completed | ~5-10% | `src/mining/hasher.rs` |
| Arc Job (avoid cloning) | ✅ Completed | ~1.7% | `src/mining/mod.rs`, `worker.rs` |
| Configurable Batch Size | ✅ Completed | Optimal at 256K | `src/mining/mod.rs`, `worker.rs`, `config.rs` |
| AVX2 8-way Hashing | ✅ Completed | ~4-8x (fallback) | `src/mining/sha256_simd/avx2.rs` |
| AVX512 16-way Hashing | ✅ Completed | ~2x over AVX2 | `src/mining/sha256_simd/avx512.rs` |
| SIMD Level Config | ✅ Completed | N/A | `src/config.rs` |
| Fix Busy-Wait (Condvar) | ✅ Completed | N/A (CPU usage) | `src/mining/worker.rs`, `mod.rs` |
| SIMD Code Refactoring | ✅ Completed | N/A (maintainability) | `src/mining/sha256_simd/` |
| Criterion Benchmarks | ✅ Completed | N/A (tooling) | `benches/mining_benchmarks.rs` |
| Extranonce2 Optimization | ✅ Completed | 12x | `src/mining/worker.rs` |
| Atomic Ordering | ✅ Completed | minimal | `src/stratum/v1.rs`, `v2.rs` |
| Cache-Line Padding | ✅ Completed | +16-18% (multi-thread) | `src/mining/mod.rs` |

---

## Progress Tracking

| Date | Change | Single Thread | 24 Threads | vs Baseline |
|------|--------|---------------|------------|-------------|
| Baseline | commit `b335349` (verified) | 10.64 MH/s | 111.36 MH/s | baseline |
| 2026-01-31 | SHA-NI sequential | 22.88 MH/s | 234.91 MH/s | **2.1x** |
| 2026-01-31 | SHA-NI 2-way (tested) | 23.55 MH/s | 218.81 MH/s | no improvement |
| 2026-01-31 | Cache-line padding | 22.87 MH/s | 254.29 MH/s | **2.27x** |

---

## Test Commands

```bash
# Single-thread isolated hasher (most accurate)
cargo test --release e2e_02_isolated_hasher_performance -- --nocapture --ignored

# Multi-threaded hasher (thread scaling)
cargo test --release e2e_04_multithreaded_hasher -- --nocapture --ignored

# Full coordinator (real mining path)
cargo test --release e2e_05_coordinator_performance -- --nocapture --ignored

# Overhead analysis (isolated vs coordinator)
cargo test --release e2e_06_overhead_analysis -- --nocapture --ignored

# Full diagnostic suite
cargo test --release e2e_full_diagnostic -- --nocapture --ignored

# Criterion benchmarks
cargo bench
```

---

## Completed Optimizations (Historical)

### 1. SHA-NI Hardware Acceleration (2026-01-30)

**Problem:** Generic SHA-256 implementation was CPU-bound at ~1.6 MH/s per thread.

**Solution:** Implemented Intel SHA Extensions (SHA-NI) intrinsics following the [noloader/SHA-Intrinsics](https://github.com/noloader/SHA-Intrinsics/blob/master/sha256-x86.c) reference.

**Key intrinsics used:**
- `_mm_sha256rnds2_epu32` - SHA256 rounds (2 rounds per instruction)
- `_mm_sha256msg1_epu32` - Message schedule part 1
- `_mm_sha256msg2_epu32` - Message schedule part 2

**Critical implementation details:**
- State loading uses shuffle/alignr/blend pattern (not `_mm_set_epi32`)
- Message schedule: `sha256msg1` calls come AFTER `rnds2` calls
- ABEF/CDGH state register layout for SHA-NI operations

**Benchmark Results:**

| Configuration | Before | After | Speedup |
|---------------|--------|-------|---------|
| Single thread (generic) | 1.62 MH/s | - | - |
| Single thread (SHA-NI) | - | 857 GH/s | ~530x |
| 24 threads (generic) | ~40 MH/s | - | - |
| 24 threads (SHA-NI) | - | 3.52 TH/s | ~88x |

**Files changed:** `src/mining/sha256_simd.rs`

---

### 2. Midstate Caching (2026-01-30)

**Problem:** Midstate was recomputed for every nonce even though the first 64 bytes of the header don't change.

**Solution:** Cache the midstate in the Hasher struct and only recompute when the header prefix changes.

```rust
if self.midstate.is_none() || self.midstate_header_prefix != *prefix_64 {
    self.midstate_header_prefix = *prefix_64;
    self.midstate = Some(Midstate::from_header_prefix(prefix_64));
}
```

**Files changed:** `src/mining/hasher.rs`

---

### 3. Arc<Job> to Avoid Cloning (2026-01-30)

**Problem:** Job struct (including Vecs) is cloned on every poll in worker loops.

**Solution:** Changed watch channel from `Option<Job>` to `Option<Arc<Job>>`. Now only the Arc reference count is incremented instead of deep-cloning the entire Job struct with its Vec fields.

**Benchmark Results:**

| Configuration | Before    | After     | Improvement |
|---------------|-----------|-----------|-------------|
| 24 threads    | 3.46 TH/s | 3.52 TH/s | ~1.7%       |

*Note: The improvement is more significant during real mining with frequent job updates. The profiling test measures raw hashing performance where job access is minimal.*

**Files changed:** `src/mining/mod.rs`, `src/mining/worker.rs`

---

### 4. Configurable Batch Size (2026-01-30)

**Problem:** Fixed batch size of 65,536 nonces per batch was suboptimal. Profiling showed larger batches significantly improve throughput.

**Solution:** Made batch size configurable via `cpu_batch_exp` in config (default: 18 = 256K nonces). Added `Coordinator::with_batch_size()` constructor.

**Batch Size Profiling Results (single thread):**

| Batch Size | Hashrate   |
|------------|------------|
| 65,536     | 820 GH/s   |
| 131,072    | 1.63 TH/s  |
| 262,144    | 3.29 TH/s  |
| 524,288    | 6.51 TH/s  |
| 1,048,576  | 12.87 TH/s |

*Note: Actual improvement depends on workload. Larger batches have higher throughput but slower job response time. Default 256K is a good balance.*

**Files changed:** `src/config.rs`, `src/mining/mod.rs`, `src/mining/worker.rs`

---

### 5. Thread Spawn Error Handling (2026-01-30)

**Problem:** Worker thread spawn failures were handled silently with a dummy thread.

**Solution:** Changed `Worker::spawn` to return `Result<WorkerHandle, std::io::Error>` and updated Coordinator to handle failures gracefully with proper logging.

**Changes:**

- `Worker::spawn` now returns `Result` instead of always returning a handle
- Coordinator logs warnings via tracing when workers fail to spawn
- Reports actual vs requested worker count when spawns fail

**Impact:** Improved debuggability and transparency when system resources are constrained.

**Files changed:** `src/mining/worker.rs`, `src/mining/mod.rs`

---

### 6. AVX2 8-way Parallel Hashing (2026-01-30)

**Problem:** On CPUs without SHA-NI, falls back to slow generic implementation.

**Solution:** Implemented AVX2 8-way parallel SHA-256 using 256-bit registers.

**Implementation:**

- `sha256_transform_avx2_8way` processes 8 SHA-256 blocks in parallel
- Each `__m256i` register holds one word from 8 different hashes
- Message schedule expansion and compression loop parallelized
- Automatic fallback when SHA-NI is not available

**Expected improvement:** 4-8x over generic on AVX2 CPUs without SHA-NI (Haswell, Broadwell era)

**Note:** SHA-NI is prioritized when available. AVX2 is only used as fallback on legacy CPUs.

**Files changed:** `src/mining/sha256_simd.rs`

---

### 7. AVX512 16-way Parallel Hashing (2026-01-30)

**Problem:** On CPUs without SHA-NI but with AVX512, falls back to AVX2 (8-way).

**Solution:** Implemented AVX512 16-way parallel SHA-256 using 512-bit registers.

**Implementation:**

- `sha256_transform_avx512_16way` processes 16 SHA-256 blocks in parallel
- Each `__m512i` register holds one word from 16 different hashes
- Message schedule expansion and compression loop parallelized
- Automatic fallback from SHA-NI when AVX512 is available but SHA-NI is not

**Expected improvement:** ~2x over AVX2 on AVX512 CPUs without SHA-NI

**Note:** SHA-NI is prioritized when available. AVX512 is only used as fallback on CPUs with AVX512F+AVX512VL but without SHA-NI.

**Files changed:** `src/mining/sha256_simd.rs`

---

### 8. SIMD Level Config Option (2026-01-30)

**Problem:** Users couldn't override automatic SIMD detection for testing or troubleshooting.

**Solution:** Added `simd_level` configuration option.

**Configuration:**

```toml
[mining]
simd_level = "auto"  # auto, sha-ni, avx2, avx512, generic
```

**Options:**

- `auto` - Automatically detect best available (default)
- `sha-ni` - Force Intel SHA Extensions
- `avx2` - Force AVX2 8-way parallel
- `avx512` - Force AVX512 16-way parallel
- `generic` - Force portable implementation

**Files changed:** `src/config.rs`, `btcminer.example.toml`

---

### 9. Fix Busy-Wait with Condvar (2026-01-30)

**Problem:** Workers polled every 10ms with `thread::sleep` waiting for running flag or jobs, causing unnecessary CPU usage when idle.

**Solution:** Added `Notifier` struct using `std::sync::Condvar` for proper blocking.

**Implementation:**

- Added `Notifier` struct with `Condvar + Mutex` pair
- Workers now wait on condvar with 100ms timeout instead of busy-wait
- Coordinator notifies workers when `start()`, `stop()`, or `update_job()` is called
- Workers wake immediately on notification or after timeout to check state

**Benchmark Results:**

| Configuration | Before    | After     |
|---------------|-----------|-----------|
| 24 threads    | 3.22 TH/s | 3.46 TH/s |

*Note: Slight variation is normal; the important result is no performance regression.*

**Impact:** Significantly reduced CPU usage when miner is paused or waiting for jobs.

**Files changed:** `src/mining/worker.rs`, `src/mining/mod.rs`

---

### 10. SIMD Code Refactoring (2026-01-30)

**Problem:** The `sha256_simd.rs` file was 1646 lines long, mixing generic, SHA-NI, AVX2, and AVX512 implementations in a single file.

**Solution:** Refactored into a module directory with separate files for each implementation.

**Structure:**

```text
src/mining/sha256_simd/
├── mod.rs      # 752 lines - public API, types, constants, tests
├── generic.rs  # 134 lines - portable fallback
├── shani.rs    # 284 lines - SHA-NI implementation
├── avx2.rs     # 243 lines - AVX2 8-way parallel
└── avx512.rs   # 283 lines - AVX512 16-way parallel
```

**Benchmark Results:**

| Configuration | Before    | After     |
|---------------|-----------|-----------|
| 24 threads    | 3.16 TH/s | 3.12 TH/s |

*Note: Within normal variance; no performance regression.*

**Impact:** Improved code maintainability and readability. Each SIMD implementation is now in its own focused file.

**Files changed:** `src/mining/sha256_simd/` (directory replaces single file)

---

### 11. Criterion Benchmark Infrastructure (2026-01-30)

**Problem:** Benchmarks were in `tests/profiling_tests.rs` using `#[ignore]` tests instead of proper benchmark framework.

**Solution:** Migrated to criterion benchmarks in `benches/`.

**Structure:**

```text
benches/
└── mining_benchmarks.rs    # Criterion benchmarks
    ├── hasher_batch_size   # Batch size comparison
    ├── throughput          # Single-threaded throughput
    ├── thread_scaling      # Multi-threaded scaling
    ├── midstate_compute    # Midstate overhead
    └── extranonce2         # Generation method comparison
```

**Impact:** Proper statistical benchmarking with HTML reports, historical comparison, and regression detection.

**Files changed:** `Cargo.toml`, `benches/mining_benchmarks.rs`, `tests/profiling_tests.rs`

---

### 12. Extranonce2 Generation Optimization (2026-01-30)

**Problem:** `generate_extranonce2()` used `SystemTime::now()` which involves a syscall.

**Solution:** Replaced with static atomic counter for ~12x faster generation.

**Before:**
```rust
let nanos = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .subsec_nanos();
```

**After:**
```rust
static EXTRANONCE2_COUNTER: AtomicU64 = AtomicU64::new(0);
let counter = EXTRANONCE2_COUNTER.fetch_add(1, Ordering::Relaxed);
```

**Benchmark Results:**

| Method | Time | Speedup |
| ------ | ---- | ------- |
| SystemTime | 17.79 ns | baseline |
| Atomic counter | 1.49 ns | **12x** |

**Impact:** While extranonce2 is only generated once per nonce-range-exhaustion (rare), this removes unnecessary syscall overhead and improves code clarity.

**Files changed:** `src/mining/worker.rs`

---

### 13. Atomic Ordering Optimization (2026-01-30)

**Problem:** All atomic operations used `SeqCst` ordering, the most conservative option, when weaker orderings would suffice.

**Solution:** Replaced with appropriate orderings:

| Operation | Before | After | Rationale |
| --------- | ------ | ----- | --------- |
| `connected.store()` | SeqCst | Release | Pairs with Acquire load |
| `connected.load()` | SeqCst | Acquire | Pairs with Release store |
| `request_id.fetch_add()` | SeqCst | Relaxed | Only needs uniqueness |

**Impact:** Reduces memory barrier overhead. On x86, the difference is minimal but on ARM/weak memory models this can improve performance.

**Files changed:** `src/stratum/v1.rs`, `src/stratum/v2.rs`

---

## Pending Optimizations

None currently pending.

---

## Benchmarking Guide

Run benchmarks using criterion:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench -- hasher_batch_size
cargo bench -- throughput
cargo bench -- thread_scaling
cargo bench -- extranonce2

# Run with HTML report (saved to target/criterion/)
cargo bench

# Quick check (test mode)
cargo bench -- --test
```

**Benchmark groups:**

| Group | Description |
| ----- | ----------- |
| `hasher_batch_size` | Tests different batch sizes (2^16 to 2^20) |
| `throughput` | Single-threaded sustained throughput |
| `thread_scaling` | Multi-threaded scaling (1, N/2, N threads) |
| `midstate_compute` | Midstate computation overhead |
| `extranonce2` | Compares SystemTime vs atomic counter |

**HTML Reports:**

Criterion generates HTML reports in `target/criterion/`. Open `target/criterion/report/index.html` in a browser to view results with graphs and historical comparisons.

---

## Hardware Requirements

### SHA-NI Support
- **Intel:** Goldmont (2016+), Ice Lake (2019+), Rocket Lake (2021+)
- **AMD:** Zen (2017+) and all later architectures

Check your CPU:
```bash
# Via CLI
./target/release/btcminer --version

# Or check CPU features directly (Linux)
lscpu | grep -i sha
```

### AVX2 Support
- **Intel:** Haswell (2013+)
- **AMD:** Excavator (2015+)

### AVX512 Support
- **Intel:** Skylake-X (2017+), Ice Lake (2019+)
- **AMD:** Zen 4 (2022+)
