//! Research Experiment v9: CPU Feature Detection and Optimization
//!
//! Detects available CPU features and measures their impact on hashing.
//! Tests SHA-NI, AVX2, AVX-512, and fallback paths.
//!
//! Correlates to: docs/RESEARCH-test-log-v9.md
//! Run with: cargo test --release --test research_v9_test -- --nocapture

use std::time::{Instant, Duration};
use btcminer::mining::{Hasher, HasherType, SimdLevel};

/// Create a test header prefix
fn make_header_prefix(seed: u64) -> [u8; 76] {
    let mut prefix = [0u8; 76];
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        prefix[i] = (state >> 56) as u8;
    }
    prefix
}

/// Benchmark a hasher for a given duration
fn benchmark_duration(hasher: &mut Hasher, duration_secs: u64) -> f64 {
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];
    let batch_size = 10_000u32;

    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut total_hashes = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
        total_hashes += batch_size as u64;
        nonce = nonce.wrapping_add(batch_size);
    }

    total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0
}

/// Test 1: Detect and display CPU features
#[test]
fn test_research_v9_cpu_features() {
    println!("\n=== Research v9: CPU Feature Detection ===\n");

    #[cfg(target_arch = "x86_64")]
    {
        println!("CPU Feature Support:\n");

        let features = [
            ("SHA-NI (sha)", std::is_x86_feature_detected!("sha")),
            ("AVX", std::is_x86_feature_detected!("avx")),
            ("AVX2", std::is_x86_feature_detected!("avx2")),
            ("AVX-512F", std::is_x86_feature_detected!("avx512f")),
            ("AVX-512VL", std::is_x86_feature_detected!("avx512vl")),
            ("SSE4.1", std::is_x86_feature_detected!("sse4.1")),
            ("SSE4.2", std::is_x86_feature_detected!("sse4.2")),
            ("AES-NI", std::is_x86_feature_detected!("aes")),
            ("POPCNT", std::is_x86_feature_detected!("popcnt")),
            ("BMI1", std::is_x86_feature_detected!("bmi1")),
            ("BMI2", std::is_x86_feature_detected!("bmi2")),
        ];

        println!("| Feature | Available |");
        println!("|---------|-----------|");
        for (name, available) in &features {
            let status = if *available { "✓ Yes" } else { "✗ No" };
            println!("| {:15} | {:9} |", name, status);
        }

        // Determine optimal SIMD level
        let sha_ni = std::is_x86_feature_detected!("sha");
        let avx512 = std::is_x86_feature_detected!("avx512f");
        let avx2 = std::is_x86_feature_detected!("avx2");

        println!("\nOptimal SIMD Strategy:");
        if sha_ni {
            println!("  → SHA-NI available - use hardware SHA instructions");
        }
        if avx512 {
            println!("  → AVX-512 available - can process 16 hashes in parallel");
        } else if avx2 {
            println!("  → AVX2 available - can process 8 hashes in parallel");
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("Non-x86_64 architecture - CPU feature detection limited");
    }
}

/// Test 2: SIMD level comparison
#[test]
fn test_research_v9_simd_levels() {
    println!("\n=== Research v9: SIMD Level Comparison ===\n");

    let detected_level = SimdLevel::detect();
    println!("Detected SIMD Level: {:?}\n", detected_level);

    let levels = [
        SimdLevel::Generic,
        SimdLevel::Avx2,
        SimdLevel::Avx512,
        SimdLevel::ShaNi,
    ];

    println!("| SIMD Level | Description |");
    println!("|------------|-------------|");

    for level in &levels {
        let desc = match level {
            SimdLevel::Generic => "Scalar fallback",
            SimdLevel::Avx2 => "AVX2 - 256-bit SIMD",
            SimdLevel::Avx512 => "AVX-512 - 512-bit SIMD",
            SimdLevel::ShaNi => "SHA-NI - Hardware SHA",
        };
        println!("| {:10} | {} |", format!("{:?}", level), desc);
    }
}

/// Test 3: SHA-NI impact (if available)
#[test]
fn test_research_v9_sha_ni_impact() {
    println!("\n=== Research v9: SHA-NI Hardware Acceleration ===\n");

    #[cfg(target_arch = "x86_64")]
    {
        let sha_ni_available = std::is_x86_feature_detected!("sha");

        if sha_ni_available {
            println!("SHA-NI is available on this CPU.\n");

            // Compare SIMD (which should use SHA-NI) vs Sha2Crate
            let duration = 3;

            let mut hasher_sha2 = Hasher::with_type(HasherType::Sha2Crate);
            let rate_sha2 = benchmark_duration(&mut hasher_sha2, duration);

            let mut hasher_simd = Hasher::with_type(HasherType::Simd);
            let rate_simd = benchmark_duration(&mut hasher_simd, duration);

            println!("| Implementation | Rate (MH/s) | Speedup |");
            println!("|----------------|-------------|---------|");
            println!("| Sha2Crate      | {:>11.2} | {:>6.2}x |", rate_sha2, 1.0);
            println!("| SIMD (SHA-NI)  | {:>11.2} | {:>6.2}x |", rate_simd, rate_simd / rate_sha2);

            if rate_simd > rate_sha2 * 1.1 {
                println!("\nSHA-NI provides significant speedup ✓");
            } else {
                println!("\nSHA-NI provides minimal speedup - sha2 crate may also use it");
            }
        } else {
            println!("SHA-NI is NOT available on this CPU.");
            println!("Consider upgrading to a CPU with SHA extensions for better mining performance.");
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        println!("SHA-NI detection not available on this architecture.");
    }
}

/// Test 4: Memory bandwidth estimation
#[test]
fn test_research_v9_memory_bandwidth() {
    println!("\n=== Research v9: Memory Bandwidth Analysis ===\n");
    println!("Estimating if SHA-256 is compute-bound or memory-bound.\n");

    let mut hasher = Hasher::with_type(HasherType::Simd);
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];

    // Measure with different batch sizes to see memory impact
    let batch_sizes = [1_000u32, 10_000, 100_000, 1_000_000];
    let duration = Duration::from_secs(2);

    println!("| Batch Size | Rate (MH/s) | Bytes/Hash | Est. Bandwidth |");
    println!("|------------|-------------|------------|----------------|");

    for batch_size in batch_sizes {
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let rate = total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;

        // Estimate bandwidth: 80 bytes input + 32 bytes output per hash
        let bytes_per_hash = 112;
        let bandwidth_gbps = rate * bytes_per_hash as f64 / 1000.0;

        println!("| {:>10} | {:>11.2} | {:>10} | {:>11.2} GB/s |",
            batch_size, rate, bytes_per_hash, bandwidth_gbps);
    }

    println!("\nNote: If rate is constant across batch sizes, hashing is compute-bound.");
    println!("If rate increases with batch size, there's memory/cache overhead.");
}

/// Test 5: Instruction throughput estimation
#[test]
fn test_research_v9_instruction_throughput() {
    println!("\n=== Research v9: Instruction Throughput Analysis ===\n");

    let mut hasher = Hasher::with_type(HasherType::Simd);
    let duration = 3;
    let rate = benchmark_duration(&mut hasher, duration);

    // SHA-256 operations per hash:
    // - 64 rounds for first SHA-256
    // - 64 rounds for second SHA-256
    // - Each round: ~20 operations (add, rotate, XOR, AND, etc.)
    let ops_per_hash = 128 * 20; // Rough estimate

    let mops = rate * ops_per_hash as f64; // Million operations per second

    println!("Hashrate: {:.2} MH/s", rate);
    println!("Estimated operations per hash: ~{}", ops_per_hash);
    println!("Estimated throughput: {:.0} million ops/sec ({:.2} GOPS)", mops, mops / 1000.0);

    // Compare to typical CPU specs
    // Modern CPUs: ~4 IPC, ~4 GHz = ~16 GOPS per core
    let typical_cpu_gops = 16.0;
    let utilization = (mops / 1000.0) / typical_cpu_gops * 100.0;

    println!("\nTypical modern CPU: ~{:.0} GOPS/core", typical_cpu_gops);
    println!("Estimated CPU utilization: {:.1}%", utilization);

    if utilization > 80.0 {
        println!("→ Compute-bound: CPU is working near capacity");
    } else if utilization > 40.0 {
        println!("→ Mixed: Some headroom for optimization");
    } else {
        println!("→ Underutilized: Check for bottlenecks (memory, cache, dependencies)");
    }
}

/// Test 6: Cache effect analysis
#[test]
fn test_research_v9_cache_effects() {
    println!("\n=== Research v9: Cache Effect Analysis ===\n");
    println!("Testing with different working set sizes.\n");

    let hard_target = [0u8; 32];
    let duration = Duration::from_secs(2);

    // Test with 1, 10, 100, 1000 different headers
    let header_counts = [1, 10, 100, 1000];

    println!("| Headers | Rate (MH/s) | Relative | Cache Pressure |");
    println!("|---------|-------------|----------|----------------|");

    let mut baseline_rate = 0.0f64;

    for &count in &header_counts {
        // Pre-generate headers
        let headers: Vec<[u8; 76]> = (0..count)
            .map(|i| make_header_prefix(i as u64))
            .collect();

        let mut hasher = Hasher::with_type(HasherType::Simd);
        let batch_size = 1_000u32;
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut idx = 0usize;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let header = &headers[idx % headers.len()];
            let _ = hasher.hash_nonce_range(header, nonce, batch_size, &hard_target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
            idx += 1;
        }

        let rate = total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;

        if baseline_rate == 0.0 {
            baseline_rate = rate;
        }

        let cache_pressure = match count {
            1 => "Minimal (L1)",
            10 => "Low (L2)",
            100 => "Medium (L3)",
            1000 => "High (RAM)",
            _ => "Unknown",
        };

        println!("| {:>7} | {:>11.2} | {:>7.2}x | {:14} |",
            count, rate, rate / baseline_rate, cache_pressure);
    }

    println!("\nNote: Significant rate drop with more headers indicates cache sensitivity.");
}
