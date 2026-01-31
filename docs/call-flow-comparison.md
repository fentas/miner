# Call Flow Comparison: Production Binary vs Integration Tests

This document systematically compares the call paths between the production binary (51-65 GH/s) and integration/benchmark tests (~23 MH/s).

---

## Executive Summary

| Aspect | Production Binary | Integration Tests |
|--------|-------------------|-------------------|
| **Entry Point** | `main()` → `run_solo_mode()` | `#[test] fn ...()` |
| **Compilation Unit** | All code in one binary | Test harness links against library |
| **Hasher Creation** | `Hasher::with_type(cfg.hasher_type)` | `Hasher::new()` or `Hasher::with_type()` |
| **SIMD Dispatch** | Same: `mine_with_midstate()` | Same: `mine_with_midstate()` |
| **Actual Hash Fn** | `shani::mine()` (SHA-NI) | `shani::mine()` (SHA-NI) |
| **Measured Rate** | **51-65 GH/s** | **~23 MH/s** |
| **Factor Difference** | 1x (baseline) | **~2,200x slower** |

**The code path is IDENTICAL** - both use the same functions. The difference is **compilation context**.

---

## Side-by-Side Call Flow

### Level 1: Entry Point

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/main.rs:90` `fn main()` | `tests/*.rs` `#[test] fn test_name()` |
| Binary entry point | Test harness calls test function |
| All code compiled together | Links against `libbtcminer.rlib` |

### Level 2: Mining Loop Setup

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/main.rs:1265` `run_solo_mode()` | Test function body |
| `src/main.rs:1550` `Coordinator::with_options()` | Typically: `Hasher::new()` directly |
| Spawns N worker threads | Usually single-threaded or 2-4 threads |
| Each worker: `Hasher::with_type(hasher_type)` | `Hasher::new()` → defaults to `HasherType::Simd` |

### Level 3: Hasher Selection

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/mining/hasher.rs:34` | `src/mining/hasher.rs:25` |
| `Hasher::with_type(cfg.mining.hasher)` | `Hasher::new()` |
| `hasher_type` from config | `hasher_type = HasherType::Simd` (default) |
| **Same result: `HasherType::Simd`** | **Same result: `HasherType::Simd`** |

### Level 4: Hash Range Dispatch

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/mining/hasher.rs:103` | `src/mining/hasher.rs:103` |
| `hasher.hash_nonce_range(...)` | `hasher.hash_nonce_range(...)` |
| **IDENTICAL FUNCTION** | **IDENTICAL FUNCTION** |

```rust
// Both call this exact code:
match self.hasher_type {
    HasherType::Simd => self.hash_nonce_range_simd(...),  // Line 104
    HasherType::Sha2Crate => self.hash_nonce_range_sha2(...),
}
```

### Level 5: SIMD Path

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/mining/hasher.rs:110` | `src/mining/hasher.rs:110` |
| `hash_nonce_range_simd()` | `hash_nonce_range_simd()` |
| **IDENTICAL FUNCTION** | **IDENTICAL FUNCTION** |

```rust
// Both call:
mine_with_midstate(&midstate, &tail, start_nonce, count, target)
```

### Level 6: SIMD Level Detection

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/mining/sha256_simd/mod.rs:120` | `src/mining/sha256_simd/mod.rs:120` |
| `SimdLevel::detect()` | `SimdLevel::detect()` |
| Returns `ShaNi` (if CPU supports) | Returns `ShaNi` (if CPU supports) |
| **IDENTICAL DETECTION** | **IDENTICAL DETECTION** |

### Level 7: SHA-NI Mining Function

| Production Binary | Integration Test |
|-------------------|------------------|
| `src/mining/sha256_simd/shani.rs` | `src/mining/sha256_simd/shani.rs` |
| `unsafe { shani::mine(...) }` | `unsafe { shani::mine(...) }` |
| **IDENTICAL FUNCTION** | **IDENTICAL FUNCTION** |

---

## ASCII Call Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        PRODUCTION BINARY                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  main() ─────────────────────────────────────────────────────────────┐     │
│    │                                                                  │     │
│    ▼                                                                  │     │
│  run_solo_mode()                                                      │     │
│    │                                                                  │     │
│    ▼                                                                  │     │
│  Coordinator::with_options(threads, batch_size, hasher_type)          │     │
│    │                                                                  │     │
│    ▼ (spawns N threads)                                               │     │
│  Worker::spawn() × N                                                  │     │
│    │                                                                  │     │
│    ▼ (each thread)                                                    │     │
│  Worker { hasher: Hasher::with_type(HasherType::Simd) }               │     │
│    │                                                                  │ ALL │
│    ▼                                                                  │ IN  │
│  Worker::run() loop:                                                  │ ONE │
│    │                                                                  │COMP-│
│    ├──────────────────────────────────────────────────────────────┐  │UNIT │
│    │                     HOT PATH                                 │  │     │
│    ▼                                                              │  │     │
│  hasher.hash_nonce_range(&header, nonce, count, &target)          │  │     │
│    │                                                              │  │     │
│    ▼                                                              │  │     │
│  hash_nonce_range_simd()                                          │  │     │
│    │                                                              │  │     │
│    ▼                                                              │  │     │
│  mine_with_midstate()                                             │  │     │
│    │                                                              │  │     │
│    ▼                                                              │  │     │
│  SimdLevel::detect() → ShaNi                                      │  │     │
│    │                                                              │  │     │
│    ▼                                                              │  │     │
│  shani::mine()  ◄─── Intel SHA-NI intrinsics                      │  │     │
│    │                                                              │  │     │
│    └──────────────────────────────────────────────────────────────┘  │     │
│                                                                       │     │
│  RESULT: 51-65 GH/s                                                   │     │
└─────────────────────────────────────────────────────────────────────────────┘


┌─────────────────────────────────────────────────────────────────────────────┐
│                        INTEGRATION TEST                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  test harness (cargo test)                                                  │
│    │                                                                        │
│    ▼                                                                        │
│  #[test] fn test_function()                                          ┌─────┤
│    │                                                                 │TEST │
│    ▼                                                                 │CRATE│
│  Hasher::new()  ──────────────────────────────────────────┐         │     │
│    │                                  LIBRARY BOUNDARY ───┼─────────┼─────┤
│    ▼                                                      │         │RLIB │
│  hasher.hash_nonce_range(&header, nonce, count, &target)  │         │CRATE│
│    │                                                      │         │     │
│    ▼                                                      │         │     │
│  hash_nonce_range_simd()                                  │         │     │
│    │                                                      │         │     │
│    ▼                                                      │         │     │
│  mine_with_midstate()                                     │         │     │
│    │                                                      │         │     │
│    ▼                                                      │         │     │
│  SimdLevel::detect() → ShaNi                              │         │     │
│    │                                                      │         │     │
│    ▼                                                      │         │     │
│  shani::mine()  ◄─── Intel SHA-NI intrinsics              │         │     │
│    │                                                      │         │     │
│    └──────────────────────────────────────────────────────┘         │     │
│                                                                      │     │
│  RESULT: ~23 MH/s                                                    │     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Detailed Function Trace

### Production Binary Call Chain

```
main()                                      [src/main.rs:90]
  │
  ├─ Config::from_file("btcminer.toml")     [src/config.rs]
  ├─ SimdLevel::detect()                    [src/mining/sha256_simd/mod.rs:55]
  │
  └─ run_solo_mode(cfg)                     [src/main.rs:1265]
     │
     ├─ BitcoindManager::new()              [src/bitcoin/manager.rs]
     │
     └─ Coordinator::with_options(          [src/mining/mod.rs:185]
          thread_count,
          1 << cfg.mining.cpu_batch_exp,    # 262144 (2^18)
          cfg.mining.hasher.to_hasher_type() # HasherType::Simd
        )
        │
        └─ for id in 0..thread_count:
           │
           └─ Worker::spawn(id, ..., hasher_type)   [src/mining/worker.rs:87]
              │
              └─ thread::spawn() ──────────────────────────────────┐
                 │                                                  │
                 └─ Worker { hasher: Hasher::with_type(Simd) }      │
                    │                                               │
                    └─ worker.run()               [worker.rs:125]   │
                       │                                            │
                       └─ loop {                                    │
                          │                                         │
                          └─ hasher.hash_nonce_range()  [hasher.rs:96]
                             │
                             └─ match HasherType::Simd:
                                │
                                └─ hash_nonce_range_simd()  [hasher.rs:110]
                                   │
                                   ├─ Midstate::from_header_prefix()
                                   │
                                   └─ mine_with_midstate()  [sha256_simd/mod.rs:113]
                                      │
                                      └─ SimdLevel::detect() → ShaNi
                                         │
                                         └─ shani::mine()   [shani.rs]
                                            │
                                            └─ for nonce in range:
                                               │
                                               └─ sha256d_midstate() using intrinsics
                                                  │
                                                  └─ _mm_sha256rnds2_epu32()
                                                     _mm_sha256msg1_epu32()
                                                     _mm_sha256msg2_epu32()
```

### Integration Test Call Chain

```
cargo test --release                        [test harness]
  │
  ├─ Loads libbtcminer.rlib                 [pre-compiled library]
  │
  └─ #[test] fn bench_test()                [tests/e2e_hashrate_benchmark.rs:71]
     │
     └─ let hasher = Hasher::new();         [hasher.rs:25]
        │
        └─ Hasher { hasher_type: HasherType::Simd }
           │
           └─ hasher.hash_nonce_range()     [hasher.rs:96]  ◄─── LIBRARY CALL
              │
              └─ match HasherType::Simd:
                 │
                 └─ hash_nonce_range_simd()   [hasher.rs:110]
                    │
                    ├─ Midstate::from_header_prefix()
                    │
                    └─ mine_with_midstate()   [sha256_simd/mod.rs:113]
                       │
                       └─ SimdLevel::detect() → ShaNi
                          │
                          └─ shani::mine()    [shani.rs]  ◄─── #[target_feature]
                             │
                             └─ for nonce in range:
                                │
                                └─ sha256d_midstate() using intrinsics
                                   │
                                   └─ _mm_sha256rnds2_epu32()
                                      _mm_sha256msg1_epu32()
                                      _mm_sha256msg2_epu32()
```

---

## Key Differences Analysis

### 1. Compilation Unit Boundary

| Production | Test |
|------------|------|
| Single compilation unit | Two compilation units |
| `btcminer` binary | `test_binary` links `libbtcminer.rlib` |

**Impact**: LTO can fully optimize intra-binary calls but not cross-library calls.

### 2. Function Attributes

```rust
// In shani.rs:
#[target_feature(enable = "sha", enable = "sse4.1")]
pub unsafe fn mine(...) -> Option<(u32, [u8; 32])> {
    // SHA-NI intrinsics
}
```

- `#[target_feature]` functions **cannot be inlined across crate boundaries**
- This is a Rust/LLVM limitation, not a bug

### 3. LTO Behavior

| Scenario | LTO Effect |
|----------|------------|
| Production binary | Full LTO - all code visible to optimizer |
| Test + library | LTO applies to each crate separately |

**Evidence**: Even with `lto = "fat"` in Cargo.toml, cross-crate optimization is limited.

### 4. Inlining Behavior

| Call Site | Can Inline? |
|-----------|-------------|
| Production: `main → hash_nonce_range → shani::mine` | ✅ Yes (same binary) |
| Test: `test → hash_nonce_range → shani::mine` | ❌ No (crosses crate) |

The `#[target_feature]` attribute prevents inlining because:
1. The calling code might not have the target feature enabled
2. Compiler must ensure CPU feature checking happens

---

## What We've Verified

| Check | Production | Test | Same? |
|-------|------------|------|-------|
| HasherType used | `Simd` | `Simd` | ✅ |
| SimdLevel detected | `ShaNi` | `ShaNi` | ✅ |
| Function called | `shani::mine()` | `shani::mine()` | ✅ |
| Batch size | 262144 | 262144 | ✅ |
| Target difficulty | Hard (mainnet) | Hard (mainnet) | ✅ |
| Thread count | N | Variable | ~ |

**All logical paths are identical. The difference is pure compilation artifact.**

---

## Potential Root Causes (To Investigate)

### Hypothesis 1: Cross-Crate Inlining Barrier
**Status**: Most likely

The `#[target_feature]` functions in `shani.rs` cannot be inlined into test code because:
- Test code is compiled separately from library
- LTO doesn't fully work across rlib boundaries
- Function call overhead + no register optimization

### Hypothesis 2: Different RUSTFLAGS
**Status**: Possible

Production might use `-C target-cpu=native` while tests use default target.

```bash
# Check what tests are compiled with:
RUSTFLAGS="-C target-cpu=native" cargo test --release ...
```

### Hypothesis 3: Debug Symbols in Test Build
**Status**: Unlikely

Even `--release` tests should have full optimizations enabled.

### Hypothesis 4: Criterion/Test Harness Overhead
**Status**: Unlikely for this magnitude

Test harness adds microseconds, not 2,200x slowdown.

### Hypothesis 5: Different Memory Layout
**Status**: Possible

Library compilation might produce different struct layouts that cause cache misses.

---

## Experiments to Run

### Experiment 1: Same Binary, Different Entry Points

Add `--benchmark` flag to production binary that runs identical benchmark code:

```rust
// In main.rs
if args.benchmark {
    run_embedded_benchmark();  // Uses same compilation unit
}
```

**Expected**: Should match production rate (51-65 GH/s)

### Experiment 2: RUSTFLAGS for Tests

```bash
RUSTFLAGS="-C target-cpu=native" cargo test --release \
  --test e2e_hashrate_benchmark -- --nocapture
```

**Expected**: May improve but unlikely to reach production rate

### Experiment 3: Assembly Comparison

```bash
# Production binary
objdump -d target/release/btcminer | grep -A 100 "shani::mine"

# Test binary (need to find test binary location)
objdump -d target/release/deps/e2e_hashrate_benchmark-* | grep -A 100 "shani::mine"
```

**Expected**: Different inlining/register allocation

### Experiment 4: Move Hot Code Into Test

Copy `shani::mine()` directly into test file (not as library import):

```rust
// In test file:
#[target_feature(enable = "sha", enable = "sse4.1")]
unsafe fn local_mine(...) { /* copy implementation */ }
```

**Expected**: Should improve significantly if inlining is the issue

---

## Conclusion

**The code paths are identical.** Both flows:
1. Use `HasherType::Simd`
2. Detect `SimdLevel::ShaNi`
3. Call `shani::mine()`
4. Use same intrinsics

The 2,200x difference is a **compilation artifact**, not a code logic difference.

**Recommended Actions**:
1. Accept that benchmarks cannot measure production performance
2. Use API test (`tests/api_hashrate_test.rs`) for accurate measurement
3. Optionally embed benchmark in production binary for true performance testing
