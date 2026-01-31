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

## Theories to Test Next

1. **Library hash_header() has magic optimization** - Maybe something about how Hasher struct works
2. **Test binary compilation differs** - Different compiler flags for tests vs library
3. **Worker thread spawning affects optimization** - Something about std::thread::spawn
4. **Loop structure matters** - Saturating_add vs wrapping_add, range types

---

## Next Steps

1. Create test that matches library code exactly (hash_header style)
2. Profile both tests to see where time is spent
3. Check compiler flags for test binaries
4. Check if `#[inline]` attributes make a difference

---

## Files Created

- `tests/standalone_sha2_bench.rs` - Direct sha2 crate test (slow: 117 MH/s)
- `tests/coordinator_sha2_bench.rs` - Library Coordinator test (fast: 76 GH/s)
- `docs/performance-investigation.md` - Full theory documentation
- `docs/performance-test-plan.md` - Test plan with 10 tests
- `docs/perf-results/investigation-log.md` - This file

---

**Status**: INVESTIGATING - Found that Coordinator matches production, so cross-crate is not the issue. Need to understand why standalone test is 650x slower.
