//! TUI Hashrate Test
//!
//! IMPORTANT: These tests use HARD targets [0x00; 32] to measure real hashrate.
//! Easy targets [0xFF; 32] cause early exit after 1 hash, inflating results by 256,000x!
//!
//! Expected performance with SHA-NI: ~10 MH/s per thread (~240 MH/s for 24 threads)
//!
//! Tests the full mining loop with TUI state tracking to verify
//! hashrate calculation accuracy.
//!
//! Run with:
//!   cargo test --release tui_hashrate -- --nocapture --ignored

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use btcminer::mining::{Coordinator, Job};
use btcminer::stats::HashrateCalculator;
use btcminer::tui::TuiState;

/// Format hashrate for display
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

/// Test hashrate calculation exactly as done in main.rs
#[test]
#[ignore]
fn tui_hashrate_with_calculator() {
    println!("\n=== TUI Hashrate with HashrateCalculator ===");

    let num_threads = num_cpus::get();
    println!("Using {} threads", num_threads);

    // Create coordinator (same as main.rs)
    let coordinator = Coordinator::new(num_threads);

    // Create a test job
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    // Hard target for realistic hashrate measurement
    job.target = [0x00; 32];
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    // Setup HashrateCalculator exactly as in main.rs (60 second window)
    let mut hashrate_calc = HashrateCalculator::new(60);

    // Simulate the main loop stats update interval (500ms as in main.rs)
    let stats_interval = Duration::from_millis(500);
    let mut last_stats = Instant::now();

    let test_duration = Duration::from_secs(10);
    let start = Instant::now();

    println!("\nRunning for {:?}...\n", test_duration);
    println!("{:<10} {:>15} {:>15} {:>15}", "Time", "Total Hashes", "Calc Rate", "Direct Rate");
    println!("{}", "-".repeat(60));

    while start.elapsed() < test_duration {
        // Update stats at the same interval as main.rs
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();

            // Add sample to calculator (same as main.rs)
            hashrate_calc.add_sample(stats.hashes);

            // Get calculated hashrate (this is what TUI displays)
            let calc_hashrate = hashrate_calc.hashrate();

            // Also calculate direct rate for comparison
            let elapsed = start.elapsed().as_secs_f64();
            let direct_hashrate = if elapsed > 0.0 {
                stats.hashes as f64 / elapsed
            } else {
                0.0
            };

            println!(
                "{:>8.1}s {:>15} {:>15} {:>15}",
                start.elapsed().as_secs_f64(),
                stats.hashes,
                format_hashrate(calc_hashrate),
                format_hashrate(direct_hashrate)
            );

            last_stats = Instant::now();
        }

        thread::sleep(Duration::from_millis(50));
    }

    coordinator.stop();

    let final_stats = coordinator.stats().snapshot();
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_direct = final_stats.hashes as f64 / final_elapsed;
    let final_calc = hashrate_calc.hashrate();

    println!("{}", "-".repeat(60));
    println!("\nFinal Results:");
    println!("  Total hashes:    {}", final_stats.hashes);
    println!("  Elapsed:         {:.2}s", final_elapsed);
    println!("  Calculator rate: {}", format_hashrate(final_calc));
    println!("  Direct rate:     {}", format_hashrate(final_direct));

    let diff_pct = ((final_calc - final_direct) / final_direct * 100.0).abs();
    println!("  Difference:      {:.1}%", diff_pct);

    if diff_pct > 10.0 {
        println!("\n  WARNING: Large difference between calculator and direct rate!");
    }
}

/// Test with TuiState tracking (full simulation)
#[test]
#[ignore]
fn tui_hashrate_with_tui_state() {
    println!("\n=== TUI Hashrate with TuiState ===");

    let num_threads = num_cpus::get();
    println!("Using {} threads", num_threads);

    // Create TUI state (same as main.rs)
    let tui_state = Arc::new(TuiState::new());
    tui_state.set_worker_count(num_threads);

    // Create coordinator
    let coordinator = Coordinator::new(num_threads);

    // Create a test job
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    // Hard target for realistic hashrate measurement
    job.target = [0x00; 32];
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    // Setup same as main.rs
    let mut hashrate_calc = HashrateCalculator::new(60);
    let stats_interval = Duration::from_millis(500);
    let mut last_stats = Instant::now();

    let test_duration = Duration::from_secs(10);
    let start = Instant::now();

    println!("\nRunning for {:?}...\n", test_duration);
    println!("{:<10} {:>15} {:>15}", "Time", "TUI Hashrate", "Direct Rate");
    println!("{}", "-".repeat(45));

    while start.elapsed() < test_duration {
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();
            hashrate_calc.add_sample(stats.hashes);
            let hashrate = hashrate_calc.hashrate();

            // Update TUI state (same as main.rs)
            tui_state.set_hashrate(hashrate);
            tui_state.set_total_hashes(stats.hashes);
            tui_state.set_shares(stats.shares_accepted, stats.shares_rejected);

            // Get hashrate from TUI state (what the UI reads)
            let tui_hashrate = tui_state.get_hashrate();

            // Direct calculation
            let elapsed = start.elapsed().as_secs_f64();
            let direct_hashrate = stats.hashes as f64 / elapsed;

            println!(
                "{:>8.1}s {:>15} {:>15}",
                elapsed,
                format_hashrate(tui_hashrate),
                format_hashrate(direct_hashrate)
            );

            last_stats = Instant::now();
        }

        thread::sleep(Duration::from_millis(50));
    }

    coordinator.stop();

    let final_tui = tui_state.get_hashrate();
    let final_stats = coordinator.stats().snapshot();
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_direct = final_stats.hashes as f64 / final_elapsed;

    println!("{}", "-".repeat(45));
    println!("\nFinal Results:");
    println!("  TUI hashrate:  {}", format_hashrate(final_tui));
    println!("  Direct rate:   {}", format_hashrate(final_direct));
    println!("  Total hashes:  {}", final_stats.hashes);
}

/// Quick test to verify HashrateCalculator math
#[test]
fn tui_hashrate_calculator_math() {
    println!("\n=== HashrateCalculator Math Test ===");

    let mut calc = HashrateCalculator::new(60);

    // Simulate adding 1 billion hashes per second for 5 seconds
    let hashes_per_sec = 1_000_000_000u64; // 1 GH/s
    let mut cumulative = 0u64;

    for i in 1..=5 {
        cumulative += hashes_per_sec;
        calc.add_sample(cumulative);

        let rate = calc.hashrate();
        println!("  After {}s: {} hashes, calc = {}",
            i, cumulative, format_hashrate(rate));

        thread::sleep(Duration::from_secs(1));
    }

    let final_rate = calc.hashrate();
    println!("\nFinal rate: {}", format_hashrate(final_rate));

    // Should be approximately 1 GH/s
    let expected = 1_000_000_000.0;
    let error = ((final_rate - expected) / expected).abs();

    println!("Expected: {}", format_hashrate(expected));
    println!("Error: {:.1}%", error * 100.0);

    assert!(error < 0.2, "HashrateCalculator error too high: {:.1}%", error * 100.0);
}

/// Test with HARD target (like real Bitcoin mining)
/// This reveals the true hashrate without early-exit inflation
#[test]
#[ignore]
fn tui_hashrate_hard_target() {
    println!("\n=== TUI Hashrate with HARD Target (Real Mining Simulation) ===");

    let num_threads = num_cpus::get();
    println!("Using {} threads", num_threads);

    let coordinator = Coordinator::new(num_threads);

    // Create a test job with HARD target (like real Bitcoin)
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    // HARD target - almost impossible to hit (like real Bitcoin)
    job.target = [0x00; 32];
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    let mut hashrate_calc = HashrateCalculator::new(60);
    let stats_interval = Duration::from_millis(500);
    let mut last_stats = Instant::now();

    let test_duration = Duration::from_secs(10);
    let start = Instant::now();

    println!("\nRunning for {:?} with HARD target (all 256K nonces processed per batch)...\n", test_duration);
    println!("{:<10} {:>15} {:>15} {:>15}", "Time", "Total Hashes", "Calc Rate", "Direct Rate");
    println!("{}", "-".repeat(60));

    while start.elapsed() < test_duration {
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();
            hashrate_calc.add_sample(stats.hashes);
            let calc_hashrate = hashrate_calc.hashrate();

            let elapsed = start.elapsed().as_secs_f64();
            let direct_hashrate = if elapsed > 0.0 {
                stats.hashes as f64 / elapsed
            } else {
                0.0
            };

            println!(
                "{:>8.1}s {:>15} {:>15} {:>15}",
                start.elapsed().as_secs_f64(),
                stats.hashes,
                format_hashrate(calc_hashrate),
                format_hashrate(direct_hashrate)
            );

            last_stats = Instant::now();
        }

        thread::sleep(Duration::from_millis(50));
    }

    coordinator.stop();

    let final_stats = coordinator.stats().snapshot();
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_direct = final_stats.hashes as f64 / final_elapsed;
    let final_calc = hashrate_calc.hashrate();

    println!("{}", "-".repeat(60));
    println!("\nFinal Results (HARD target - no early exit):");
    println!("  Total hashes:    {}", final_stats.hashes);
    println!("  Elapsed:         {:.2}s", final_elapsed);
    println!("  Calculator rate: {}", format_hashrate(final_calc));
    println!("  Direct rate:     {}", format_hashrate(final_direct));

    println!("\n⚠️  If this shows similar hashrate to the TUI (e.g., 200-300 MH/s),");
    println!("    then the previous tests were WRONG due to early-exit inflation!");
    println!("    The real mining hashrate IS around 200-300 MH/s per thread.");
}

/// Full diagnostic combining all tests
#[test]
#[ignore]
fn tui_hashrate_full_diagnostic() {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           TUI Hashrate Diagnostic                             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    tui_hashrate_calculator_math();
    tui_hashrate_with_calculator();
    tui_hashrate_with_tui_state();

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    Diagnostic Complete                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
}
