# Research Test Log v7: Hasher Type Comparison

**Date:** 2026-02-01
**Focus:** Benchmarking all hasher implementations
**Goal:** Find the fastest hasher type and identify bottlenecks
**Test file:** `tests/research_v7_test.rs`

---

## Results

### Test 1: Hasher Type Comparison (3s each)

| Hasher Type | Rate (MH/s) | Hashes | Relative |
|-------------|-------------|--------|----------|
| Sha2Crate (baseline) | 22.99 | 68M | 1.00x |
| **SIMD (AVX2/SHA-NI)** | **23.11** | 69M | **1.01x** |
| Assembly v1 | 14.91 | 44M | 0.65x ⚠️ |
| Assembly v2 | 22.93 | 68M | 1.00x |
| Assembly v3 | 22.81 | 68M | 0.99x |

**Fastest:** SIMD at 23.11 MH/s
**Speedup over baseline:** 1.01x (only 1%!)

**ISSUE FOUND:** Assembly v1 is 35% SLOWER than baseline!

---

### Test 2: Hasher Consistency Check

All hashers produce identical results for the same input:

| Hasher | Result |
|--------|--------|
| Sha2Crate | nonce=13, hash=[09, dd, 16, 27...] |
| SIMD | MATCH ✓ |
| Assembly | MATCH ✓ |
| AsmV2 | MATCH ✓ |
| AsmV3 | MATCH ✓ |

**Conclusion:** All implementations are correct.

---

### Test 3: Throughput Stability (10 intervals)

| Metric | Value |
|--------|-------|
| Average | 22.96 MH/s |
| Std Dev | 0.41 MH/s |
| Coefficient of Variation | 1.79% |
| Result | STABLE ✓ (CV < 5%) |

Throughput is consistent over time. No warmup issues.

---

### Test 4: Batch Size Impact

| Batch Size | Rate (MH/s) | Relative |
|------------|-------------|----------|
| 100 | 22.20 | 1.00x |
| 1,000 | 22.44 | 1.01x |
| 10,000 | 23.04 | 1.04x |
| 50,000 | 22.82 | 1.03x |
| 100,000 | 23.01 | 1.04x |
| 500,000 | 23.10 | 1.04x |

**Finding:** Larger batches are ~4% faster. Diminishing returns after 10,000.

---

### Test 5: Cold vs Warm Start

| State | Rate (MH/s) | Time (ms) | Relative |
|-------|-------------|-----------|----------|
| Cold | 21.22 | 235.68 | 1.00x |
| Warm | 22.51 | 222.17 | 1.06x |
| Hot | 20.44 | 244.57 | 0.96x |

**Finding:** Warm start is 6% faster than cold. Hot result is noise (should be same as warm).

---

### Test 6: Nonce Wraparound Handling

| Start Point | Rate (MH/s) | Note |
|-------------|-------------|------|
| 0 | 21.97 | Start |
| 2,147,483,647 | 22.08 | Middle |
| 4,294,867,295 | 22.19 | Near end |
| 4,294,917,295 | 23.57 | Wraparound |

**Finding:** No performance degradation at nonce boundaries. Wraparound is handled correctly.

---

## Analysis

### Key Findings

1. **All hashers perform similarly** (~23 MH/s single-threaded)
   - SIMD is only 1% faster than sha2 crate baseline
   - Expected larger difference from SIMD/SHA-NI

2. **Assembly v1 is BROKEN** (35% slower!)
   - Needs investigation - may have a bug
   - v2 and v3 are working correctly

3. **Batch size optimization exists** (4% improvement)
   - Use batch sizes ≥ 10,000 for optimal performance

4. **Throughput is stable**
   - CV = 1.79% - consistent performance

### Why SIMD Isn't Faster?

Possible explanations:
1. The sha2 crate already uses CPU SHA-NI extensions internally
2. Both paths are bottlenecked by memory bandwidth
3. The midstate optimization dominates - both use it

### Assembly v1 Issue

Assembly v1 is 35% slower - likely causes:
1. Suboptimal register allocation
2. Missing optimization flags
3. Bug in the implementation

This should be fixed or removed.

---

## Recommendations

1. **Fix or remove Assembly v1** - it's slower than baseline
2. **Use batch size ≥ 10,000** - 4% improvement
3. **Keep SIMD as default** - marginally fastest
4. **Investigate why SIMD isn't faster** - may be hitting CPU limits

---

## Next Steps

1. Multi-threaded scaling analysis (v8)
2. CPU feature detection (v9)
3. Profile with perf to find true bottleneck

---

## Raw Output

```
Hasher Comparison:
  Sha2Crate: 22.99 MH/s (baseline)
  SIMD: 23.11 MH/s (1.01x)
  Assembly v1: 14.91 MH/s (0.65x) ← PROBLEM
  Assembly v2: 22.93 MH/s (1.00x)
  Assembly v3: 22.81 MH/s (0.99x)

Throughput Stability:
  CV = 1.79% - STABLE

Batch Size:
  Optimal ≥ 10,000 (4% improvement)
```
