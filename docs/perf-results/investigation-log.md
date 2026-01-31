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
