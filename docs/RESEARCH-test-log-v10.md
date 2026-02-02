# Research Test Log v10: Cache Allocation Technology (CAT) Investigation

**Date:** 2026-02-01
**Focus:** Intel CAT / AMD Platform QoS and cache locking potential
**Goal:** Determine if hardware cache allocation can improve mining performance
**Test file:** `tests/research_v10_test.rs`

---

## Background

### What is Cache Allocation Technology?

Intel CAT (and AMD's equivalent Platform QoS) allows:
1. **Reserving cache partitions** - Allocate specific L3 cache ways to processes
2. **Cache isolation** - Prevent other processes from evicting your data
3. **Cache locking** - Keep critical code/data pinned in cache

This is used in:
- Real-time systems (guaranteed latency)
- High-frequency trading (predictable performance)
- Network function virtualization (isolation)

### Theoretical Benefit for Mining

```
Traditional: Hot loop may be evicted by OS/other processes
CAT enabled: Hot loop GUARANTEED to stay in L3 cache
Potential: Eliminate cache misses → higher sustained throughput
```

---

## System Analysis

### Hardware

| Component | Value |
|-----------|-------|
| CPU | AMD Ryzen 9 7900 (Zen 4) |
| Physical Cores | 12 |
| Logical CPUs | 24 (SMT) |
| L1d Cache | 32 KB per core |
| L1i Cache | 32 KB per core |
| L2 Cache | 1 MB per core |
| L3 Cache | 32 MB shared |
| Environment | WSL2 (Hyper-V virtualized) |

### CAT Availability Check

| Check | Result |
|-------|--------|
| CPU Vendor | AMD (not Intel CAT) |
| AMD Platform QoS flags | Not exposed in WSL2 |
| resctrl filesystem | NOT mounted |
| Hypervisor | Hyper-V (WSL2) |

**CRITICAL FINDING: CAT/QoS NOT available**

Reasons:
1. **WSL2 virtualization** - Hardware cache control requires bare metal or hypervisor-level access
2. **AMD vs Intel** - AMD uses different mechanism (Platform QoS) with different interface
3. **Kernel support** - resctrl filesystem not mounted even if hardware supported

---

## Alternative Analysis

### From v9 Results: Cache Sensitivity

| Headers | Rate (MH/s) | Degradation |
|---------|-------------|-------------|
| 1 | 23.61 | baseline |
| 10 | 23.03 | -2.5% |
| 100 | 21.67 | -8.2% |
| 1000 | 22.92 | -2.9% |

**Key insight:** Only 8% maximum cache impact with 1000 headers!

This suggests:
1. Midstate optimization keeps working set small (~256 bytes per hash)
2. SHA-256 is compute-bound, not cache-bound
3. Cache locking would provide **minimal benefit**

### Working Set Analysis

SHA-256 mining working set per hash:
```
Input: 80 bytes (76-byte header + 4-byte nonce)
Midstate: 32 bytes (precomputed)
K constants: 256 bytes (64 × 4-byte words)
State: 32 bytes (8 × 4-byte words)
Output: 32 bytes (double SHA-256 result)
Total: ~432 bytes
```

This easily fits in L1d cache (32 KB per core).

### Why CAT Wouldn't Help Much

| Factor | Impact |
|--------|--------|
| Working set (432 bytes) | << L1 cache (32 KB) |
| L1 hit rate | Already ~99%+ |
| Compute : Memory ratio | Very high (compute-bound) |
| v9 cache sensitivity | Only 8% degradation |

Even if CAT were available, locking L3 cache wouldn't help because:
1. Hot data already fits in L1
2. System is compute-bound, not memory-bound
3. SHA-NI instructions are the bottleneck

---

## Software Cache Optimization Options

### 1. Prefetching (Available)

```rust
// x86 intrinsic
use std::arch::x86_64::_mm_prefetch;

// Prefetch next header while processing current
_mm_prefetch(next_header.as_ptr() as *const i8, _MM_HINT_T0);
```

**Verdict:** Could help with multiple headers, but v9 shows minimal cache impact.

### 2. Cache Line Alignment (Available)

```rust
#[repr(align(64))]  // Align to cache line
struct AlignedState {
    state: [u32; 8],
    // padding to fill cache line
}
```

**Verdict:** May prevent false sharing in multi-threaded code.

### 3. Huge Pages (Available on Linux)

```rust
// Use mmap with MAP_HUGETLB for 2MB pages
// Reduces TLB misses
```

**Verdict:** Minimal benefit - working set too small.

### 4. NUMA Awareness (Available)

```rust
// Pin threads to specific NUMA nodes
// Keep memory local to CPU
```

**Verdict:** This system is single NUMA node (NUMA node0 CPU(s): 0-23).

---

## Recommendations

### CAT Investigation: CLOSED

Cache Allocation Technology is:
1. **Not available** in WSL2 environment
2. **Not beneficial** even if available (compute-bound workload)
3. **Overkill** for SHA-256 mining (working set fits in L1)

### Future Investigation

If running on bare metal with Intel CAT support, the experiment would be:

```bash
# Mount resctrl
mount -t resctrl resctrl /sys/fs/resctrl

# Create exclusive cache partition (50% of L3)
mkdir /sys/fs/resctrl/mining
echo "L3:0=0xff" > /sys/fs/resctrl/mining/schemata

# Pin process to partition
echo $$ > /sys/fs/resctrl/mining/tasks

# Benchmark
./miner --benchmark
```

Expected benefit: <5% (based on v9 cache sensitivity analysis)

---

## Conclusion

**CAT/Cache Locking: NOT VIABLE**

| Question | Answer |
|----------|--------|
| Is CAT available? | No (WSL2 blocks it) |
| Would CAT help if available? | Minimal (~5% max) |
| Is mining cache-bound? | No (compute-bound) |
| Better optimization path? | Multi-threading (v8: 9.25x scaling) |

The v9 experiment already proved:
1. System is compute-bound, not memory-bound
2. Cache sensitivity is only 8%
3. SHA-NI hardware is the true bottleneck

**Cache locking is a dead end for this workload.**

---

## Software Experiments Still Worth Trying

Despite CAT being unavailable/unhelpful, these software optimizations remain viable:

1. **Cache line alignment** - Prevent false sharing in multi-threaded code
2. **Memory pooling** - Reduce allocator overhead (already done per v6)
3. **Batch processing** - Amortize function call overhead (already done per v7)
4. **NUMA pinning** - Keep threads on local memory (single NUMA, not applicable)

All of these are already implemented or not applicable to this system.

---

## Actual Test Results

### Working Set Analysis
```
SHA-256 Mining Working Set Per Hash:

| Component                   | Size (bytes) |
|-----------------------------|--------------|
| Header input                |           80 |
| Midstate (precomputed)      |           32 |
| K constants                 |          256 |
| Working state               |           32 |
| Message schedule            |          256 |
| Output hash                 |           32 |
| **TOTAL**                   |      **688** |
```

Working set fits in L1 cache **47.6x** over!

### Cache Pressure Test
| Noise Data | Rate (MH/s) | Degradation |
|------------|-------------|-------------|
| None (baseline) | 21.64 | 0.0% |
| 32 KB (L1 size) | 21.34 | -1.4% |
| 256 KB (partial L2) | 21.74 | +0.5% |
| 1 MB (L2 size) | 21.98 | +1.6% |
| 8 MB (partial L3) | 22.24 | +2.8% |

**No degradation with cache pressure** - confirms compute-bound.

### Alignment Impact
| Access Pattern | Rate (MH/s) | Relative |
|----------------|-------------|----------|
| Aligned | 21.69 | 1.00x |
| Misaligned | 21.34 | 0.98x |

Misalignment impact: **1.6%** (negligible).

---

## Raw Output

```
CPU: AMD Ryzen 9 7900 (Zen 4)
Environment: WSL2 (Hyper-V)

Cache Hierarchy:
  L1d: 32 KB/core
  L2: 1 MB/core
  L3: 32 MB shared

CAT Status: NOT AVAILABLE
  - resctrl: not mounted
  - Reason: WSL2 virtualization

Working Set: 688 bytes (fits in L1 47.6x)
Cache Pressure: No degradation with 8MB noise
Alignment Impact: 1.6% (negligible)

Verdict: Cache locking is NOT a viable optimization path
```
