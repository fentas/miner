# Research Test Log v6: Implementation-Level Optimization Analysis

**Date:** 2026-02-01
**Focus:** Implementation optimization, not algorithmic shortcuts
**Goal:** Find bottlenecks in SHA-256 implementation that can be improved
**Test file:** `tests/research_v6_test.rs`

---

## Background

After v1-v5 proved no algorithmic shortcuts exist, v6 explores **implementation-level** optimizations. Just as hardware continues to improve through better engineering, software implementations can be optimized.

---

## Results

### Test 1: Batch Size Analysis

| Batch Size | Rate (MH/s) | Relative |
|------------|-------------|----------|
| 1          | 8.61        | 1.00x    |
| 4          | 8.46        | 0.98x    |
| 8          | 8.60        | 1.00x    |

**Interpretation:** Simple sequential batching provides no benefit. True SIMD parallelism required for batch speedup.

---

### Test 2: Memory Access Patterns ⚡

| Pattern | Rate (MH/s) | Relative |
|---------|-------------|----------|
| Cold (generate each time) | 7.88  | 1.00x |
| Warm (cached headers)     | 13.14 | 1.67x |
| **Hot (modify in place)** | **14.06** | **1.79x** |

**KEY FINDING:** Memory access pattern provides **1.79x speedup**!

Keeping the header in cache and only modifying the nonce bytes is significantly faster than regenerating headers.

---

### Test 3: Nonce Increment Strategy

| Strategy | Rate (MH/s) | Relative |
|----------|-------------|----------|
| Sequential    | 13.72 | 1.00x |
| Large jumps   | 13.14 | 0.96x |
| Random        | 13.95 | 1.02x |

**Interpretation:** Nonce strategy doesn't matter. Branch prediction handles all patterns equally well.

---

### Test 4: SHA2 Crate Usage Patterns ⚡⚡

| Method | Rate (MH/s) | Relative |
|--------|-------------|----------|
| New hasher each time | 23.91 | 1.00x |
| **Reuse with reset** | **57.65** | **2.41x** |
| Chained digest()     | 23.44 | 0.98x |

**KEY FINDING:** Reusing hasher with `finalize_reset()` is **2.41x faster**!

Creating a new `Sha256::new()` each hash has significant overhead. Reusing the hasher object and calling `finalize_reset()` between hashes dramatically improves throughput.

---

### Test 5: Instruction Overhead

| Operation | Rate (MH/s) | Overhead |
|-----------|-------------|----------|
| Hash only           | 13.99 | 0.0%  |
| + Target comparison | 13.03 | 6.9%  |
| + Leading zero count| 14.19 | -1.4% |

**Interpretation:** Target comparison adds ~7% overhead. Leading zero counting is essentially free (within noise). The comparison operation itself isn't expensive, but branch prediction effects matter.

---

### Test 6: Parallel Stream Simulation

| Streams | Rate (MH/s) | Relative |
|---------|-------------|----------|
| 1       | 8.83        | 1.00x    |
| 2       | 8.37        | 0.95x    |
| 4       | 7.24        | 0.82x    |

**Interpretation:** Sequential interleaving of multiple streams adds overhead. This is expected - true SIMD processes multiple hashes in parallel at the instruction level, not by interleaving separate calls.

---

## Analysis

### Actionable Optimizations Found

1. **Hasher Reuse: 2.41x improvement available**
   - Instead of: `Sha256::digest(&data)`
   - Use: Persistent hasher with `finalize_reset()`
   - Check if current implementation uses this pattern

2. **Memory Pattern: 1.79x improvement available**
   - Keep header template in hot cache
   - Only modify nonce bytes in place
   - Avoid regenerating full header

3. **Comparison Overhead: ~7%**
   - Target comparison adds measurable overhead
   - Consider: batch hash then batch compare?
   - Or: optimize comparison to early-exit on first byte

### Non-Optimizations (No Improvement)

- Batch size (without true SIMD)
- Nonce strategy (sequential vs random)
- Stream interleaving (adds overhead)

---

## Comparison with Current Implementation

Need to verify if current hasher implementation:
1. Reuses hasher objects (2.41x potential)
2. Modifies header in place (1.79x potential)
3. Uses efficient target comparison

If not, **combined potential: 2.41 × 1.79 = 4.3x improvement possible!**

---

## Recommendations

1. **Audit current hasher for object reuse**
   - Look at `src/mining/hasher.rs`
   - Check if `Sha256::new()` called per hash or reused

2. **Audit header generation**
   - Look at `hash_nonce_range` function
   - Check if header regenerated or modified in place

3. **Consider batch comparison**
   - Hash N nonces, then compare all N
   - May improve instruction cache behavior

---

## Conclusion

**POSITIVE RESULT:** Implementation-level optimizations exist!

| Optimization | Potential Speedup |
|--------------|-------------------|
| Hasher reuse | 2.41x |
| Memory pattern | 1.79x |
| Combined (theoretical) | 4.3x |

Unlike algorithmic shortcuts (which don't exist), implementation optimizations are real and measurable. Further investigation of the actual hasher code is warranted.

---

## Next Steps

1. Review current hasher implementation
2. Check for hasher object reuse
3. Check memory access patterns
4. Profile with `perf` or similar tool
5. Compare against theoretical maximum

---

## Raw Output

```
SHA2 Crate Variants:
  New hasher each time: 23.91 MH/s
  Reuse with reset: 57.65 MH/s (2.41x FASTER)
  Chained digest(): 23.44 MH/s

Memory Patterns:
  Cold: 7.88 MH/s
  Warm: 13.14 MH/s (1.67x)
  Hot: 14.06 MH/s (1.79x FASTER)

Instruction Overhead:
  Target comparison: 6.9% overhead
```
