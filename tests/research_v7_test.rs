//! Research Experiment v7: Hasher Type Comparison
//!
//! Benchmarks all available hasher implementations to find the fastest.
//! Tests Sha2Crate, Simd, Assembly, AsmV2, AsmV3 under identical conditions.
//!
//! Correlates to: docs/RESEARCH-test-log-v7.md
//! Run with: cargo test --release --test research_v7_test -- --nocapture

use std::time::{Instant, Duration};
use btcminer::mining::{Hasher, HasherType};

/// Create a test header prefix (first 76 bytes)
fn make_header_prefix(seed: u64) -> [u8; 76] {
    let mut prefix = [0u8; 76];
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        prefix[i] = (state >> 56) as u8;
    }
    prefix
}

/// Benchmark a hasher type for a given duration
fn benchmark_hasher(hasher_type: HasherType, duration_secs: u64) -> (f64, u64) {
    let mut hasher = Hasher::with_type(hasher_type);
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32]; // Impossible target - we just want to measure speed

    let duration = Duration::from_secs(duration_secs);
    let batch_size = 10_000u32;
    let start = Instant::now();
    let mut total_hashes = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
        total_hashes += batch_size as u64;
        nonce = nonce.wrapping_add(batch_size);
    }

    let elapsed = start.elapsed().as_secs_f64();
    let hashrate = total_hashes as f64 / elapsed / 1_000_000.0;

    (hashrate, total_hashes)
}

/// Test 1: Compare all hasher types
#[test]
fn test_research_v7_hasher_comparison() {
    println!("\n=== Research v7: Hasher Type Comparison ===\n");
    println!("Benchmarking all available hasher implementations.\n");

    let duration = 3; // seconds per hasher

    let hasher_types = [
        (HasherType::Sha2Crate, "Sha2Crate (baseline)"),
        (HasherType::Simd, "SIMD (AVX2/SHA-NI)"),
        (HasherType::Assembly, "Assembly v1"),
        (HasherType::AsmV2, "Assembly v2"),
        (HasherType::AsmV3, "Assembly v3"),
    ];

    println!("| Hasher Type | Rate (MH/s) | Hashes | Relative |");
    println!("|-------------|-------------|--------|----------|");

    let mut baseline_rate = 0.0f64;
    let mut results: Vec<(String, f64)> = Vec::new();

    for (hasher_type, name) in &hasher_types {
        let (rate, hashes) = benchmark_hasher(*hasher_type, duration);

        if baseline_rate == 0.0 {
            baseline_rate = rate;
        }

        let relative = rate / baseline_rate;
        println!("| {:23} | {:>11.2} | {:>6}M | {:>7.2}x |",
            name, rate, hashes / 1_000_000, relative);

        results.push((name.to_string(), rate));
    }

    // Find fastest
    let (fastest_name, fastest_rate) = results.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    println!("\nFastest: {} at {:.2} MH/s", fastest_name, fastest_rate);
    println!("Speedup over baseline: {:.2}x", fastest_rate / baseline_rate);
}

/// Test 2: Consistency check - all hashers produce same result
#[test]
fn test_research_v7_hasher_consistency() {
    println!("\n=== Research v7: Hasher Consistency Check ===\n");
    println!("Verifying all hashers produce identical results.\n");

    let header_prefix = make_header_prefix(12345);
    // Easy target that will find a match quickly
    let mut easy_target = [0xFF; 32];
    easy_target[0] = 0x0F; // First byte < 0x10

    let hasher_types = [
        HasherType::Sha2Crate,
        HasherType::Simd,
        HasherType::Assembly,
        HasherType::AsmV2,
        HasherType::AsmV3,
    ];

    let mut reference_result: Option<(u32, [u8; 32])> = None;

    for hasher_type in &hasher_types {
        let mut hasher = Hasher::with_type(*hasher_type);
        let result = hasher.hash_nonce_range(&header_prefix, 0, 1000, &easy_target);

        match (&reference_result, &result) {
            (None, Some(r)) => {
                reference_result = Some(*r);
                println!("Reference ({:?}): nonce={}, hash={:02x?}",
                    hasher_type, r.0, &r.1[..4]);
            }
            (Some(ref_r), Some(r)) => {
                if ref_r.0 == r.0 && ref_r.1 == r.1 {
                    println!("{:?}: MATCH ✓", hasher_type);
                } else {
                    println!("{:?}: MISMATCH ✗ nonce={}, hash={:02x?}",
                        hasher_type, r.0, &r.1[..4]);
                }
            }
            (Some(_), None) => {
                println!("{:?}: NO RESULT ✗", hasher_type);
            }
            (None, None) => {
                println!("{:?}: No match found (continuing...)", hasher_type);
            }
        }
    }
}

/// Test 3: Throughput stability over time
#[test]
fn test_research_v7_throughput_stability() {
    println!("\n=== Research v7: Throughput Stability ===\n");
    println!("Checking if hashrate is stable over time.\n");

    let mut hasher = Hasher::with_type(HasherType::Simd);
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];

    let batch_size = 100_000u32;
    let intervals = 10;
    let interval_duration = Duration::from_secs(1);

    println!("| Interval | Hashes | Rate (MH/s) | Deviation |");
    println!("|----------|--------|-------------|-----------|");

    let mut rates: Vec<f64> = Vec::new();
    let mut nonce = 0u32;

    for interval in 1..=intervals {
        let start = Instant::now();
        let mut interval_hashes = 0u64;

        while start.elapsed() < interval_duration {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
            interval_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let rate = interval_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;
        rates.push(rate);

        let avg_so_far: f64 = rates.iter().sum::<f64>() / rates.len() as f64;
        let deviation = (rate - avg_so_far) / avg_so_far * 100.0;

        println!("| {:>8} | {:>6}M | {:>11.2} | {:>+8.2}% |",
            interval, interval_hashes / 1_000_000, rate, deviation);
    }

    let avg: f64 = rates.iter().sum::<f64>() / rates.len() as f64;
    let variance: f64 = rates.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / rates.len() as f64;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg * 100.0;

    println!("\nAverage: {:.2} MH/s", avg);
    println!("Std Dev: {:.2} MH/s", std_dev);
    println!("Coefficient of Variation: {:.2}%", cv);

    if cv < 5.0 {
        println!("Result: STABLE ✓ (CV < 5%)");
    } else {
        println!("Result: UNSTABLE (CV >= 5%)");
    }
}

/// Test 4: Batch size impact on throughput
#[test]
fn test_research_v7_batch_size_impact() {
    println!("\n=== Research v7: Batch Size Impact ===\n");
    println!("Testing how batch size affects throughput.\n");

    let mut hasher = Hasher::with_type(HasherType::Simd);
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];

    let batch_sizes = [100, 1_000, 10_000, 50_000, 100_000, 500_000];
    let test_duration = Duration::from_secs(2);

    println!("| Batch Size | Rate (MH/s) | Relative |");
    println!("|------------|-------------|----------|");

    let mut baseline_rate = 0.0f64;

    for batch_size in batch_sizes {
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < test_duration {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let rate = total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;

        if baseline_rate == 0.0 {
            baseline_rate = rate;
        }

        println!("| {:>10} | {:>11.2} | {:>7.2}x |", batch_size, rate, rate / baseline_rate);
    }
}

/// Test 5: Cold vs warm start
#[test]
fn test_research_v7_cold_vs_warm() {
    println!("\n=== Research v7: Cold vs Warm Start ===\n");
    println!("Testing performance difference between cold and warm starts.\n");

    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];
    let batch_size = 100_000u32;
    let iterations = 50;

    // Cold start - new hasher each batch
    let start = Instant::now();
    for i in 0..iterations {
        let mut hasher = Hasher::with_type(HasherType::Simd);
        let _ = hasher.hash_nonce_range(&header_prefix, i * batch_size, batch_size, &hard_target);
    }
    let cold_time = start.elapsed();
    let cold_rate = (iterations as u64 * batch_size as u64) as f64 / cold_time.as_secs_f64() / 1_000_000.0;

    // Warm start - reuse hasher
    let mut hasher = Hasher::with_type(HasherType::Simd);
    let start = Instant::now();
    for i in 0..iterations {
        let _ = hasher.hash_nonce_range(&header_prefix, i * batch_size, batch_size, &hard_target);
    }
    let warm_time = start.elapsed();
    let warm_rate = (iterations as u64 * batch_size as u64) as f64 / warm_time.as_secs_f64() / 1_000_000.0;

    // Hot start - same prefix, only nonce changes (midstate cached)
    let start = Instant::now();
    for i in 0..iterations {
        let _ = hasher.hash_nonce_range(&header_prefix, i * batch_size, batch_size, &hard_target);
    }
    let hot_time = start.elapsed();
    let hot_rate = (iterations as u64 * batch_size as u64) as f64 / hot_time.as_secs_f64() / 1_000_000.0;

    println!("| State | Rate (MH/s) | Time (ms) | Relative |");
    println!("|-------|-------------|-----------|----------|");
    println!("| Cold  | {:>11.2} | {:>9.2} | {:>7.2}x |", cold_rate, cold_time.as_secs_f64() * 1000.0, 1.0);
    println!("| Warm  | {:>11.2} | {:>9.2} | {:>7.2}x |", warm_rate, warm_time.as_secs_f64() * 1000.0, warm_rate / cold_rate);
    println!("| Hot   | {:>11.2} | {:>9.2} | {:>7.2}x |", hot_rate, hot_time.as_secs_f64() * 1000.0, hot_rate / cold_rate);
}

/// Test 6: Nonce wraparound handling
#[test]
fn test_research_v7_nonce_wraparound() {
    println!("\n=== Research v7: Nonce Wraparound Handling ===\n");
    println!("Testing performance near nonce space boundaries.\n");

    let mut hasher = Hasher::with_type(HasherType::Simd);
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];
    let batch_size = 100_000u32;
    let test_duration = Duration::from_millis(500);

    let test_points: [(u32, &str); 4] = [
        (0, "Start (0)"),
        (u32::MAX / 2, "Middle"),
        (u32::MAX - batch_size, "Near end"),
        (u32::MAX - batch_size / 2, "Wraparound"),
    ];

    println!("| Start Point | Rate (MH/s) | Note |");
    println!("|-------------|-------------|------|");

    for (start_nonce, name) in &test_points {
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = *start_nonce;

        while start.elapsed() < test_duration {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let rate = total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;
        println!("| {:>11} | {:>11.2} | {} |", start_nonce, rate, name);
    }
}
