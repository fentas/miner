# Research Test Log v12: Profile-Guided Optimization (PGO)

**Date:** 2026-02-01
**Focus:** PGO implementation and performance testing
**Goal:** Determine if PGO provides hashrate improvement
**Script:** `scripts/build-pgo.sh`

---

## Background

Profile-Guided Optimization (PGO) uses runtime profiling data to guide compiler optimizations:
- Branch prediction hints
- Code layout for better cache locality
- Function inlining decisions
- Hot/cold code separation

Expected benefit: 5-15% for compute-intensive workloads.

---

## Implementation

### Build Script

Created `scripts/build-pgo.sh`:

```bash
# Step 1: Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo" cargo build --release

# Step 2: Run training workload (60s benchmark)
./target/release/btcminer --benchmark --threads 1

# Step 3: Merge profiles
llvm-profdata merge -o merged.profdata /tmp/pgo

# Step 4: Build with profile
RUSTFLAGS="-Cprofile-use=merged.profdata" cargo build --release
```

### Requirements

- `llvm-tools-preview` component: `rustup component add llvm-tools-preview`
- Or system LLVM: `apt install llvm`

---

## Results

### Instrumented Build (Training Phase)

| Hasher | Rate (MH/s) | Notes |
|--------|-------------|-------|
| SIMD | 22.56 | Baseline |
| SHA2 Crate | **7.55** | 3x slower! |

**Key Finding:** Instrumentation overhead affects SHA2 Crate more than SIMD.
This suggests cross-compilation-unit inlining issues.

### PGO-Optimized Build

| Hasher | Rate (MH/s) | vs Baseline |
|--------|-------------|-------------|
| SIMD | 22.89 | +0.3% |
| SHA2 Crate | 22.84 | -0.5% |

**PGO did NOT improve performance significantly.**

### Comparison Summary

| Build Type | SIMD (MH/s) | SHA2 (MH/s) |
|------------|-------------|-------------|
| Baseline (no PGO) | ~23.0 | ~23.0 |
| Instrumented | 22.56 | 7.55 |
| PGO-Optimized | 22.89 | 22.84 |

---

## Analysis

### Why PGO Didn't Help

1. **Already at hardware limits**
   - SHA-NI instructions are the bottleneck
   - CPU is compute-bound, not cache-bound (v9, v10)
   - 267% utilization of theoretical CPU ops (v11)

2. **No significant branching**
   - SHA-256 is straight-line computation
   - No unpredictable branches to optimize
   - Loop structure is simple and predictable

3. **Code already hot**
   - Mining loop runs millions of times
   - CPU branch predictor already optimized
   - Cache is already warmed

### When PGO WOULD Help

- Code with many branches (parsing, protocol handling)
- Cold start scenarios
- Mixed workloads with varying code paths
- Large codebases with complex call graphs

For tight computational loops like SHA-256, PGO provides minimal benefit.

---

## CAT/QoS Research Summary

Also researched Intel CAT and AMD Platform QoS:

### AMD Platform QoS
- Uses MSRs (L3_MASK_n starting at C90h)
- Linux support via `resctrl` interface
- **No Windows-native API** - focused on Linux/hypervisors
- [AMD QoS PDF Spec](http://kib.kiev.ua/x86docs/AMD/MISC/56375_1.00_PUB.pdf)

### Intel RDT/CAT
- [intel-cmt-cat](https://github.com/intel/intel-cmt-cat) - userspace tools
- Windows: Only RTX64 (real-time extension) supports it
- Standard Windows has no built-in interface
- [Intel RDT Overview](https://www.intel.com/content/www/us/en/architecture-and-technology/resource-director-technology.html)

**Conclusion:** CAT/QoS requires Linux or specialized tools. Not viable for standard Windows, and wouldn't help anyway (compute-bound workload).

---

## Conclusion

**PGO: NEGATIVE RESULT**

| Finding | Impact |
|---------|--------|
| PGO implemented | ✅ Script ready |
| Performance improvement | ❌ None (<1%) |
| SHA-256 is branch-free | ✅ Explains no benefit |
| Hardware limits reached | ✅ Confirmed again |

**Recommendation:** PGO is not worth the build complexity for this workload.
Focus optimization efforts on:
1. Multi-threading (v8: 9.25x scaling)
2. GPU acceleration (future work)

---

## Raw Output

```
Baseline (no PGO):
  SIMD: ~23.0 MH/s
  SHA2 Crate: ~23.0 MH/s

Instrumented:
  SIMD: 22.56 MH/s (normal)
  SHA2 Crate: 7.55 MH/s (3x slower - overhead)

PGO-Optimized:
  SIMD: 22.89 MH/s (+0.3%)
  SHA2 Crate: 22.84 MH/s (-0.5%)

Verdict: PGO provides NO benefit for SHA-256 mining
```
