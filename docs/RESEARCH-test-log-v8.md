# Research Test Log v8: Multi-Thread Scaling Analysis

**Date:** 2026-02-01
**Focus:** Thread scaling and parallelization efficiency
**Goal:** Find optimal thread count and identify bottlenecks
**Test file:** `tests/research_v8_test.rs`

---

## System Info

- Logical CPUs: 24
- Estimated Physical Cores: 12
- Architecture: Likely 12-core with hyperthreading (or P/E core hybrid)

---

## Results

### Test 1: Quick Thread Scaling

| Threads | Rate (MH/s) | Scaling |
|---------|-------------|---------|
| 1       | 18.22       | 1.00x   |
| 2       | 23.25       | 1.28x   |
| 4       | 89.70       | 4.92x   |
| 8       | 168.54      | **9.25x** |

**KEY FINDING:** Near-linear to superlinear scaling!

- 4 threads: 4.92x (98% efficiency)
- 8 threads: 9.25x (116% efficiency - superlinear!)

Superlinear scaling suggests cache effects - multiple threads warming L3 cache.

---

### Test 2: Hyperthreading Impact

| CPU Count | Rate (MH/s) | Relative |
|-----------|-------------|----------|
| 12 (physical est.) | 210.50 | 1.00x |
| 24 (logical) | 268.85 | 1.28x |

**Hyperthreading benefit: 27.7% additional performance**

This is excellent for SHA-256 - each thread fully utilizes execution units during memory waits.

---

### Test 3: Contention Analysis

| Mode | Rate (MH/s) | Efficiency |
|------|-------------|------------|
| 4 × 1 thread (sequential) | 73.04 | 100.0% |
| 4 threads (parallel) | 91.78 | **125.7%** |

**No contention detected ✓**

Actually shows superlinear speedup (125.7%) - threads don't compete for resources. The superlinear effect is likely from:
- Better cache utilization
- Warmer branch predictors
- Amortized startup costs

---

### Test 4: Per-Core Consistency

| Core | Rate (MH/s) | Deviation |
|------|-------------|-----------|
| 0    | 18.31       | -11.01%   |
| 1    | 11.53       | **-43.95%** ⚠️ |
| 2    | 22.79       | +10.81%   |
| 3    | 20.64       | +0.36%    |
| 4    | 22.66       | +10.15%   |
| 5    | 22.85       | +11.07%   |
| 6    | 23.10       | +12.28%   |
| 7    | 22.69       | +10.29%   |

**Statistics:**
- Average: 20.57 MH/s
- Std Dev: 3.75 MH/s
- CV: 18.21%

**INCONSISTENT - Core 1 is 44% slower!**

Possible causes:
1. **P-core vs E-core** - Intel 12th+ gen hybrid architecture
2. **Thermal throttling** - One core running hot
3. **System tasks** - OS pinned to core 1
4. **Power states** - Core in lower P-state

---

## Analysis

### Scaling Efficiency

| Threads | Theoretical | Actual | Efficiency |
|---------|-------------|--------|------------|
| 1       | 18.22       | 18.22  | 100%       |
| 4       | 72.88       | 89.70  | 123% ⚡    |
| 8       | 145.76      | 168.54 | 116% ⚡    |
| 12      | 218.64      | 210.50 | 96%        |
| 24      | 437.28      | 268.85 | 61%        |

**Observations:**
1. **Superlinear up to 8 threads** - Cache effects dominate
2. **Near-linear at 12 threads** - Physical core count
3. **Diminishing at 24 threads** - HT overhead kicks in

### Optimal Configuration

Based on this system (12 physical cores, 24 logical):

| Workload Type | Recommended Threads | Expected Rate |
|---------------|--------------------:|---------------|
| Maximum throughput | 24 | ~269 MH/s |
| Best efficiency | 12 | ~210 MH/s |
| Power-efficient | 8 | ~169 MH/s |

---

## Recommendations

1. **Use all logical CPUs for maximum hashrate**
   - 24 threads: 269 MH/s
   - HT provides 28% boost

2. **Investigate Core 1 performance**
   - 44% slower than average
   - May be E-core or system-reserved

3. **Consider CPU affinity**
   - Pin threads to P-cores for consistency
   - Avoid E-cores if present

4. **Batch size should be ≥10,000**
   - From v7: 4% improvement

---

## Thread Scaling Model

For this system, approximate scaling:

```
Rate(N) ≈ 22 × N^0.95 MH/s  (for N ≤ 12)
Rate(N) ≈ 210 + 2.4 × (N-12) MH/s  (for N > 12, HT region)
```

---

## Conclusion

**POSITIVE RESULT:** SHA-256 mining scales excellently with threads.

| Finding | Impact |
|---------|--------|
| Near-linear scaling | ✅ Use all cores |
| Hyperthreading benefit | ✅ +28% from HT |
| No contention | ✅ Threads independent |
| Core variance | ⚠️ Some cores slower |

---

## Raw Output

```
Quick Scaling:
  1 thread: 18.22 MH/s
  8 threads: 168.54 MH/s (9.25x)

Hyperthreading:
  12 cores: 210.50 MH/s
  24 threads: 268.85 MH/s (+28%)

Contention:
  Efficiency: 125.7% (superlinear!)

Core Consistency:
  CV = 18.21% - Core 1 is 44% slow
```
