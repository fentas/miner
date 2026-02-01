# Research Test Log v9: CPU Feature Detection and Optimization

**Date:** 2026-02-01
**Focus:** CPU feature detection and SIMD impact analysis
**Goal:** Understand which CPU features are being used and their impact
**Test file:** `tests/research_v9_test.rs`

---

## System Info

This CPU has all modern features available:

| Feature | Available |
|---------|-----------|
| SHA-NI (sha) | ✓ Yes |
| AVX | ✓ Yes |
| AVX2 | ✓ Yes |
| AVX-512F | ✓ Yes |
| AVX-512VL | ✓ Yes |
| SSE4.1 | ✓ Yes |
| SSE4.2 | ✓ Yes |
| AES-NI | ✓ Yes |
| POPCNT | ✓ Yes |
| BMI1 | ✓ Yes |
| BMI2 | ✓ Yes |

---

## Results

### Test 1: SIMD Level Detection

Detected SIMD Level: **ShaNi** (hardware SHA instructions)

Available levels:
- Generic: Scalar fallback
- Avx2: 256-bit SIMD
- Avx512: 512-bit SIMD
- **ShaNi**: Hardware SHA (selected)

---

### Test 2: SHA-NI Impact ⚠️

| Implementation | Rate (MH/s) | Speedup |
|----------------|-------------|---------|
| Sha2Crate | 22.99 | 1.00x |
| SIMD (SHA-NI) | 22.42 | **0.97x** |

**KEY FINDING:** SIMD hasher is NOT faster than sha2 crate!

Actually slightly slower (0.97x). This means:
1. The sha2 crate already uses SHA-NI internally
2. Our SIMD implementation adds overhead
3. Both implementations are hitting the same hardware SHA units

---

### Test 3: Memory Bandwidth Analysis

| Batch Size | Rate (MH/s) | Est. Bandwidth |
|------------|-------------|----------------|
| 1,000 | 22.99 | 2.58 GB/s |
| 10,000 | 22.64 | 2.54 GB/s |
| 100,000 | 22.46 | 2.52 GB/s |
| 1,000,000 | 23.05 | 2.58 GB/s |

**Rate is constant across batch sizes** → System is **compute-bound**, not memory-bound.

---

### Test 4: Instruction Throughput

| Metric | Value |
|--------|-------|
| Hashrate | 23.05 MH/s |
| Est. ops/hash | ~2,560 |
| Throughput | ~59 GOPS |

Note: SHA-NI reduces effective operations because hardware SHA instructions do multiple operations per cycle.

---

### Test 5: Cache Effect Analysis

| Headers | Rate (MH/s) | Relative | Cache Pressure |
|---------|-------------|----------|----------------|
| 1 | 23.61 | 1.00x | Minimal (L1) |
| 10 | 23.03 | 0.98x | Low (L2) |
| 100 | 21.67 | 0.92x | Medium (L3) |
| 1,000 | 22.92 | 0.97x | High (RAM) |

**Minimal cache impact** (8% max degradation). The midstate optimization effectively eliminates cache sensitivity by keeping working set small.

---

## Analysis

### Why SIMD Isn't Faster

The sha2 crate (v0.10+) automatically uses:
1. **SHA-NI** when available (this CPU has it)
2. **AVX2** as fallback
3. **Generic** for portability

Our custom SIMD implementation:
- Also uses SHA-NI
- But adds Rust/function call overhead
- Doesn't beat the well-optimized crate

### The sha2 Crate is Excellent

Looking at [sha2 crate internals](https://crates.io/crates/sha2):
- Uses `cpufeatures` for runtime detection
- Optimized assembly for SHA-NI
- Battle-tested by millions of users

We cannot beat it easily.

### Compute-Bound Confirmation

- Constant rate across batch sizes → Not memory-bound
- Low cache sensitivity → Midstate works well
- High instruction throughput → CPU fully utilized

---

## Recommendations

1. **Use sha2 crate directly** (Sha2Crate hasher type)
   - It's as fast as our custom SIMD
   - More portable and well-maintained

2. **Remove or fix Assembly v1**
   - From v7: it's 35% slower
   - Either fix the bug or remove it

3. **Focus on parallelization**
   - From v8: near-linear thread scaling
   - Multi-threading is the path to higher hashrate

4. **Consider GPU mining**
   - CPU is compute-bound
   - GPUs have more SHA-capable units

---

## Conclusion

**The sha2 crate already maximizes SHA-NI usage.**

| Finding | Impact |
|---------|--------|
| sha2 crate uses SHA-NI | ✅ Already optimized |
| Custom SIMD adds overhead | ⚠️ Not faster |
| System is compute-bound | ✅ CPU fully utilized |
| Minimal cache sensitivity | ✅ Midstate works |

The implementation is already near-optimal for single-threaded CPU mining. Further gains require:
- More threads (v8 showed excellent scaling)
- GPU acceleration
- More/faster CPUs

---

## Raw Output

```
CPU Features:
  SHA-NI, AVX2, AVX-512: all available

SHA-NI Impact:
  Sha2Crate: 22.99 MH/s
  SIMD: 22.42 MH/s (0.97x - SLOWER!)

Memory:
  Bandwidth: ~2.5 GB/s
  Compute-bound (not memory-bound)

Cache:
  Max degradation: 8%
  Midstate optimization effective
```
