//! Test 5: Worker-style thread spawning
//!
//! Mimics exactly how Worker::spawn creates threads and hashers.
//!
//! Run with:
//!   cargo test --release --test worker_style_bench -- --nocapture --ignored

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;
use btcminer::mining::{Hasher, HasherType};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 { format!("{:.2} TH/s", hashrate / 1e12) }
    else if hashrate >= 1e9 { format!("{:.2} GH/s", hashrate / 1e9) }
    else if hashrate >= 1e6 { format!("{:.2} MH/s", hashrate / 1e6) }
    else if hashrate >= 1e3 { format!("{:.2} KH/s", hashrate / 1e3) }
    else { format!("{:.2} H/s", hashrate) }
}

/// Mimic Worker::spawn exactly
#[test]
#[ignore]
fn worker_style_spawn() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 5: Worker-Style Thread Spawning                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    let duration = Duration::from_secs(10);
    let batch_size = 262144u32;

    println!("Threads:    {}", thread_count);
    println!("Duration:   {:?}", duration);
    println!("Batch size: {}", batch_size);
    println!();

    let total_hashes = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let mut handles = Vec::new();

    for id in 0..thread_count {
        let hashes = Arc::clone(&total_hashes);
        let run = Arc::clone(&running);
        let hasher_type = HasherType::Sha2Crate;

        // Use thread::Builder::new().name() like Worker does
        let handle = thread::Builder::new()
            .name(format!("miner-{}", id))
            .spawn(move || {
                // Create hasher INSIDE thread like Worker does
                let mut hasher = Hasher::with_type(hasher_type);
                let header_prefix = [0u8; 76];
                let target = [0x00u8; 32];
                let mut nonce = (id as u32) * (u32::MAX / thread_count as u32);

                while run.load(Ordering::Acquire) {  // Use Acquire like Worker
                    let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &target);
                    hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                    nonce = nonce.wrapping_add(batch_size);
                }
            })
            .expect("Failed to spawn thread");

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
        println!("  ✓ GH/s - Worker-style is fast!");
    } else {
        println!("  ✗ <GH/s - Worker-style is slow");
    }
}
