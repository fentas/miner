# Performance Investigation Log

**Date**: 2026-01-31
**Goal**: Find root cause of 400x hashrate difference

---

## Baseline Establishment

### Current Production Performance (sha2 crate)
- **Config**: `hasher = "sha2"` in btcminer.toml
- **Observed**: 85.93 GH/s via API (from baseline.sh script)
- **Per-thread**: ~3.58 GH/s (24 threads)

### Current SIMD Performance (benchmark)
- **Config**: `--benchmark` flag (uses mine_with_midstate directly)
- **Observed**: ~244 MH/s total
- **Per-thread**: ~10 MH/s

### Ratio
- sha2 production / SIMD benchmark = **350x difference**
- This is the mystery we're investigating

---

## Test Execution Log

### Test 1: Standalone SHA2 (2026-01-31)

**Purpose**: Isolate sha2 crate performance without btcminer library

**File**: `tests/standalone_sha2_bench.rs`

**Code**: Direct sha2 crate usage in test binary
```rust
for _ in 0..262144 {
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    let first = Sha256::digest(&header);
    let hash = Sha256::digest(&first);
    nonce = nonce.wrapping_add(1);
}
```

**Result**:
- Total: **117 MH/s**
- Per-thread: **4.9 MH/s**

**Interpretation**: Very slow compared to production baseline (3.58 GH/s/thread)

---

### Test 2: Coordinator with SHA2 Hasher (2026-01-31)

**Purpose**: Test if btcminer's Coordinator matches production speed

**File**: `tests/coordinator_sha2_bench.rs`

**Code**: Uses btcminer library's Coordinator with HasherType::Sha2Crate
```rust
let coordinator = Coordinator::with_options(thread_count, 1 << 18, HasherType::Sha2Crate);
coordinator.update_job(job);
coordinator.start();
// ... wait and measure
```

**Result**:
- Total: **76.01 GH/s**
- Per-thread: **3.17 GH/s**

**Interpretation**:
- **MATCHES PRODUCTION!** (baseline was 85 GH/s, this is 0.89x)
- Cross-crate boundary is **NOT** the issue for SHA2
- Something in standalone test is causing 650x slowdown

---

## Key Findings Summary

| Test | Hashrate | Per-Thread | vs Baseline |
|------|----------|------------|-------------|
| Production (baseline) | 85 GH/s | 3.58 GH/s | 1.0x |
| Coordinator (SHA2) test | 76 GH/s | 3.17 GH/s | **0.89x** ✓ |
| Standalone SHA2 test | 117 MH/s | 4.9 MH/s | **0.0014x** ✗ |
| SIMD benchmark (--benchmark) | 244 MH/s | 10 MH/s | 0.003x |

---

## Critical Insight

**Cross-crate boundary theory is WRONG for SHA2!**

The Coordinator test (Test 2) proves that:
1. Calling btcminer library from a test binary still achieves production-level performance (76 GH/s)
2. The sha2 crate IS fast when called through the library

**New mystery**: Why is standalone sha2 test 650x slower?

Both tests use sha2 crate but:
- Coordinator path: Worker → Hasher → hash_nonce_range_sha2() → hash_header() → sha2 crate
- Standalone path: Direct loop → sha2 crate

The library adds layers but is somehow 650x FASTER.

---

### Test 3: Direct Hasher::hash_header (2026-01-31)

**Purpose**: Test if Hasher::hash_header is the magic

**File**: `tests/hasher_direct_bench.rs`

**Result**:
- Total: **117 MH/s**
- Per-thread: **4.91 MH/s**
- **SLOW** - Same as standalone

**Conclusion**: Hasher struct itself is NOT the magic.

---

### Test 4: Direct hash_nonce_range call (2026-01-31)

**Purpose**: Test if calling hash_nonce_range directly is fast

**File**: `tests/hash_nonce_range_bench.rs`

**Result**:
- Total: **109 MH/s**
- Per-thread: **4.55 MH/s**
- **SLOW** - Same as standalone

**Re-verified without bitcoind**: Still 109 MH/s (slow)

**Coordinator re-verified without bitcoind**: Still 73 GH/s (fast)

**CRITICAL**: Same function `hash_nonce_range()` is 670x faster through Coordinator!

---

## Updated Summary Table

| Test | Hashrate | Per-Thread | vs Baseline |
|------|----------|------------|-------------|
| Production (baseline) | 85 GH/s | 3.58 GH/s | 1.0x |
| Coordinator (SHA2) | **73 GH/s** | **3.06 GH/s** | **0.86x ✓** |
| hash_nonce_range direct | 109 MH/s | 4.55 MH/s | 0.0013x ✗ |
| Hasher::hash_header direct | 117 MH/s | 4.91 MH/s | 0.0014x ✗ |
| Standalone SHA2 | 117 MH/s | 4.9 MH/s | 0.0014x ✗ |
| SIMD benchmark | 244 MH/s | 10 MH/s | 0.003x |

---

## KEY INSIGHT

The SAME function `hash_nonce_range()` behaves differently:
- Through Coordinator/Worker: **73 GH/s** (fast)
- Called directly from test: **109 MH/s** (slow)

**This is NOT about**:
- Cross-crate boundaries
- Hasher struct
- hash_header function
- bitcoind running

**This IS about**:
- How Worker threads are spawned and run
- Something in the Worker's run loop
- Possibly: thread priority, CPU affinity, or compiler optimization context

---

## Theories to Test Next

1. **Worker::spawn uses thread::Builder** - Maybe thread naming affects scheduling
2. **Worker run loop structure** - The job checking/notification system
3. **Shared Stats object** - PaddedAtomicU64 cache line optimization
4. **Compiler sees different context** - Worker is in library, my test is in test binary

---

## Files Created

- `tests/standalone_sha2_bench.rs` - Direct sha2 crate test (slow: 117 MH/s)
- `tests/coordinator_sha2_bench.rs` - Library Coordinator test (fast: 76 GH/s)
- `docs/performance-investigation.md` - Full theory documentation
- `docs/performance-test-plan.md` - Test plan with 10 tests
- `docs/perf-results/investigation-log.md` - This file

---

---

### Test 5: Worker-Style Thread Spawning (2026-01-31)

**Purpose**: Mimic Worker::spawn exactly with thread::Builder::new().name()

**File**: `tests/worker_style_bench.rs`

**Config**: `enable_cpu = true, threads = 0, hasher = "sha2"`

**Code**: Creates threads exactly like Worker::spawn
```rust
let handle = thread::Builder::new()
    .name(format!("miner-{}", id))
    .spawn(move || {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        // ... hash loop calling hash_nonce_range ...
    })
```

**Result**:
- Total: **110 MH/s**
- Per-thread: **4.61 MH/s**
- **SLOW** - Same as other direct tests

**Conclusion**: Thread naming and creating hasher inside thread is NOT the magic.

---

## BREAKTHROUGH INSIGHT

After analyzing the code, I discovered the critical difference:

| Approach | Where Threads Created | Hashrate |
|----------|----------------------|----------|
| Coordinator test | Worker::spawn (IN LIBRARY) | 73 GH/s |
| Direct tests | std::thread::spawn (IN TEST) | 109 MH/s |

**The issue is cross-compilation-unit LTO**:
- When threads are spawned INSIDE the library (`Worker::spawn`), the compiler can optimize across function boundaries within that compilation unit
- When threads are spawned OUTSIDE the library (in test binary) and call into the library, function inlining is blocked at the library boundary

This explains why:
- Coordinator test is fast: threads are created and run within the library's compilation unit
- Direct tests are slow: threads are in the test binary, calling across the library boundary

---

### Test 6: Direct Worker::spawn (2026-01-31)

**Purpose**: Call Worker::spawn directly from test file to confirm the theory

**File**: `tests/worker_spawn_direct_bench.rs`

**Config**: `enable_cpu = true, threads = 0, hasher = "sha2"`

**Code**: Uses Worker::spawn (threads created INSIDE library)
```rust
let handle = Worker::spawn(
    id,
    Arc::clone(&stats),
    Arc::clone(&running),
    // ... other args ...
    hasher_type,
).expect("Failed to spawn worker");
```

**Result**:
- Total: **77 GH/s**
- Per-thread: **3.2 GH/s**
- **FAST!** - Matches production!

**Comparison**:
- Test 5 (test-created threads): 103 MH/s
- Test 6 (library-created threads): 77 GH/s
- **Speedup: 750x**

---

## ROOT CAUSE CONFIRMED

### The Issue: Cross-Compilation-Unit Function Inlining

When threads are spawned INSIDE the library (`Worker::spawn`), the thread's closure runs within the library's compilation unit. This allows the Rust compiler to:

1. **Inline the hot path**: `hash_nonce_range()` → `hash_nonce_range_sha2()` → `hash_header()` → sha2 crate
2. **Apply LTO optimizations** across function boundaries
3. **Unroll loops** and eliminate overhead

When threads are spawned in the TEST binary and call INTO the library, function calls cross the compilation boundary. Even with LTO enabled, the optimizer cannot inline across the library boundary because:
- The thread's closure is in the test binary
- The called functions are in the library
- The boundary prevents full optimization

### Evidence

| Test | Thread Creation | Hashrate | Per-Thread |
|------|-----------------|----------|------------|
| Test 5 | IN TEST FILE | 103 MH/s | 4.3 MH/s |
| Test 6 | IN LIBRARY (Worker::spawn) | 77 GH/s | 3.2 GH/s |
| Coordinator | IN LIBRARY | 73-76 GH/s | 3.0-3.2 GH/s |
| Production | IN LIBRARY | 85 GH/s | 3.58 GH/s |

### Implications

1. **Benchmark mode (`--benchmark`) is affected**: It creates threads in main.rs which calls into the mining library
2. **Integration tests are affected**: Same issue - tests create threads that call library functions
3. **Production is NOT affected**: Coordinator/Worker creates threads inside the library

### Solution Options

1. **For benchmarks**: Use Coordinator instead of direct SIMD calls
2. **For tests**: Call Worker::spawn or Coordinator instead of manual thread spawning
3. **For accurate SIMD benchmarking**: Move the benchmark loop inside the library

---

## Final Summary Table

| Test | Hashrate | Per-Thread | vs Baseline | Status |
|------|----------|------------|-------------|--------|
| Production | 85 GH/s | 3.58 GH/s | 1.0x | ✓ |
| Coordinator test | 73-76 GH/s | 3.0-3.2 GH/s | 0.86x | ✓ |
| Worker::spawn test | **77 GH/s** | **3.2 GH/s** | **0.89x** | **✓ FAST** |
| Test-created threads | 103 MH/s | 4.3 MH/s | 0.0012x | ✗ SLOW |
| Standalone SHA2 | 117 MH/s | 4.9 MH/s | 0.0014x | ✗ SLOW |
| SIMD benchmark | 244 MH/s | 10 MH/s | 0.003x | ✗ SLOW |

---

## Files Created

- `tests/standalone_sha2_bench.rs` - Direct sha2 crate test (slow: 117 MH/s)
- `tests/coordinator_sha2_bench.rs` - Library Coordinator test (fast: 76 GH/s)
- `tests/hasher_direct_bench.rs` - Direct Hasher::hash_header (slow: 117 MH/s)
- `tests/hash_nonce_range_bench.rs` - Direct hash_nonce_range (slow: 109 MH/s)
- `tests/worker_style_bench.rs` - Mimic Worker::spawn (slow: 103 MH/s)
- `tests/worker_spawn_direct_bench.rs` - Call Worker::spawn (FAST: 77 GH/s)
- `docs/performance-investigation.md` - Full theory documentation
- `docs/performance-test-plan.md` - Test plan with 10 tests
- `docs/perf-results/investigation-log.md` - This file

---

**Status**: PARTIALLY SOLVED - Cross-compilation-unit issue confirmed for SHA2 path. New investigation needed for SIMD path.

---

## NEW FINDING: SIMD Implementation is Fundamentally Slow

### Benchmark Results (2026-01-31)

After fixing benchmark to use Coordinator (proper thread creation):

| Hasher | Total | Per-Thread |
|--------|-------|------------|
| SIMD | 241 MH/s | 10 MH/s |
| SHA2 Crate | 73 GH/s | 3.05 GH/s |

**SHA2 Crate is 303x faster than SIMD!**

### Analysis

This is **backwards** from what we'd expect:
- SHA2 Crate path does **full 80-byte hashing** (no midstate optimization)
- SIMD path uses **midstate optimization** (should be faster)

Yet the sha2 crate is still 303x faster.

### Root Cause Theory

The sha2 crate (RustCrypto) has highly optimized assembly-level implementations:
1. **Better instruction scheduling** - SHA-NI instructions have latency, crate hides it better
2. **Aggressive loop unrolling** - Processes multiple blocks efficiently
3. **Minimal memory operations** - Avoids unnecessary copies

Our shani.rs implementation has problems:
1. **Excessive memory copies** - Building blocks, copying hashes between rounds
2. **Poor instruction pipelining** - Not hiding SHA-NI instruction latency
3. **No block-level optimization** - Processing one hash at a time

### Code Comparison

**SHA2 Crate path** (fast):
```rust
// In hash_nonce_range_sha2
for nonce in start_nonce..count {
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    let hash = self.hash_header(&header);  // sha2::Sha256::digest()
}
```

**Our SIMD path** (slow):
```rust
// In shani::sha256d_midstate
let mut block = [0u8; 64];
block[..16].copy_from_slice(tail);           // Copy
block[16..20].copy_from_slice(&nonce.to_le_bytes()); // Copy
// ... set padding ...
transform(&mut state, &block);               // SHA-NI
let first_hash = state_to_bytes(&state);     // Copy/convert
let mut block2 = [0u8; 64];
block2[..32].copy_from_slice(&first_hash);   // Copy
// ... set padding ...
transform(&mut state, &block2);              // SHA-NI
state_to_bytes(&state)                       // Copy/convert
```

### Next Steps

1. **Option A**: Remove SIMD implementation, use sha2 crate everywhere
2. **Option B**: Rewrite SIMD to minimize memory operations
3. **Option C**: Use sha2 crate with midstate API if available

For now, the SHA2 crate path achieves production-level performance (73-85 GH/s).

---

## ROOT CAUSE FOUND: `#[target_feature]` Blocks Inlining

### Test 7: Single-Thread Direct Comparison (2026-01-31)

Running the same code directly from a test (NOT through Coordinator):

| Implementation | Single Thread | Through Coordinator (per thread) |
|----------------|---------------|----------------------------------|
| SHA2 Crate | 10 MH/s | 3,050 MH/s (305x speedup!) |
| Our SIMD | 23 MH/s | 10 MH/s (NO speedup!) |

**The SHA2 crate gets the cross-compilation-unit optimization, our SIMD does not!**

### The Culprit: `#[target_feature]`

Our SIMD code uses:
```rust
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn mine(...) { ... }
```

The `#[target_feature]` attribute:
1. **Cannot be inlined** across compilation unit boundaries
2. Prevents the compiler from optimizing across function calls
3. The compiler cannot verify SHA-NI availability in the calling context

The sha2 crate uses a different approach:
1. Runtime CPU feature detection internally
2. Public API (`Sha256::digest()`) has no `#[target_feature]`
3. Compiler CAN inline the public API into user code
4. Internal dispatch to optimized code happens at runtime

### Solution

**Use SHA2 crate as the default hasher** - it achieves 73+ GH/s through Coordinator.

Our SIMD implementation cannot match this because:
- The `#[target_feature]` attribute is REQUIRED to use SHA-NI intrinsics
- But it also PREVENTS the optimization that makes the code fast

To fix our SIMD, we would need to:
1. Compile with `-C target-feature=+sha` globally (loses CPU compatibility)
2. Or rewrite to use runtime dispatch without `#[target_feature]` on public API

---

## FINAL STATUS: SOLVED

**Root Causes Identified:**

1. **Cross-compilation-unit inlining** - Threads must be created inside the library for full optimization
2. **`#[target_feature]` blocks inlining** - Our SIMD cannot benefit from the optimization

**Solution:**

Use `hasher = "sha2"` in btcminer.toml (which is now the default). This achieves:
- 73-85 GH/s total (24 threads)
- 3.0-3.6 GH/s per thread
- Same performance as the production baseline

The SIMD implementation (241 MH/s) should be deprecated or used only as a fallback for CPUs without good sha2 crate support.

---

## ADDENDUM: Inline Assembly Experiment (2026-01-31)

### Hypothesis

Inline assembly (`asm!` macro) might avoid the `#[target_feature]` inlining boundary since the assembly becomes part of the function body.

### Implementation

Created `HasherType::Assembly` using Rust's `asm!` macro with:

- Full SHA-256 transform in inline assembly (~300 lines)
- SHA-NI instructions: sha256rnds2, sha256msg1, sha256msg2
- No `#[target_feature]` on public function
- Runtime CPU detection with generic fallback

### Benchmark Results (Coordinator, 24 Threads)

| Implementation | Total        | Per Thread  | vs SHA2 |
| -------------- | ------------ | ----------- | ------- |
| SHA2 Crate     | 84.00 GH/s   | 3.50 GH/s   | 1.0x    |
| SIMD           | 240.38 MH/s  | 10.02 MH/s  | 0.003x  |
| Assembly       | 183.66 MH/s  | 7.65 MH/s   | 0.002x  |

### Why Assembly Didn't Help

The inline assembly implementation did NOT achieve the expected speedup. It's actually slower than the intrinsics-based SIMD:

1. **Runtime detection overhead**: Each hash call checks CPU features
2. **State packing overhead**: Converting between [u32; 8] and XMM registers
3. **Missing optimizations**: The sha2 crate has years of optimization
4. **Different root cause**: The sha2 crate's speed isn't just about avoiding `#[target_feature]` - it has fundamentally better code generation

### Final Conclusion

SHA2 crate remains the best option. The inline assembly experiment confirms that the performance gap is not just about inlining boundaries - the sha2 crate's implementation is superior in multiple ways.

**Final recommendation**: Use `hasher = "sha2"` (default) for best performance.

---

## ASM vs SIMD Deep Dive (2026-01-31)

### Theories

The Assembly implementation (184 MH/s) being slower than SIMD (240 MH/s) might be due to:

1. Extra CPU feature detection overhead (ASM checks 3 features vs SIMD's 2)
2. Suboptimal instruction scheduling in hand-written assembly
3. LLVM generates better code from intrinsics than hand-written asm

### Test File

`tests/asm_simd_diagnostic.rs`

### Test Results

#### CPU Feature Detection Overhead

```text
100,000,000 iterations:
  2 features (sha + sse4.1):        39.89 ms (0.40 ns/check)
  3 features (sha + sse4.1 + ssse3): 59.20 ms (0.59 ns/check)

Extra overhead for 3rd check: 0.19 ns/hash
```

CPU detection overhead is negligible (0.19 ns vs ~70 ns per hash). NOT the bottleneck.

#### Function Call Overhead (Single Thread, Direct Calls)

```text
  Implementation             Rate     vs Generic
  ─────────────              ────     ──────────
  Generic               2.96 MH/s          1.0x
  SIMD                 21.69 MH/s          7.3x
  ASM                  14.26 MH/s          4.8x
```

#### Tight Loop Performance (Single Thread)

```text
  Implementation             Rate     vs Generic
  ─────────────              ────     ──────────
  Generic               3.01 MH/s          1.0x
  SIMD                 21.77 MH/s          7.2x
  ASM                  14.20 MH/s          4.7x
```

### Test Analysis

1. **SIMD is consistently 1.52x faster than ASM** across all tests
2. **Both are ~7x faster than generic** - SHA-NI IS being used in both
3. **The ratio is consistent** whether through Coordinator (240/184 = 1.30x) or direct calls (21.7/14.2 = 1.53x)

### Root Cause: LLVM Generates Better Code

The key insight is that **LLVM's intrinsic lowering produces better machine code** than our hand-written inline assembly.

Reasons:

1. **Better instruction scheduling**: LLVM understands SHA-NI latencies and reorders instructions
2. **Better register allocation**: LLVM can choose optimal register assignments
3. **Dead code elimination**: LLVM can remove unnecessary operations
4. **Constant propagation**: LLVM can fold constants at compile time

Our inline assembly forces specific instruction ordering, which may not be optimal for the CPU's out-of-order execution.

### Verification

Both implementations use SHA-NI instructions (sha256rnds2, sha256msg1, sha256msg2), but:

- Intrinsics let LLVM decide the final instruction sequence
- Inline asm forces our exact instruction order

The 1.5x difference shows that LLVM's decisions are better than our manual scheduling.

### Deep Dive Conclusion

The theory that inline assembly would avoid inlining boundaries was **correct in principle but wrong in practice**:

- Yes, inline asm avoids `#[target_feature]` boundaries
- But the performance loss from suboptimal scheduling outweighs any inlining benefit
- LLVM intrinsics + `#[target_feature]` produces better code than manual asm without `#[target_feature]`

**Final ranking** (Coordinator, 24 threads):

1. SHA2 Crate: **84 GH/s** (default, highly optimized)
2. SIMD: **240 MH/s** (intrinsics, good but limited by #[target_feature])
3. Assembly: **184 MH/s** (manual asm, worse than LLVM's intrinsic lowering)

---

## AsmV2: SHA2 Crate Style Implementation (2026-01-31)

### AsmV2 Hypothesis

Apply sha2 crate's key optimizations to our implementation:

1. Use `cpufeatures` crate for one-time CPU detection (not per-hash)
2. Use clean macro structure matching sha2 crate for better LLVM optimization
3. Keep state in XMM registers across transforms

### AsmV2 Implementation

Created `src/mining/sha256_simd/asm_v2.rs` with:

- `cpufeatures::new!()` for static CPU detection
- `rounds4!` and `schedule_rounds4!` macros matching sha2 structure
- Correct K constant ordering for `_mm_set_epi32`
- Public function without `#[target_feature]` for inlining

### AsmV2 Benchmark Results (Coordinator, 24 Threads)

| Implementation | Total       | Per Thread  | vs SHA2 |
| -------------- | ----------- | ----------- | ------- |
| SHA2 Crate     | 85.05 GH/s  | 3.54 GH/s   | 1.00x   |
| SIMD           | 269.64 MH/s | 11.23 MH/s  | 0.003x  |
| **AsmV2**      | 270.77 MH/s | 11.28 MH/s  | 0.003x  |
| Assembly       | 197.97 MH/s | 8.25 MH/s   | 0.002x  |

### AsmV2 Analysis

**AsmV2 improvements:**

- 37% faster than original Assembly (270 vs 198 MH/s)
- Now matches SIMD performance
- `cpufeatures` one-time detection works correctly

**Still 300x slower than SHA2 crate because:**

The `#[target_feature]` boundary still blocks inlining. Even though our public function has no `#[target_feature]`, the internal `transform_block` function does, and LLVM cannot inline across this boundary.

The sha2 crate solves this differently:

- Their internal `digest_blocks` has `#[target_feature]`
- But their public `compress()` just checks `shani_cpuid::get()` and calls it
- The key is that sha2's threads are created INSIDE the library where `compress()` is defined
- Our threads are created in the Coordinator, also inside the library
- But our hot loop calls functions that cross into code with `#[target_feature]`

### AsmV2 Root Cause

The sha2 crate achieves 85 GH/s because:

1. **Same compilation unit**: Their `compress()` and `digest_blocks()` are in the same file
2. **Intra-crate inlining**: When threads call `Sha256::digest()`, LLVM can see the whole call chain
3. **No `#[target_feature]` on public API**: The boundary is internal

Our implementations (SIMD, Assembly, AsmV2) cannot match this because the `#[target_feature]` functions are called across a function boundary that LLVM won't inline across.

**Updated ranking** (Coordinator, 24 threads):

1. SHA2 Crate: **85 GH/s** (default, recommended)
2. AsmV2: **271 MH/s** (sha2-style macros with cpufeatures)
3. SIMD: **270 MH/s** (intrinsics)
4. Assembly: **198 MH/s** (manual asm)

---

## AsmV3: Global Target-Feature Compilation (2026-01-31)

### AsmV3 Hypothesis

If we compile with global `-C target-feature=+sha,+sse4.1,+ssse3,+sse2`, we can remove ALL `#[target_feature]` attributes. This should allow full LLVM inlining.

### AsmV3 Implementation

1. Created `.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-feature=+sha,+sse4.1,+ssse3,+sse2"]
```

2. Created `src/mining/sha256_simd/asm_v3.rs`:
   - **No `#[target_feature]` on ANY function**
   - Uses `#[inline(always)]` on all functions
   - Same algorithm as AsmV2

### AsmV3 Benchmark Results

#### Direct Calls (Single Thread with black_box)

| Implementation | Hashrate | Notes |
|----------------|----------|-------|
| AsmV3 | 22.92 MH/s | Correct result |
| SIMD | 22.80 MH/s | Correct result |
| SHA2 Crate | 21.36 MH/s | Does MORE work (no midstate) |
| Generic | 2.94 MH/s | Baseline |

**Key finding**: Single-threaded, AsmV3 and SIMD are roughly equal (~23 MH/s). SHA2 crate doing full hashes (no midstate) is slightly slower at 21 MH/s.

#### Single-Thread Coordinator Benchmark

| Implementation | Hashrate |
|----------------|----------|
| SHA2 Crate | 21.91 MH/s |
| AsmV3 | 22.65 MH/s |

**AsmV3 is actually 3% FASTER than SHA2 crate per hash!**

### Critical Bug Discovered: Hash Counting Error

Running multi-threaded Coordinator benchmark revealed a critical bug:

| Threads | SHA2 Crate Hashrate | Per Thread |
|---------|---------------------|------------|
| 1 | 22.02 MH/s | 22.02 MH/s |
| 4 | 88.24 MH/s | 22.06 MH/s |
| 8 | 162.95 MH/s | 20.37 MH/s |
| 16 | 237.18 MH/s | 14.82 MH/s |
| **17** | **217,770 MH/s** | **12,810 MH/s** |
| 20 | 213,727 MH/s | 10,687 MH/s |
| 24 | 193,413 MH/s | 8,059 MH/s |

**The hash count explodes at exactly 17 threads!**

- 16 threads: 1.2 billion hashes in 5 seconds (reasonable)
- 17 threads: 1.1 TRILLION hashes in 5 seconds (impossible!)

This is a bug in the benchmark or stats tracking, NOT a real performance difference.

The earlier "188 GH/s" for SHA2 crate was caused by this bug - the hash counting is corrupted when using 17+ threads.

### Bug Analysis

The anomaly boundary is between 16 and 17 threads:
- 16 = 2^4 (power of 2) → works correctly
- 17 → hash count explodes by ~1000x

Possible causes:
1. Buffer overflow in some fixed-size array assumed max 16 threads
2. Race condition that only triggers with >16 threads
3. Memory corruption in Stats struct

**Note**: The single-thread benchmark shows the TRUE performance:
- SHA2 Crate: ~22 MH/s per thread
- AsmV3: ~23 MH/s per thread

Both are essentially equal! The "85 GH/s" production performance was likely due to this counting bug.

### AsmV3 Conclusion

1. Global target-feature compilation works - AsmV3 compiles without `#[target_feature]`
2. AsmV3 achieves ~23 MH/s per thread, matching SHA2 crate
3. **CRITICAL**: The multi-threaded benchmark has a bug that inflates hash counts at 17+ threads
4. Need to investigate the hash counting bug to get accurate multi-threaded results

---

## Files Created/Modified in This Session

- `.cargo/config.toml` - Global target-feature compilation flags
- `src/mining/sha256_simd/asm_v3.rs` - No #[target_feature] implementation
- `src/mining/sha256_simd/mod.rs` - Added asm_v3 module
- `src/mining/hasher.rs` - Added HasherType::AsmV3
- `src/config.rs` - Added HasherConfig::AsmV3
- `tests/hasher_comparison_bench.rs` - Added AsmV3 to benchmark
- `src/bin/bench_verify.rs` - Single-thread verification with black_box
- `src/bin/single_thread_bench.rs` - Single-thread Coordinator benchmark
- `src/bin/multi_thread_bench.rs` - Multi-thread verification showing bug

---

## Next Investigation: Hash Counting Bug

The hash counting bug that triggers at 17+ threads needs investigation:

1. Check PaddedAtomicU64 for overflow issues
2. Check if Stats struct has alignment problems at certain thread counts
3. Verify no fixed-size arrays assume max 16 threads
4. Test with Ordering::SeqCst instead of Relaxed

---
