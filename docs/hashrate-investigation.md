# Hashrate Performance Investigation

## Problem Statement

**Observed:** ~280-300 MH/s on Windows (native)
**Expected:** 2-3.5 TH/s (single thread), 10-12 TH/s (24 threads)
**Gap:** 10,000x+ slower than expected

## KEY DISCOVERY: Linux Works, Windows Doesn't

### Linux/WSL2 Test Results (24 cores, SHA-NI)

```
=== E2E Diagnostic Results (2026-01-30) ===
SIMD Level: SHA-NI ✓

Isolated Hasher (single-thread):
  Batch 1K:     12.40 GH/s
  Batch 64K:   798.55 GH/s
  Batch 256K:    3.14 TH/s  ← DEFAULT BATCH SIZE
  Batch 1M:    12.78 TH/s

mine_with_midstate (single-thread):
  Batch 256K:    3.51 TH/s  ← EXCELLENT

Multi-threaded (24 cores):
  1 thread:     3.49 TH/s
  12 threads:   8.81 TH/s
  24 threads:  12.49 TH/s  ← EXCELLENT SCALING

Coordinator (real mining path):
  24 threads:  11.78 TH/s  ← REAL-WORLD PERFORMANCE

SIMD detection overhead: 1.23 ns/call ← NOT a bottleneck
```

### Windows Native Test Results (2026-01-30)

**IMPORTANT: Previous tests were WRONG due to early-exit target inflation!**

#### Tests with EASY target `[0xFF; 32]` (INCORRECT):
```
TUI Hashrate with HashrateCalculator (24 threads):
  Calculator rate: 11.32 TH/s  ← FAKE (256,000x inflated)
```

#### Test with HARD target `[0x00; 32]` (CORRECT - matches real mining):
```
=== TUI Hashrate with HARD Target (Real Mining Simulation) ===
Using 24 threads

Final Results (HARD target - no early exit):
  Total hashes:    2,429,812,736
  Elapsed:         10.01s
  Calculator rate: 240.20 MH/s  ← REAL HASHRATE!
  Direct rate:     242.76 MH/s
```

**ROOT CAUSE FOUND: Easy target causes massive hashrate inflation in tests!**

With easy target `[0xFF; 32]`:
- Every single hash matches the target
- `mine_with_midstate()` returns after processing just 1 nonce
- Worker reports `batch_size` (256K) hashes
- Inflation: 256,000x (1 hash → 256K reported)

With hard target (real Bitcoin mining):
- Almost no hashes match the target
- All 256K nonces are processed per batch
- Accurate hash count

**The TUI showing 240-280 MH/s is CORRECT!**
This is approximately 10 MH/s per thread with SHA-NI sequential processing.

---

## Investigation: What's Different on Windows?

### Hypothesis 1: SIMD Not Detected on Windows

The most likely cause is `is_x86_feature_detected!("sha")` returning `false` on Windows, causing fallback to generic implementation.

**To verify:** Run the diagnostic on Windows and check reported SIMD level.

### Hypothesis 2: MSVC Compiler Optimizations

The SIMD intrinsics might not be compiled correctly with MSVC vs GCC/Clang.

**To verify:** Check if the Windows binary was built with correct target features.

### Hypothesis 3: CPU Feature Detection Bug on Windows

The Rust std library's feature detection may have a bug on Windows for SHA-NI.

**To verify:** Compare `is_x86_feature_detected!("sha")` output on both platforms.

---

## Diagnostic Tests

Run on Windows native to identify the problem:

```bash
# Full diagnostic (all tests)
cargo test --release e2e_full_diagnostic -- --nocapture --ignored

# Quick check - just SIMD level
cargo test --release e2e_01_simd_detection_overhead -- --nocapture --ignored
cargo test --release e2e_02_isolated_hasher_performance -- --nocapture --ignored
```

### Expected Output

**If SHA-NI is working (expected on Windows):**
```
SIMD Level: SHA-NI
  Batch 256K (default): 3.XX TH/s
```

**If falling back to generic (THIS IS THE BUG):**
```
SIMD Level: generic
  Batch 256K (default): 1-2 MH/s
```

**If SHA-NI detected but slow (different bug):**
```
SIMD Level: SHA-NI
  Batch 256K (default): 280 MH/s  <-- Something else is wrong
```

---

## Code Analysis

### SIMD Detection (may be the issue on Windows)

**Location:** `src/mining/sha256_simd/mod.rs:56-70`

```rust
impl SimdLevel {
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sha") && is_x86_feature_detected!("sse4.1") {
                return SimdLevel::ShaNi;  // <-- Does this work on Windows?
            }
            // ...
        }
        SimdLevel::Generic
    }
}
```

### Mining Hot Path

```
Worker::run()
  └── loop
        └── hasher.hash_nonce_range()              // src/mining/hasher.rs:66
              └── mine_with_midstate()             // src/mining/sha256_simd/mod.rs:113
                    ├── SimdLevel::detect()        // Fast (1.23ns) - NOT the issue
                    └── shani::mine()              // Should be called if SHA-NI detected
```

---

## Proposed Fixes

### Fix 1: Force SIMD Level via Environment Variable

Add ability to override SIMD detection for debugging:

```rust
impl SimdLevel {
    pub fn detect() -> Self {
        // Allow override for debugging
        if let Ok(level) = std::env::var("BTCMINER_SIMD") {
            return match level.to_lowercase().as_str() {
                "sha-ni" | "shani" => SimdLevel::ShaNi,
                "avx512" => SimdLevel::Avx512,
                "avx2" => SimdLevel::Avx2,
                _ => SimdLevel::Generic,
            };
        }
        // Normal detection...
    }
}
```

### Fix 2: Explicit Target Features for Windows

In `Cargo.toml` or `.cargo/config.toml`:

```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+sha,+sse4.1"]
```

### Fix 3: Use cpuid crate instead of std

The `cpuid` crate may be more reliable across platforms:

```rust
use raw_cpuid::CpuId;

fn detect_sha_extension() -> bool {
    let cpuid = CpuId::new();
    cpuid.get_extended_feature_info()
        .map(|info| info.has_sha())
        .unwrap_or(false)
}
```

---

## Test Matrix

| Test | Easy Target (WRONG) | Hard Target (CORRECT) | Notes |
|------|---------------------|----------------------|-------|
| 24-thread hashrate | 11.32 TH/s | 240 MH/s | Easy target inflates by 256,000x |
| Per-thread | ~470 GH/s | ~10 MH/s | Real SHA-NI sequential performance |
| SIMD detection | SHA-NI | SHA-NI | Working correctly |
| HashrateCalculator | 0.0% error | 0.0% error | Working correctly |

---

## Action Items

1. [x] Run `e2e_full_diagnostic` on Linux - **WORKING: 12 TH/s**
2. [x] Run `e2e_full_diagnostic` on Windows native - **WORKING: 11+ TH/s**
3. [x] Check SIMD level reported on Windows - **SHA-NI detected ✓**
4. [x] Run TUI hashrate tests on Windows - **11.18 TH/s ✓**
5. [x] Add Windows-specific CI test for hashrate regression
6. [ ] Investigate actual application run mode showing 280 MH/s
7. [ ] Check if idle mode is enabled
8. [ ] Verify thread count in actual run

---

## Related Files

| File | Purpose |
|------|---------|
| `src/mining/sha256_simd/mod.rs` | SIMD detection and dispatch |
| `src/mining/sha256_simd/shani.rs` | SHA-NI implementation |
| `src/mining/sha256_simd/avx2.rs` | AVX2 fallback |
| `src/mining/sha256_simd/generic.rs` | Generic fallback |
| `tests/e2e_hashrate_benchmark.rs` | E2E diagnostic tests |
| `benches/mining_benchmarks.rs` | Criterion benchmarks |

---

## History

| Date | Event | Platform | Result |
|------|-------|----------|--------|
| Before optimizations | Initial implementation | - | 20 GH/s (inflated) |
| SHA-NI added | Performance documented | Linux | 857 GH/s (inflated) |
| 2026-01-30 | User report (TUI display) | Windows native | **280 MH/s** |
| 2026-01-30 | Easy target test (WRONG) | Windows native | 11.32 TH/s (256Kx inflated) |
| 2026-01-30 | **Hard target test (CORRECT)** | Windows native | **240 MH/s** ✓ |

**Conclusion: All previous "high" hashrate measurements were inflated by using easy targets that cause early exit. The real SHA-NI sequential hashrate is ~10 MH/s per thread.**
