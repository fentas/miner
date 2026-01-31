//! Standalone SHA2 benchmark - Test 1 from performance-test-plan.md
//!
//! This test isolates the sha2 crate performance without any btcminer library calls.
//! If this shows ~4 GH/s per thread, cross-crate boundary is NOT the issue.
//! If this shows ~10 MH/s per thread, cross-crate boundary IS the issue.
//!
//! Run with:
//!   cargo test --release --test standalone_sha2_bench -- --nocapture --ignored

use sha2::{Sha256, Digest};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

/// Test 1: Direct sha2 crate usage (no btcminer library)
#[test]
#[ignore]
fn standalone_sha2_single_thread() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 1: Standalone SHA2 Crate (Single Thread)               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(10);
    let start = Instant::now();
    let mut total = 0u64;
    let mut header = [0u8; 80];

    println!("Running for {:?}...", duration);

    while start.elapsed() < duration {
        // Batch of 262144 to match btcminer batch size
        for nonce in 0u32..262144 {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());

            // Double SHA-256 (same as Bitcoin)
            let first = Sha256::digest(&header);
            let _hash = Sha256::digest(&first);

            total += 1;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = total as f64 / elapsed;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Single Thread)");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total hashes: {}", total);
    println!("  Elapsed:      {:.2}s", elapsed);
    println!("  Hashrate:     {}", format_hashrate(rate));
    println!();

    if rate > 1e9 {
        println!("  ✓ GH/s achieved - cross-crate is NOT the issue");
    } else if rate > 100e6 {
        println!("  ~ 100+ MH/s - moderate performance");
    } else {
        println!("  ✗ <100 MH/s - cross-crate boundary confirmed");
    }
    println!();
}

/// Test 1b: Multi-threaded sha2 (matches production thread count)
#[test]
#[ignore]
fn standalone_sha2_multi_thread() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 1: Standalone SHA2 Crate (Multi Thread)                ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    let duration = Duration::from_secs(10);

    println!("Threads:  {}", thread_count);
    println!("Duration: {:?}", duration);
    println!();

    let total_hashes = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let mut handles = Vec::new();

    for thread_id in 0..thread_count {
        let hashes = Arc::clone(&total_hashes);
        let run = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            let mut header = [0u8; 80];
            // Each thread starts at different nonce range
            let mut nonce = (thread_id as u32) * (u32::MAX / thread_count as u32);

            while run.load(Ordering::Relaxed) {
                // Batch of 262144
                // Use black_box to prevent compiler from optimizing away the work
                let mut checksum = 0u8;
                for _ in 0..262144 {
                    header[76..80].copy_from_slice(&nonce.to_le_bytes());

                    // Double SHA-256
                    let first = Sha256::digest(&header);
                    let hash = Sha256::digest(&first);

                    // Prevent optimization - accumulate checksum
                    checksum = checksum.wrapping_add(hash[0]);
                    nonce = nonce.wrapping_add(1);
                }
                // Use checksum to prevent optimization
                std::hint::black_box(checksum);
                hashes.fetch_add(262144, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    // Progress reporting
    let start = Instant::now();
    let report_hashes = Arc::clone(&total_hashes);

    println!("Running...");
    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(2));
        let elapsed = start.elapsed().as_secs_f64();
        let hashes = report_hashes.load(Ordering::Relaxed);
        let rate = hashes as f64 / elapsed;
        println!("  {:>5.1}s: {} ({} hashes)", elapsed, format_hashrate(rate), hashes);
    }

    // Stop workers
    running.store(false, Ordering::SeqCst);
    for h in handles {
        let _ = h.join();
    }

    let final_hashes = total_hashes.load(Ordering::Relaxed);
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_rate = final_hashes as f64 / final_elapsed;
    let per_thread = final_rate / thread_count as f64;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Multi Thread)");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total hashes:    {}", final_hashes);
    println!("  Elapsed:         {:.2}s", final_elapsed);
    println!("  Total hashrate:  {}", format_hashrate(final_rate));
    println!("  Per-thread rate: {}", format_hashrate(per_thread));
    println!();

    if final_rate > 10e9 {
        println!("  ✓ 10+ GH/s achieved - matches production!");
        println!("    Cross-crate boundary is NOT the primary issue.");
    } else if final_rate > 1e9 {
        println!("  ~ 1-10 GH/s - partial cross-crate effect");
    } else {
        println!("  ✗ <1 GH/s - cross-crate boundary confirmed");
    }
    println!();
}
