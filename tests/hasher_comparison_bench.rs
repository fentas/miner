//! Hasher Implementation Comparison Benchmark
//!
//! Compares all three hasher implementations:
//! - SHA2 Crate: RustCrypto's sha2 crate with hardware acceleration
//! - SIMD: Custom SIMD implementation using #[target_feature]
//! - Assembly: Inline assembly avoiding #[target_feature] boundary
//!
//! Tests both:
//! - Direct calls (from test binary - affected by cross-unit inlining)
//! - Coordinator calls (threads inside library - full optimization)
//!
//! Run with:
//!   cargo test --release --test hasher_comparison_bench -- --nocapture --ignored

use std::time::{Duration, Instant};
use btcminer::mining::{Coordinator, Job, Hasher, HasherType};
use btcminer::mining::sha256_simd::{Midstate, sha256d_midstate_generic, sha256d_midstate_simd, sha256d_midstate_asm, sha256d_midstate_v2, sha256d_midstate_v3, sha256d_midstate_ultimate};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 { format!("{:.2} TH/s", hashrate / 1e12) }
    else if hashrate >= 1e9 { format!("{:.2} GH/s", hashrate / 1e9) }
    else if hashrate >= 1e6 { format!("{:.2} MH/s", hashrate / 1e6) }
    else if hashrate >= 1e3 { format!("{:.2} KH/s", hashrate / 1e3) }
    else { format!("{:.2} H/s", hashrate) }
}

/// Benchmark direct function calls (single-threaded, from test binary)
#[test]
#[ignore]
fn bench_direct_calls() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Direct Function Call Benchmark (Single Thread)              ║");
    println!("║   Note: Affected by cross-compilation-unit inlining limits    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0xABu8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0x12u8; 16];

    // Test 1: Generic
    println!("Testing Generic...");
    let mut count = 0u64;
    let mut nonce = 0u32;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_generic(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_generic = count as f64 / start.elapsed().as_secs_f64();

    // Test 2: SIMD
    println!("Testing SIMD...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_simd(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_simd = count as f64 / start.elapsed().as_secs_f64();

    // Test 3: Assembly
    println!("Testing Assembly...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_asm(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_asm = count as f64 / start.elapsed().as_secs_f64();

    // Test 4: AsmV2 (sha2 crate style)
    println!("Testing AsmV2...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_v2(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_v2 = count as f64 / start.elapsed().as_secs_f64();

    // Test 5: AsmV3 (no target_feature at all - requires global compilation flags)
    println!("Testing AsmV3 (no target_feature)...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_v3(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_v3 = count as f64 / start.elapsed().as_secs_f64();

    // Test 6: Ultimate (combined research optimizations)
    println!("Testing Ultimate...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = sha256d_midstate_ultimate(&midstate, &tail, nonce);
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_ultimate = count as f64 / start.elapsed().as_secs_f64();

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Direct Calls - Single Thread)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  {:25} {:>15} {:>10}", "Implementation", "Hashrate", "Ratio");
    println!("  {:25} {:>15} {:>10}", "─────────────", "────────", "─────");
    println!("  {:25} {:>15} {:>10}", "Generic", format_hashrate(rate_generic), "1.0x");
    println!("  {:25} {:>15} {:>10}", "SIMD", format_hashrate(rate_simd), format!("{:.1}x", rate_simd / rate_generic));
    println!("  {:25} {:>15} {:>10}", "Assembly", format_hashrate(rate_asm), format!("{:.1}x", rate_asm / rate_generic));
    println!("  {:25} {:>15} {:>10}", "AsmV2", format_hashrate(rate_v2), format!("{:.1}x", rate_v2 / rate_generic));
    println!("  {:25} {:>15} {:>10}", "AsmV3 (no target_feature)", format_hashrate(rate_v3), format!("{:.1}x", rate_v3 / rate_generic));
    println!("  {:25} {:>15} {:>10}", "Ultimate", format_hashrate(rate_ultimate), format!("{:.1}x", rate_ultimate / rate_generic));
    println!();
}

/// Benchmark Hasher class with different implementations
#[test]
#[ignore]
fn bench_hasher_class() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Hasher Class Benchmark (Single Thread)                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0xABu8; 76];
    let target = [0x00u8; 32]; // Hard target - no early exit
    let batch_size = 262144u32;

    let hasher_types = [
        (HasherType::Sha2Crate, "SHA2 Crate"),
        (HasherType::Simd, "SIMD"),
        (HasherType::Assembly, "Assembly"),
        (HasherType::AsmV2, "AsmV2"),
        (HasherType::AsmV3, "AsmV3 (no target_feature)"),
        (HasherType::Ultimate, "Ultimate"),
    ];

    let mut results = Vec::new();

    for (hasher_type, name) in hasher_types {
        println!("Testing {}...", name);
        let mut hasher = Hasher::with_type(hasher_type);
        let mut count = 0u64;
        let mut nonce = 0u32;

        let start = Instant::now();
        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &target);
            nonce = nonce.wrapping_add(batch_size);
            count += batch_size as u64;
        }

        let rate = count as f64 / start.elapsed().as_secs_f64();
        results.push((name, rate));
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Hasher Class - Single Thread)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  {:25} {:>15} {:>10}", "Implementation", "Hashrate", "Ratio");
    println!("  {:25} {:>15} {:>10}", "─────────────", "────────", "─────");

    let base_rate = results[0].1;
    for (name, rate) in &results {
        println!("  {:25} {:>15} {:>10}", name, format_hashrate(*rate), format!("{:.1}x", rate / base_rate));
    }
    println!();
}

/// Benchmark through Coordinator (threads inside library - full optimization)
#[test]
#[ignore]
fn bench_coordinator() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Coordinator Benchmark (Multi-threaded, Library Threads)     ║");
    println!("║   This is the REAL performance - threads inside library       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(10);
    let thread_count = num_cpus::get();

    let hasher_types = [
        (HasherType::Sha2Crate, "SHA2 Crate"),
        (HasherType::Simd, "SIMD"),
        (HasherType::Assembly, "Assembly"),
        (HasherType::AsmV2, "AsmV2"),
        (HasherType::AsmV3, "AsmV3 (no target_feature)"),
        (HasherType::Ultimate, "Ultimate"),
    ];

    let mut results = Vec::new();

    for (hasher_type, name) in hasher_types {
        println!("Testing {} with {} threads...", name, thread_count);

        let coordinator = Coordinator::with_options(thread_count, 1 << 18, hasher_type);

        // Create a job with hard target (no early exit)
        let mut job = Job::new("bench-job".to_string());
        job.version = 0x20000000;
        job.prev_hash = [0xABu8; 32];
        job.merkle_root_override = Some([0xCDu8; 32]);
        job.ntime = 0x12345678;
        job.nbits = 0x1d00ffff;
        job.target = [0x00; 32]; // Impossible target
        job.clean_jobs = true;

        coordinator.update_job(job);
        coordinator.start();

        let start = Instant::now();
        std::thread::sleep(duration);

        let hashes = coordinator.stats().get_hashes();
        let elapsed = start.elapsed().as_secs_f64();
        let rate = hashes as f64 / elapsed;
        let per_thread = rate / thread_count as f64;

        coordinator.stop();

        results.push((name, rate, per_thread));
        println!("  {} hashes in {:.1}s = {}", hashes, elapsed, format_hashrate(rate));
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Coordinator - {} Threads)", thread_count);
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  {:15} {:>15} {:>15} {:>10}", "Implementation", "Total", "Per Thread", "Ratio");
    println!("  {:15} {:>15} {:>15} {:>10}", "─────────────", "─────", "──────────", "─────");

    let base_rate = results[0].1;
    for (name, rate, per_thread) in &results {
        println!("  {:15} {:>15} {:>15} {:>10}",
            name,
            format_hashrate(*rate),
            format_hashrate(*per_thread),
            format!("{:.2}x", rate / base_rate)
        );
    }
    println!();

    // Performance analysis
    println!("═══════════════════════════════════════════════════════════════");
    println!("  ANALYSIS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let sha2_rate = results[0].1;
    let simd_rate = results[1].1;
    let asm_rate = results[2].1;
    let v2_rate = results[3].1;
    let v3_rate = results[4].1;
    let ultimate_rate = results[5].1;

    // Find fastest implementation
    let rates = [
        ("SHA2 Crate", sha2_rate),
        ("SIMD", simd_rate),
        ("Assembly", asm_rate),
        ("AsmV2", v2_rate),
        ("AsmV3", v3_rate),
        ("Ultimate", ultimate_rate),
    ];
    let (fastest_name, fastest_rate) = rates.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();

    println!("  ✓ {} is fastest!", fastest_name);
    println!();
    for (name, rate) in &rates {
        if name != fastest_name {
            println!("    {} is {:.1}x vs {}", fastest_name, fastest_rate / rate, name);
        }
    }
    println!();

    if v3_rate > sha2_rate * 0.9 {
        println!("  SUCCESS: AsmV3 matches SHA2 Crate performance!");
        println!("  The global target-feature compilation allows full LLVM inlining.");
    } else {
        println!("  AsmV3 is {:.1}x slower than SHA2 Crate", sha2_rate / v3_rate);
    }
    println!();

    if ultimate_rate > sha2_rate * 0.9 {
        println!("  SUCCESS: Ultimate matches or beats SHA2 Crate performance!");
        println!("  Combined optimizations from all research are effective.");
    } else if ultimate_rate > sha2_rate {
        println!("  EXCELLENT: Ultimate is {:.1}% faster than SHA2 Crate!", (ultimate_rate / sha2_rate - 1.0) * 100.0);
    } else {
        println!("  Ultimate is {:.1}x slower than SHA2 Crate", sha2_rate / ultimate_rate);
    }
    println!();
}

/// Full comparison benchmark
#[test]
#[ignore]
fn bench_full_comparison() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║          FULL HASHER COMPARISON BENCHMARK                     ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    bench_direct_calls();
    bench_hasher_class();
    bench_coordinator();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    BENCHMARK COMPLETE                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
}
