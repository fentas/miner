//! Test 2: Coordinator with SHA2 hasher
//!
//! Uses btcminer's actual Coordinator/Worker with SHA2 hasher.
//! This tests if the library code path matches production.
//!
//! Run with:
//!   cargo test --release --test coordinator_sha2_bench -- --nocapture --ignored

use std::time::{Duration, Instant};
use btcminer::mining::{Coordinator, Job, HasherType};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 {
        format!("{:.2} TH/s", hashrate / 1e12)
    } else if hashrate >= 1e9 {
        format!("{:.2} GH/s", hashrate / 1e9)
    } else if hashrate >= 1e6 {
        format!("{:.2} MH/s", hashrate / 1e6)
    } else if hashrate >= 1e3 {
        format!("{:.2} KH/s", hashrate / 1e3)
    } else {
        format!("{:.2} H/s", hashrate)
    }
}

/// Test using btcminer's Coordinator with SHA2 hasher
#[test]
#[ignore]
fn coordinator_sha2_hashrate() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 2: Coordinator with SHA2 Hasher                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    println!("Threads: {}", thread_count);
    println!("Hasher:  SHA2 Crate");
    println!();

    // Create coordinator with SHA2 hasher (same as production with hasher = "sha2")
    let coordinator = Coordinator::with_options(thread_count, 1 << 18, HasherType::Sha2Crate);

    // Create a realistic job
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.target = [0x00; 32]; // Hard target - no early exit
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    let duration = Duration::from_secs(10);
    let start = Instant::now();

    println!("Running for {:?}...", duration);
    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(2));
        let stats = coordinator.stats().snapshot();
        let elapsed = start.elapsed().as_secs_f64();
        let rate = stats.hashes as f64 / elapsed;
        println!("  {:>5.1}s: {} ({} hashes)", elapsed, format_hashrate(rate), stats.hashes);
    }

    coordinator.stop();

    let final_stats = coordinator.stats().snapshot();
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_rate = final_stats.hashes as f64 / final_elapsed;
    let per_thread = final_rate / thread_count as f64;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total hashes:    {}", final_stats.hashes);
    println!("  Elapsed:         {:.2}s", final_elapsed);
    println!("  Total hashrate:  {}", format_hashrate(final_rate));
    println!("  Per-thread rate: {}", format_hashrate(per_thread));
    println!();

    // Compare to baselines
    let production_per_thread = 3.58e9; // 3.58 GH/s from baseline
    let standalone_per_thread = 4.84e6; // 4.84 MH/s from Test 1

    let vs_production = per_thread / production_per_thread;
    let vs_standalone = per_thread / standalone_per_thread;

    println!("  Comparison:");
    println!("    vs Production (3.58 GH/s/thread): {:.1}x", vs_production);
    println!("    vs Standalone (4.84 MH/s/thread): {:.1}x", vs_standalone);
    println!();

    if per_thread > 1e9 {
        println!("  ✓ GH/s per thread - matches production!");
        println!("    Cross-crate boundary is NOT the issue for SHA2.");
    } else if per_thread > 100e6 {
        println!("  ~ 100+ MH/s per thread - moderate");
    } else {
        println!("  ✗ <100 MH/s per thread - library boundary issue");
    }
    println!();
}

/// Test using btcminer's Coordinator with SIMD hasher (for comparison)
#[test]
#[ignore]
fn coordinator_simd_hashrate() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 2b: Coordinator with SIMD Hasher                       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    println!("Threads: {}", thread_count);
    println!("Hasher:  SIMD");
    println!();

    // Create coordinator with SIMD hasher (default)
    let coordinator = Coordinator::with_options(thread_count, 1 << 18, HasherType::Simd);

    // Create a realistic job
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.target = [0x00; 32]; // Hard target - no early exit
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    let duration = Duration::from_secs(10);
    let start = Instant::now();

    println!("Running for {:?}...", duration);
    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(2));
        let stats = coordinator.stats().snapshot();
        let elapsed = start.elapsed().as_secs_f64();
        let rate = stats.hashes as f64 / elapsed;
        println!("  {:>5.1}s: {} ({} hashes)", elapsed, format_hashrate(rate), stats.hashes);
    }

    coordinator.stop();

    let final_stats = coordinator.stats().snapshot();
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_rate = final_stats.hashes as f64 / final_elapsed;
    let per_thread = final_rate / thread_count as f64;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total hashes:    {}", final_stats.hashes);
    println!("  Elapsed:         {:.2}s", final_elapsed);
    println!("  Total hashrate:  {}", format_hashrate(final_rate));
    println!("  Per-thread rate: {}", format_hashrate(per_thread));
    println!();
}
