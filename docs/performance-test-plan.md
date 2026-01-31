# Performance Test Plan: Proving/Disproving Theories

This document describes specific tests to validate or invalidate each theory from the performance investigation.

---

## Test 1: Isolate Cross-Crate Boundary Effect

### Purpose
Determine if cross-crate boundary alone explains the sha2 crate slowdown in tests.

### Method
Create a standalone binary that uses sha2 crate directly (not through btcminer library).

### Implementation

```bash
# Create test binary
cat > /tmp/sha2_direct_test.rs << 'EOF'
use sha2::{Sha256, Digest};
use std::time::{Duration, Instant};

fn main() {
    let duration = Duration::from_secs(10);
    let start = Instant::now();
    let mut total = 0u64;
    let mut header = [0u8; 80];

    while start.elapsed() < duration {
        for nonce in 0u32..262144 {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());

            // Double SHA-256
            let first = Sha256::digest(&header);
            let _hash = Sha256::digest(&first);

            total += 1;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = total as f64 / elapsed;
    println!("Direct sha2 crate: {:.2} MH/s", rate / 1e6);
}
EOF

# Compile with same flags as btcminer
rustc -O --edition 2021 -C lto=fat -C target-cpu=native /tmp/sha2_direct_test.rs -o /tmp/sha2_direct_test

# Run test
/tmp/sha2_direct_test
```

### Expected Results
- **If Theory 1 (cross-crate) is correct**: Should show ~4 GH/s (matching production)
- **If Theory 1 is wrong**: Will show ~10 MH/s (matching tests)

### Interpretation
- Fast result → Cross-crate boundary is the main factor
- Slow result → Something else is causing sha2 slowness

---

## Test 2: Assembly Output Comparison

### Purpose
Compare generated assembly between sha2 crate and our SIMD implementation.

### Method
Generate assembly output and compare instruction counts, memory operations.

### Implementation

```bash
# Generate assembly for our SIMD mine function
cd /home/fentas/github/fentas/miner
RUSTFLAGS="--emit=asm" cargo build --release 2>/dev/null

# Find the assembly file
find target/release -name "*.s" -exec grep -l "shani::mine" {} \;

# Alternatively, use objdump
objdump -d target/release/btcminer | grep -A 500 "sha256d_midstate" > /tmp/our_simd.asm

# For sha2 crate, we need to find where it's inlined
objdump -d target/release/btcminer | grep -A 500 "sha2.*compress" > /tmp/sha2_asm.asm
```

### What to Look For
1. **Memory operations**: Count `mov`, `lea` instructions
2. **SHA-NI usage**: Look for `sha256rnds2`, `sha256msg1`, `sha256msg2`
3. **Loop structure**: Unrolling, branching
4. **Stack usage**: Stack frame size, spills

### Expected Results
- **sha2 crate**: Minimal memory ops, tight loop
- **Our SIMD**: More memory ops, function calls

---

## Test 3: Profile with perf

### Purpose
Identify where time is actually spent in each implementation.

### Method
Use Linux `perf` to profile both code paths.

### Implementation

```bash
# Profile production binary with sha2 crate
cd /home/fentas/github/fentas/miner
perf record -g ./target/release/btcminer --benchmark --benchmark-duration 30 2>/dev/null &
PID=$!
sleep 35
kill $PID 2>/dev/null
perf report --stdio > /tmp/perf_simd_benchmark.txt

# Profile by modifying config to use sha2
# First change btcminer.toml: hasher = "sha2"
# Then run the actual miner for 30 seconds
timeout 30 ./target/release/btcminer 2>/dev/null &
PID=$!
perf record -g -p $PID
wait $PID
perf report --stdio > /tmp/perf_sha2_production.txt
```

### What to Look For
1. **Hot functions**: Which functions consume most cycles?
2. **Memory stalls**: Cache misses, TLB misses
3. **Branch mispredictions**: In hot loops
4. **Function call overhead**: Time in call/ret

### Expected Results
- **sha2 in production**: Most time in SHA-NI instructions
- **Our SIMD**: Time split between SHA-NI and memory/setup

---

## Test 4: Inline Everything Test

### Purpose
Test if forcing inlining helps our SIMD performance.

### Method
Modify code to use `#[inline(always)]` where possible and compare.

### Implementation

```rust
// In src/mining/sha256_simd/mod.rs, change mine_with_midstate:

#[inline(always)]  // Force inline the dispatch
pub fn mine_with_midstate(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    // REMOVED: SimdLevel::detect() call - assume SHA-NI

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sha") {
            return unsafe { shani::mine(midstate, tail, nonce_start, nonce_count, target) };
        }
    }
    generic::mine(midstate, tail, nonce_start, nonce_count, target)
}
```

Note: Cannot use `#[inline(always)]` on `#[target_feature]` functions.

### Alternative: Move Hot Loop to main.rs

```rust
// In main.rs run_benchmark(), replace mine_with_midstate call with direct code:

while run.load(Ordering::Relaxed) {
    // Inline the hot loop directly here
    for i in 0..batch_size {
        let nonce = nonce.wrapping_add(i);
        // Call sha256d_midstate directly if possible
        // Or inline the entire double-SHA256
    }
}
```

### Expected Results
- **If dispatch overhead is the issue**: Should see improvement
- **If not**: No change

---

## Test 5: Memory Allocation Test

### Purpose
Test if stack allocations are causing slowdown.

### Method
Pre-allocate buffers outside the hot loop.

### Implementation

```rust
// Modified shani::mine with pre-allocated buffers:

#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn mine_preallocated(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    // Pre-allocate ONCE outside loop
    let mut block1 = [0u8; 64];
    let mut block2 = [0u8; 64];

    // Pre-fill static parts
    block1[..16].copy_from_slice(tail);
    block1[20] = 0x80;
    block1[62] = 0x02;
    block1[63] = 0x80;

    block2[32] = 0x80;
    block2[62] = 0x01;

    for i in 0..nonce_count {
        let nonce = nonce_start.wrapping_add(i);

        // Only update nonce (4 bytes)
        block1[16..20].copy_from_slice(&nonce.to_le_bytes());

        // First hash
        let mut state = midstate.state;
        transform(&mut state, &block1);

        // Copy to block2 (could optimize further)
        let first_hash = state_to_bytes(&state);
        block2[..32].copy_from_slice(&first_hash);

        // Second hash
        let mut state = H_INIT;
        transform(&mut state, &block2);
        let hash = state_to_bytes(&state);

        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }
    None
}
```

### Expected Results
- **If allocation is the issue**: Significant improvement
- **If not**: Minimal change

---

## Test 6: sha2 Crate Source Analysis

### Purpose
Understand what optimizations sha2 crate uses.

### Method
Read sha2 crate source code.

### Implementation

```bash
# Clone sha2 crate source
git clone https://github.com/RustCrypto/hashes.git /tmp/rustcrypto-hashes
cd /tmp/rustcrypto-hashes/sha2

# Find SHA-NI implementation
find . -name "*.rs" -exec grep -l "sha256rnds2\|target_feature.*sha" {} \;

# Read the implementation
cat src/sha256/x86.rs  # or similar path
```

### What to Look For
1. **State management**: How is SHA state handled?
2. **Block processing**: Any unrolling or interleaving?
3. **Memory access**: Aligned loads, cache hints?
4. **Loop structure**: Batching, prefetching?

---

## Test 7: Benchmark with sha2 Crate Directly in main.rs

### Purpose
Test sha2 crate performance when called from same compilation unit.

### Method
Add a benchmark mode that uses sha2 crate directly in main.rs.

### Implementation

```rust
// Add to main.rs:

async fn run_benchmark_sha2(duration_secs: u64, thread_count: usize) -> anyhow::Result<()> {
    use sha2::{Sha256, Digest};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    let total_hashes = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let mut handles = Vec::new();
    for _ in 0..thread_count {
        let hashes = Arc::clone(&total_hashes);
        let run = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            let mut header = [0u8; 80];
            let mut nonce = 0u32;

            while run.load(Ordering::Relaxed) {
                for _ in 0..262144 {
                    header[76..80].copy_from_slice(&nonce.to_le_bytes());

                    // Double SHA-256 using sha2 crate directly
                    let first = Sha256::digest(&header);
                    let _hash = Sha256::digest(&first);

                    nonce = nonce.wrapping_add(1);
                }
                hashes.fetch_add(262144, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    // ... timing and reporting code ...
    Ok(())
}
```

### Expected Results
- **If cross-crate is the issue**: Should show ~4 GH/s per thread
- **If sha2 crate itself is slow**: Will show ~10 MH/s

---

## Test 8: Compare with Known-Good Implementation

### Purpose
Benchmark against a known-good Bitcoin mining implementation.

### Method
Use cgminer or similar to establish baseline.

### Implementation

```bash
# Install cgminer (CPU version)
# Or use another known Bitcoin miner

# Run benchmark mode
cgminer --benchmark

# Compare results with our implementation
```

### Expected Results
Establishes ground truth for what hashrate is achievable on this hardware.

---

## Test 9: Single-Threaded vs Multi-Threaded Scaling

### Purpose
Check if performance scales linearly with threads.

### Method
Run benchmarks with 1, 2, 4, 8, 16, 24 threads.

### Implementation

```bash
# Modify config threads and run benchmark
for threads in 1 2 4 8 16 24; do
    echo "Testing with $threads threads..."
    # Would need CLI flag for thread count
    ./target/release/btcminer --benchmark --benchmark-duration 10 --threads $threads
done
```

### Expected Results
- Linear scaling → No contention issues
- Sub-linear scaling → Possible cache/memory contention

---

## Test 10: Verify SHA-NI is Actually Being Used

### Purpose
Confirm SHA-NI instructions are in the generated code.

### Method
Check for SHA-NI instructions in running process.

### Implementation

```bash
# While benchmark is running:
./target/release/btcminer --benchmark --benchmark-duration 60 &
PID=$!

# Check instruction mix with perf
perf stat -e instructions,cycles,r04c5:u -p $PID sleep 10
# r04c5:u = SHA instruction counter (Intel)

# Or disassemble running process
gdb -p $PID -batch -ex "disas shani::mine"

kill $PID
```

### Expected Results
Should see `sha256rnds2`, `sha256msg1`, `sha256msg2` instructions.

---

## Summary: Test Priority

| Test | Effort | Expected Insight |
|------|--------|------------------|
| Test 1: Direct sha2 | Low | Isolates cross-crate effect |
| Test 7: sha2 in main.rs | Low | Confirms cross-crate theory |
| Test 3: perf profile | Medium | Shows where time goes |
| Test 5: Pre-allocate | Medium | Tests memory hypothesis |
| Test 2: Assembly | Medium | Reveals optimization gaps |
| Test 6: sha2 source | Low | Understand best practices |
| Test 4: Force inline | Medium | Tests dispatch overhead |
| Test 10: Verify SHA-NI | Low | Sanity check |
| Test 8: Known-good | High | Ground truth |
| Test 9: Thread scaling | Medium | Contention check |

**Recommended order**: 1, 7, 3, 10, 5, 2, 6

---

## Recording Results

Create a results file for each test:

```bash
mkdir -p /home/fentas/github/fentas/miner/docs/perf-results

# Template for each test:
cat > /home/fentas/github/fentas/miner/docs/perf-results/test-N-results.md << 'EOF'
# Test N: [Name]

## Date
YYYY-MM-DD

## Environment
- CPU:
- OS:
- Rust version:
- Commit:

## Results
[Paste output here]

## Interpretation
[What does this tell us?]

## Theory Status
- [ ] Confirmed
- [ ] Disproved
- [ ] Inconclusive
EOF
```
