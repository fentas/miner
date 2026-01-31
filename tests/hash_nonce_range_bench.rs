//! Test 4: Direct hash_nonce_range usage
//!
//! Tests if calling hash_nonce_range directly achieves fast speed.
//!
//! Run with:
//!   cargo test --release --test hash_nonce_range_bench -- --nocapture --ignored

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use btcminer::mining::{Hasher, HasherType};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 { format!("{:.2} TH/s", hashrate / 1e12) }
    else if hashrate >= 1e9 { format!("{:.2} GH/s", hashrate / 1e9) }
    else if hashrate >= 1e6 { format!("{:.2} MH/s", hashrate / 1e6) }
    else if hashrate >= 1e3 { format!("{:.2} KH/s", hashrate / 1e3) }
    else { format!("{:.2} H/s", hashrate) }
}

/// Test using hash_nonce_range directly with SHA2 hasher
#[test]
#[ignore]
fn hash_nonce_range_sha2_direct() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 4: hash_nonce_range(SHA2) Direct                       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    let duration = Duration::from_secs(10);
    let batch_size = 262144u32;

    println!("Threads:    {}", thread_count);
    println!("Duration:   {:?}", duration);
    println!("Batch size: {}", batch_size);
    println!("Hasher:     SHA2Crate");
    println!();

    let total_hashes = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let mut handles = Vec::new();

    for thread_id in 0..thread_count {
        let hashes = Arc::clone(&total_hashes);
        let run = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            // Use SHA2Crate hasher type
            let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
            let header_prefix = [0u8; 76];
            let target = [0x00u8; 32]; // Hard target
            let mut nonce = (thread_id as u32) * (u32::MAX / thread_count as u32);

            while run.load(Ordering::Relaxed) {
                // Call hash_nonce_range directly
                let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &target);
                hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                nonce = nonce.wrapping_add(batch_size);
            }
        });
        handles.push(handle);
    }

    let start = Instant::now();
    let report_hashes = Arc::clone(&total_hashes);

    println!("Running...");
    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(2));
        let elapsed = start.elapsed().as_secs_f64();
        let h = report_hashes.load(Ordering::Relaxed);
        let rate = h as f64 / elapsed;
        println!("  {:>5.1}s: {} ({} hashes)", elapsed, format_hashrate(rate), h);
    }

    running.store(false, Ordering::SeqCst);
    for h in handles { let _ = h.join(); }

    let final_hashes = total_hashes.load(Ordering::Relaxed);
    let final_elapsed = start.elapsed().as_secs_f64();
    let final_rate = final_hashes as f64 / final_elapsed;
    let per_thread = final_rate / thread_count as f64;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total hashes:    {}", final_hashes);
    println!("  Total hashrate:  {}", format_hashrate(final_rate));
    println!("  Per-thread rate: {}", format_hashrate(per_thread));
    println!();

    if per_thread > 1e9 {
        println!("  ✓ GH/s - hash_nonce_range is fast!");
    } else {
        println!("  ✗ <GH/s - hash_nonce_range is slow");
    }
}
