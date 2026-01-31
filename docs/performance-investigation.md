# Performance Investigation: 400x Hashrate Discrepancy

## Executive Summary

We observe a **400x performance difference** between different hashrate measurement scenarios:

| Scenario | Per-Thread Rate | Total (24 threads) |
|----------|-----------------|-------------------|
| sha2 crate in production | ~4 GH/s | **98 GH/s** |
| sha2 crate in tests | ~10 MH/s | ~240 MH/s |
| Our SIMD in production (benchmark) | ~10 MH/s | ~241 MH/s |
| Our SIMD in tests | ~12 MH/s | ~288 MH/s |

**Key observations:**
1. sha2 crate is **400x faster** in production than in tests
2. Our SIMD implementation is **400x slower** than sha2 crate in production
3. Our SIMD and sha2 crate perform **nearly identically** in tests (~10 MH/s)

---

## Detailed Measurements

### Test Environment
- **CPU**: 24 threads (likely 12-core with hyperthreading)
- **SIMD Level**: SHA-NI detected and available
- **OS**: WSL2 Linux / Windows native
- **Rust**: Release build with `lto = "fat"`

### Measurement Results

#### 1. Production Binary with sha2 crate (`hasher = "sha2"`)
```
TUI Display: 98.60 GH/s
Per-thread: ~4.1 GH/s
DEBUG SHA2-HASHER messages confirm real hashes computed
```

#### 2. Production Binary Benchmark (`--benchmark`, uses SIMD)
```
SIMD Level:    SHA-NI
Threads:       24
Total hashrate:   241.45 MH/s
Per-thread rate:  10.06 MH/s
```

#### 3. Integration Test with SIMD hasher
```
cargo test --release --test hashrate_compare_test hasher_type_comparison
SIMD hashrate: 12.16 MH/s (single-threaded)
```

#### 4. Integration Test with sha2 crate hasher
```
SHA2 hashrate: 10.42 MH/s (single-threaded)
```

---

## Theories

### Theory 1: Cross-Crate Optimization Boundary

**Hypothesis**: Rust's `#[target_feature]` functions cannot be inlined across crate boundaries, causing massive performance degradation.

**Evidence for:**
- sha2 crate shows 400x difference between production and tests
- `#[target_feature(enable = "sha")]` prevents `#[inline(always)]`
- Test binaries link against pre-compiled `.rlib` files

**Evidence against:**
- Our SIMD implementation shows **same ~10 MH/s** in both production benchmark AND tests
- If cross-crate was the only issue, our SIMD should be fast in production benchmark
- The benchmark calls `mine_with_midstate` directly from `main.rs` (same compilation unit)

**Status**: Partially explains sha2 behavior, but NOT our SIMD slowness

---

### Theory 2: Our SIMD Implementation is Fundamentally Slow

**Hypothesis**: Our custom SHA-NI implementation has inefficiencies that make it ~400x slower than the sha2 crate's implementation.

**Evidence for:**
- Our SIMD shows ~10 MH/s even in production benchmark (same compilation unit)
- sha2 crate achieves ~4 GH/s per thread when fully optimized
- Our code does extensive memory operations per hash:
  - Builds 64-byte blocks on stack for each nonce
  - Multiple `copy_from_slice` calls per hash
  - `state_to_bytes` conversion after each transform
  - Two transform calls per double-SHA256

**Evidence against:**
- Our implementation uses the same SHA-NI intrinsics as sha2 crate
- The algorithm is mathematically identical
- SHA-NI intrinsics should dominate the performance

**Status**: Likely contributing factor, needs verification

---

### Theory 3: Benchmark Function Has Issues

**Hypothesis**: The `--benchmark` flag implementation has bugs that prevent full optimization.

**Evidence for:**
- Benchmark uses `mine_with_midstate` which includes dispatch logic:
  ```rust
  pub fn mine_with_midstate(...) {
      let level = SimdLevel::detect();  // Called every time!
      if level == SimdLevel::ShaNi {
          return unsafe { shani::mine(...) };
      }
      ...
  }
  ```
- `SimdLevel::detect()` is called on every batch (262144 nonces)
- Dispatch through match/if prevents inlining

**Evidence against:**
- SIMD detection should be fast (CPUID cached)
- 262144 nonces per call should amortize dispatch overhead

**Status**: Possible minor factor, unlikely to cause 400x difference

---

### Theory 4: sha2 Crate Uses Assembly Backend

**Hypothesis**: The sha2 crate uses highly optimized assembly code that our implementation cannot match.

**Evidence for:**
- sha2 crate 0.10+ includes `sha2-asm` support
- RustCrypto libraries are heavily optimized
- sha2 crate auto-detects and uses SHA-NI via cpufeatures
- Professional cryptographers optimized the assembly

**Evidence against:**
- Both use the same SHA-NI instructions
- Intrinsics should compile to equivalent assembly
- Our implementation follows Intel's reference

**Status**: Likely significant factor

---

### Theory 5: Compiler Optimization Differences

**Hypothesis**: The production binary and benchmark use different compiler settings.

**Evidence for:**
- Binary: Full LTO across entire crate
- Benchmark: Same binary, but calling library functions
- Test: Links against `.rlib` with separate compilation

**Evidence against:**
- Benchmark is compiled INTO the main binary
- Should have same optimization level

**Status**: Unlikely for benchmark, confirmed for tests

---

### Theory 6: Memory/Cache Effects

**Hypothesis**: Our implementation has poor cache utilization compared to sha2 crate.

**Evidence for:**
- Our code allocates stack arrays per hash
- Block building involves memory copies
- sha2 crate may use better memory access patterns

**Evidence against:**
- SHA-256 is compute-bound, not memory-bound
- Stack allocations should be fast

**Status**: Possible minor factor

---

### Theory 7: Instruction-Level Parallelism (ILP)

**Hypothesis**: sha2 crate's implementation has better instruction scheduling that maximizes CPU pipeline utilization.

**Evidence for:**
- SHA-NI instructions have 4-6 cycle latency
- Can issue multiple independent instructions per cycle
- Our sequential loop may not exploit ILP
- sha2 crate may use 2-way or 4-way interleaving

**Evidence against:**
- We implemented 2-way interleaving but saw no improvement
- Modern CPUs do out-of-order execution

**Status**: Possible factor, needs investigation

---

## Code Analysis

### Our SIMD Implementation Flow

```
mine_with_midstate()
├── SimdLevel::detect()           // CPU feature detection
├── shani::mine()                 // Dispatch to SHA-NI
    ├── for nonce in 0..count:    // Sequential loop
    │   ├── sha256d_midstate()    // Double SHA-256
    │   │   ├── Build block (64 bytes on stack)
    │   │   ├── copy_from_slice (16 bytes)
    │   │   ├── copy_from_slice (4 bytes nonce)
    │   │   ├── transform()       // First SHA-256
    │   │   ├── state_to_bytes()  // Convert state
    │   │   ├── Build block2 (64 bytes on stack)
    │   │   ├── copy_from_slice (32 bytes)
    │   │   ├── transform()       // Second SHA-256
    │   │   └── state_to_bytes()  // Convert state
    │   └── hash_meets_target()   // Compare
```

**Per-nonce overhead:**
- 2x stack array allocation (128 bytes)
- 4x `copy_from_slice` calls (56 bytes total)
- 2x `state_to_bytes` conversions
- 2x `transform` calls (actual SHA-256)

### sha2 Crate Implementation Flow (Simplified)

```
hash_nonce_range_sha2()
├── for nonce in 0..count:
│   ├── Build header (80 bytes)
│   ├── copy_from_slice (4 bytes nonce)
│   ├── hash_header()
│   │   ├── Sha256::new()
│   │   ├── hasher.update(header)
│   │   ├── hasher.finalize()      // First SHA-256
│   │   ├── Sha256::new()
│   │   ├── hasher.update(first_hash)
│   │   └── hasher.finalize()      // Second SHA-256
│   └── hash_meets_target()
```

**Key difference**: sha2 crate uses streaming API with internal buffering, potentially more cache-friendly.

---

## Open Questions

1. **Why doesn't our SIMD benefit from same-compilation-unit optimization?**
   - The benchmark calls `mine_with_midstate` from main.rs
   - This should be same compilation unit
   - Yet performance matches cross-crate test performance

2. **What specific optimizations does sha2 crate use?**
   - Need to examine sha2 crate source code
   - Compare assembly output

3. **Is SimdLevel::detect() being optimized away?**
   - Called every batch
   - Should be constant after first call
   - Compiler may not know this

4. **Are there hidden function call boundaries?**
   - `#[target_feature]` functions may force boundaries
   - Even within same crate

---

## Conclusions

### Most Likely Explanation

The 400x difference is caused by a **combination of factors**:

1. **sha2 crate has professional-grade optimizations** that our implementation lacks
2. **Cross-crate boundary** prevents sha2 from being fast in tests
3. **Our SIMD implementation** has fundamental inefficiencies:
   - Excessive memory operations per hash
   - Suboptimal instruction scheduling
   - Function call overhead from dispatch

### Why Tests Show Similar Performance

Both sha2 and our SIMD hit a **~10 MH/s floor** in tests because:
- Cross-crate boundary destroys sha2's optimizations
- Our SIMD was never optimized to begin with
- They converge at the "unoptimized baseline"

### Recommended Action

Use `hasher = "sha2"` in production. Our custom SIMD implementation needs significant rework to match sha2 crate performance.

---

## References

- [sha2 crate source](https://github.com/RustCrypto/hashes/tree/master/sha2)
- [Intel SHA-NI reference](https://github.com/noloader/SHA-Intrinsics)
- [Rust target_feature documentation](https://doc.rust-lang.org/reference/attributes/codegen.html#the-target_feature-attribute)
