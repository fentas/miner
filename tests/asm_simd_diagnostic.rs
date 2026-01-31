//! Diagnostic: Why is Assembly slower than SIMD?
//!
//! Isolates the performance difference between:
//! - SIMD implementation (shani.rs with #[target_feature])
//! - Assembly implementation (asm.rs without #[target_feature])
//!
//! Run with:
//!   cargo test --release --test asm_simd_diagnostic -- --nocapture --ignored

use std::time::{Duration, Instant};
use btcminer::mining::sha256_simd::{Midstate, sha256d_midstate_generic, sha256d_midstate_simd, sha256d_midstate_asm};

fn format_rate(rate: f64) -> String {
    if rate >= 1e9 { format!("{:.2} GH/s", rate / 1e9) }
    else if rate >= 1e6 { format!("{:.2} MH/s", rate / 1e6) }
    else if rate >= 1e3 { format!("{:.2} KH/s", rate / 1e3) }
    else { format!("{:.2} H/s", rate) }
}

/// Test 1: Raw function call overhead
/// Measures how many times we can call each function per second
/// (with minimal work, to isolate call overhead)
#[test]
#[ignore]
fn test_function_call_overhead() {
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Test 1: Function Call Overhead (same inputs)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0xABu8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0x12u8; 16];
    let nonce = 0u32; // Same nonce every time - no real work

    // Test Generic
    println!("Testing Generic...");
    let mut count = 0u64;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_generic(&midstate, &tail, nonce));
            count += 1;
        }
    }
    let rate_generic = count as f64 / start.elapsed().as_secs_f64();

    // Test SIMD
    println!("Testing SIMD...");
    count = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_simd(&midstate, &tail, nonce));
            count += 1;
        }
    }
    let rate_simd = count as f64 / start.elapsed().as_secs_f64();

    // Test ASM
    println!("Testing ASM...");
    count = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_asm(&midstate, &tail, nonce));
            count += 1;
        }
    }
    let rate_asm = count as f64 / start.elapsed().as_secs_f64();

    println!();
    println!("  {:15} {:>15} {:>10}", "Implementation", "Rate", "vs Generic");
    println!("  {:15} {:>15} {:>10}", "─────────────", "────", "──────────");
    println!("  {:15} {:>15} {:>10}", "Generic", format_rate(rate_generic), "1.0x");
    println!("  {:15} {:>15} {:>10}", "SIMD", format_rate(rate_simd), format!("{:.1}x", rate_simd / rate_generic));
    println!("  {:15} {:>15} {:>10}", "ASM", format_rate(rate_asm), format!("{:.1}x", rate_asm / rate_generic));
    println!();

    if rate_simd > rate_asm {
        println!("  SIMD is {:.2}x faster than ASM", rate_simd / rate_asm);
        println!();
        println!("  Possible reasons ASM is slower:");
        println!("  1. ASM checks 3 CPU features vs SIMD's 2 features per call");
        println!("  2. Inline assembly might have worse register allocation");
        println!("  3. LLVM intrinsics are better optimized than hand-written asm");
    }
    println!();
}

/// Test 2: Tight loop performance
/// Each function in a tight loop - measures sustained throughput
#[test]
#[ignore]
fn test_tight_loop() {
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Test 2: Tight Loop Performance (varying nonce)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0xABu8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0x12u8; 16];

    // Test Generic
    println!("Testing Generic...");
    let mut count = 0u64;
    let mut nonce = 0u32;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_generic(&midstate, &tail, nonce));
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_generic = count as f64 / start.elapsed().as_secs_f64();

    // Test SIMD
    println!("Testing SIMD...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_simd(&midstate, &tail, nonce));
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_simd = count as f64 / start.elapsed().as_secs_f64();

    // Test ASM
    println!("Testing ASM...");
    count = 0;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            let _ = std::hint::black_box(sha256d_midstate_asm(&midstate, &tail, nonce));
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }
    let rate_asm = count as f64 / start.elapsed().as_secs_f64();

    println!();
    println!("  {:15} {:>15} {:>10}", "Implementation", "Rate", "vs Generic");
    println!("  {:15} {:>15} {:>10}", "─────────────", "────", "──────────");
    println!("  {:15} {:>15} {:>10}", "Generic", format_rate(rate_generic), "1.0x");
    println!("  {:15} {:>15} {:>10}", "SIMD", format_rate(rate_simd), format!("{:.1}x", rate_simd / rate_generic));
    println!("  {:15} {:>15} {:>10}", "ASM", format_rate(rate_asm), format!("{:.1}x", rate_asm / rate_generic));
    println!();
}

/// Test 3: CPU feature detection overhead
/// How much time is spent in is_x86_feature_detected!() ?
#[test]
#[ignore]
fn test_cpu_detection_overhead() {
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Test 3: CPU Feature Detection Overhead");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let iterations = 100_000_000u64;

    // Test checking 2 features (like SIMD does)
    println!("Testing 2 feature checks (sha + sse4.1)...");
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..iterations {
        if is_x86_feature_detected!("sha") && is_x86_feature_detected!("sse4.1") {
            sum += 1;
        }
    }
    let time_2 = start.elapsed();
    std::hint::black_box(sum);

    // Test checking 3 features (like ASM does)
    println!("Testing 3 feature checks (sha + sse4.1 + ssse3)...");
    let start = Instant::now();
    let mut sum = 0u64;
    for _ in 0..iterations {
        if is_x86_feature_detected!("sha") && is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("ssse3") {
            sum += 1;
        }
    }
    let time_3 = start.elapsed();
    std::hint::black_box(sum);

    println!();
    println!("  {} iterations:", iterations);
    println!("  2 features: {:?} ({:.2} ns/check)", time_2, time_2.as_nanos() as f64 / iterations as f64);
    println!("  3 features: {:?} ({:.2} ns/check)", time_3, time_3.as_nanos() as f64 / iterations as f64);
    println!();

    let overhead_per_hash = (time_3.as_nanos() - time_2.as_nanos()) as f64 / iterations as f64;
    println!("  Extra overhead for 3rd check: {:.2} ns/hash", overhead_per_hash);
    println!();
}

/// Run all diagnostics
#[test]
#[ignore]
fn run_all_diagnostics() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   ASM vs SIMD Performance Diagnostic                          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    test_cpu_detection_overhead();
    test_function_call_overhead();
    test_tight_loop();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    Diagnostic Complete                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
}
