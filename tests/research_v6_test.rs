//! Research Experiment v6: Implementation-Level Optimization Analysis
//!
//! Profiles different SHA-256 implementation strategies to find bottlenecks.
//! Tests SIMD widths, batch sizes, memory patterns, and instruction mix.
//!
//! Correlates to: docs/RESEARCH-test-log-v6.md
//! Run with: cargo test --release --test research_v6_test -- --nocapture

use std::time::{Instant, Duration};
use sha2::{Sha256, Digest};

/// Simple SHA-256 double hash
fn sha256d_simple(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Generate test header
fn make_header(seed: u64, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    header
}

/// Measure hash rate for a given duration
fn measure_hashrate<F>(name: &str, duration_secs: u64, mut hash_fn: F) -> f64
where
    F: FnMut(u32) -> [u8; 32],
{
    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut count = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        // Batch of 1000 to reduce timing overhead
        for _ in 0..1000 {
            let _ = hash_fn(nonce);
            nonce = nonce.wrapping_add(1);
        }
        count += 1000;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let hashrate = count as f64 / elapsed / 1_000_000.0;

    println!("  {}: {:.2} MH/s ({} hashes in {:.2}s)", name, hashrate, count, elapsed);
    hashrate
}

/// Test 1: Batch Size Analysis
/// Does processing multiple nonces together improve throughput?
#[test]
fn test_research_v6_batch_size() {
    println!("\n=== Research v6: Batch Size Analysis ===\n");
    println!("Question: Does batching multiple nonces improve throughput?\n");

    let seed = 12345u64;
    let duration = 2;

    // Single hash at a time
    let rate_single = measure_hashrate("Single", duration, |nonce| {
        let header = make_header(seed, nonce);
        sha256d_simple(&header)
    });

    // Batch of 4
    let rate_batch4 = measure_hashrate("Batch 4", duration, |nonce| {
        let mut result = [0u8; 32];
        for i in 0..4 {
            let header = make_header(seed, nonce.wrapping_add(i));
            result = sha256d_simple(&header);
        }
        result
    });

    // Batch of 8
    let rate_batch8 = measure_hashrate("Batch 8", duration, |nonce| {
        let mut result = [0u8; 32];
        for i in 0..8 {
            let header = make_header(seed, nonce.wrapping_add(i));
            result = sha256d_simple(&header);
        }
        result
    });

    println!("\n| Batch Size | Rate (MH/s) | Relative |");
    println!("|------------|-------------|----------|");
    println!("| 1          | {:>11.2} | {:>7.2}x |", rate_single, 1.0);
    println!("| 4          | {:>11.2} | {:>7.2}x |", rate_batch4 * 4.0, rate_batch4 * 4.0 / rate_single);
    println!("| 8          | {:>11.2} | {:>7.2}x |", rate_batch8 * 8.0, rate_batch8 * 8.0 / rate_single);
}

/// Test 2: Memory Access Pattern Analysis
/// Does data layout affect performance?
#[test]
fn test_research_v6_memory_patterns() {
    println!("\n=== Research v6: Memory Access Pattern Analysis ===\n");
    println!("Question: Does memory layout affect hashing throughput?\n");

    let iterations = 1_000_000u64;
    let seed = 12345u64;

    // Pattern 1: Generate header each time (cold)
    let start = Instant::now();
    for i in 0..iterations {
        let header = make_header(seed, i as u32);
        let _ = sha256d_simple(&header);
    }
    let cold_time = start.elapsed();

    // Pattern 2: Pre-generate headers (warm cache)
    let mut headers: Vec<[u8; 80]> = Vec::with_capacity(1000);
    for i in 0..1000 {
        headers.push(make_header(seed, i as u32));
    }

    let start = Instant::now();
    for i in 0..iterations {
        let header = &headers[(i % 1000) as usize];
        let _ = sha256d_simple(header);
    }
    let warm_time = start.elapsed();

    // Pattern 3: Same header repeatedly (hot cache)
    let header = make_header(seed, 0);
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());
        let _ = sha256d_simple(&h);
    }
    let hot_time = start.elapsed();

    let cold_rate = iterations as f64 / cold_time.as_secs_f64() / 1_000_000.0;
    let warm_rate = iterations as f64 / warm_time.as_secs_f64() / 1_000_000.0;
    let hot_rate = iterations as f64 / hot_time.as_secs_f64() / 1_000_000.0;

    println!("| Pattern | Time (s) | Rate (MH/s) | Relative |");
    println!("|---------|----------|-------------|----------|");
    println!("| Cold (generate each time) | {:>8.3} | {:>11.2} | {:>7.2}x |",
        cold_time.as_secs_f64(), cold_rate, 1.0);
    println!("| Warm (cached headers)     | {:>8.3} | {:>11.2} | {:>7.2}x |",
        warm_time.as_secs_f64(), warm_rate, warm_rate / cold_rate);
    println!("| Hot (modify in place)     | {:>8.3} | {:>11.2} | {:>7.2}x |",
        hot_time.as_secs_f64(), hot_rate, hot_rate / cold_rate);
}

/// Test 3: Nonce Increment Strategy
/// Does the nonce increment pattern affect performance?
#[test]
fn test_research_v6_nonce_strategy() {
    println!("\n=== Research v6: Nonce Increment Strategy ===\n");
    println!("Question: Does nonce increment pattern affect throughput?\n");

    let iterations = 1_000_000u64;
    let seed = 12345u64;
    let header = make_header(seed, 0);

    // Strategy 1: Sequential increment
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());
        let _ = sha256d_simple(&h);
    }
    let seq_time = start.elapsed();

    // Strategy 2: Large jumps (simulating multi-threaded ranges)
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        let nonce = (i as u32).wrapping_mul(0x01000000); // Large jumps
        h[76..80].copy_from_slice(&nonce.to_le_bytes());
        let _ = sha256d_simple(&h);
    }
    let jump_time = start.elapsed();

    // Strategy 3: Random nonces
    let start = Instant::now();
    let mut rng_state = 0x12345678u64;
    for _ in 0..iterations {
        let mut h = header;
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let nonce = (rng_state >> 32) as u32;
        h[76..80].copy_from_slice(&nonce.to_le_bytes());
        let _ = sha256d_simple(&h);
    }
    let rand_time = start.elapsed();

    let seq_rate = iterations as f64 / seq_time.as_secs_f64() / 1_000_000.0;
    let jump_rate = iterations as f64 / jump_time.as_secs_f64() / 1_000_000.0;
    let rand_rate = iterations as f64 / rand_time.as_secs_f64() / 1_000_000.0;

    println!("| Strategy | Time (s) | Rate (MH/s) | Relative |");
    println!("|----------|----------|-------------|----------|");
    println!("| Sequential    | {:>8.3} | {:>11.2} | {:>7.2}x |", seq_time.as_secs_f64(), seq_rate, 1.0);
    println!("| Large jumps   | {:>8.3} | {:>11.2} | {:>7.2}x |", jump_time.as_secs_f64(), jump_rate, jump_rate / seq_rate);
    println!("| Random        | {:>8.3} | {:>11.2} | {:>7.2}x |", rand_time.as_secs_f64(), rand_rate, rand_rate / seq_rate);
}

/// Test 4: Compare sha2 crate implementations
#[test]
fn test_research_v6_sha2_variants() {
    println!("\n=== Research v6: SHA2 Crate Implementation Analysis ===\n");
    println!("Comparing different usage patterns of the sha2 crate.\n");

    let iterations = 1_000_000u64;
    let seed = 12345u64;
    let header = make_header(seed, 0);

    // Method 1: Create new hasher each time
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());

        let first = Sha256::digest(&h);
        let second = Sha256::digest(&first);
        let _ = second;
    }
    let new_time = start.elapsed();

    // Method 2: Reuse hasher with reset
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());

        let mut hasher = Sha256::new();
        hasher.update(&h);
        let first = hasher.finalize_reset();

        hasher.update(&first);
        let second = hasher.finalize();
        let _ = second;
    }
    let reuse_time = start.elapsed();

    // Method 3: Chain style
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());

        let result = Sha256::digest(&Sha256::digest(&h));
        let _ = result;
    }
    let chain_time = start.elapsed();

    let new_rate = iterations as f64 / new_time.as_secs_f64() / 1_000_000.0;
    let reuse_rate = iterations as f64 / reuse_time.as_secs_f64() / 1_000_000.0;
    let chain_rate = iterations as f64 / chain_time.as_secs_f64() / 1_000_000.0;

    println!("| Method | Time (s) | Rate (MH/s) | Relative |");
    println!("|--------|----------|-------------|----------|");
    println!("| New hasher each time | {:>8.3} | {:>11.2} | {:>7.2}x |", new_time.as_secs_f64(), new_rate, 1.0);
    println!("| Reuse with reset     | {:>8.3} | {:>11.2} | {:>7.2}x |", reuse_time.as_secs_f64(), reuse_rate, reuse_rate / new_rate);
    println!("| Chained digest()     | {:>8.3} | {:>11.2} | {:>7.2}x |", chain_time.as_secs_f64(), chain_rate, chain_rate / new_rate);
}

/// Test 5: Instruction Mix Analysis
/// Measure impact of additional operations in the hot loop
#[test]
fn test_research_v6_instruction_overhead() {
    println!("\n=== Research v6: Instruction Overhead Analysis ===\n");
    println!("Question: How much do extra operations in hot loop cost?\n");

    let iterations = 1_000_000u64;
    let seed = 12345u64;
    let header = make_header(seed, 0);

    // Baseline: Just hash
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());
        let _ = sha256d_simple(&h);
    }
    let baseline_time = start.elapsed();

    // With target check (simulating mining)
    let target = [0u8; 32]; // Impossibly hard target
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());
        let hash = sha256d_simple(&h);
        if hash < target {
            // Would submit block
        }
    }
    let check_time = start.elapsed();

    // With leading zero count
    let start = Instant::now();
    for i in 0..iterations {
        let mut h = header;
        h[76..80].copy_from_slice(&(i as u32).to_le_bytes());
        let hash = sha256d_simple(&h);
        let mut lz = 0u32;
        for byte in hash.iter() {
            if *byte == 0 { lz += 8; } else { lz += byte.leading_zeros(); break; }
        }
        if lz > 100 { /* impossible */ }
    }
    let lz_time = start.elapsed();

    let baseline_rate = iterations as f64 / baseline_time.as_secs_f64() / 1_000_000.0;
    let check_rate = iterations as f64 / check_time.as_secs_f64() / 1_000_000.0;
    let lz_rate = iterations as f64 / lz_time.as_secs_f64() / 1_000_000.0;

    println!("| Operation | Time (s) | Rate (MH/s) | Overhead |");
    println!("|-----------|----------|-------------|----------|");
    println!("| Hash only           | {:>8.3} | {:>11.2} | {:>7.2}% |", baseline_time.as_secs_f64(), baseline_rate, 0.0);
    println!("| + Target comparison | {:>8.3} | {:>11.2} | {:>7.2}% |", check_time.as_secs_f64(), check_rate, (1.0 - check_rate/baseline_rate) * 100.0);
    println!("| + Leading zero count| {:>8.3} | {:>11.2} | {:>7.2}% |", lz_time.as_secs_f64(), lz_rate, (1.0 - lz_rate/baseline_rate) * 100.0);
}

/// Test 6: Parallel Stream Simulation
/// How much speedup from parallel independent hashes?
#[test]
fn test_research_v6_parallel_streams() {
    println!("\n=== Research v6: Parallel Stream Simulation ===\n");
    println!("Question: What's the speedup from parallel hash streams?\n");

    let iterations = 500_000u64;
    let seed = 12345u64;

    // Single stream
    let start = Instant::now();
    for i in 0..iterations {
        let header = make_header(seed, i as u32);
        let _ = sha256d_simple(&header);
    }
    let single_time = start.elapsed();

    // 2 interleaved streams (simulating 2-way SIMD)
    let start = Instant::now();
    for i in 0..(iterations/2) {
        let h1 = make_header(seed, (i * 2) as u32);
        let h2 = make_header(seed, (i * 2 + 1) as u32);
        let _ = sha256d_simple(&h1);
        let _ = sha256d_simple(&h2);
    }
    let dual_time = start.elapsed();

    // 4 interleaved streams
    let start = Instant::now();
    for i in 0..(iterations/4) {
        let h1 = make_header(seed, (i * 4) as u32);
        let h2 = make_header(seed, (i * 4 + 1) as u32);
        let h3 = make_header(seed, (i * 4 + 2) as u32);
        let h4 = make_header(seed, (i * 4 + 3) as u32);
        let _ = sha256d_simple(&h1);
        let _ = sha256d_simple(&h2);
        let _ = sha256d_simple(&h3);
        let _ = sha256d_simple(&h4);
    }
    let quad_time = start.elapsed();

    let single_rate = iterations as f64 / single_time.as_secs_f64() / 1_000_000.0;
    let dual_rate = iterations as f64 / dual_time.as_secs_f64() / 1_000_000.0;
    let quad_rate = iterations as f64 / quad_time.as_secs_f64() / 1_000_000.0;

    println!("| Streams | Time (s) | Rate (MH/s) | Relative |");
    println!("|---------|----------|-------------|----------|");
    println!("| 1       | {:>8.3} | {:>11.2} | {:>7.2}x |", single_time.as_secs_f64(), single_rate, 1.0);
    println!("| 2       | {:>8.3} | {:>11.2} | {:>7.2}x |", dual_time.as_secs_f64(), dual_rate, dual_rate / single_rate);
    println!("| 4       | {:>8.3} | {:>11.2} | {:>7.2}x |", quad_time.as_secs_f64(), quad_rate, quad_rate / single_rate);

    println!("\nNote: True SIMD would process multiple hashes in parallel, not sequentially.");
    println!("This test shows overhead of managing multiple streams.");
}
