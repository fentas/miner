//! Test 6: Direct Worker::spawn usage
//!
//! Tests if calling Worker::spawn directly (library-created threads) is fast.
//! This confirms/disproves the cross-compilation-unit threading theory.
//!
//! Run with:
//!   cargo test --release --test worker_spawn_direct_bench -- --nocapture --ignored

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use btcminer::mining::{Worker, Notifier, Stats, Job, HasherType};
use tokio::sync::{mpsc, watch};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 { format!("{:.2} TH/s", hashrate / 1e12) }
    else if hashrate >= 1e9 { format!("{:.2} GH/s", hashrate / 1e9) }
    else if hashrate >= 1e6 { format!("{:.2} MH/s", hashrate / 1e6) }
    else if hashrate >= 1e3 { format!("{:.2} KH/s", hashrate / 1e3) }
    else { format!("{:.2} H/s", hashrate) }
}

/// Test using Worker::spawn directly (library creates threads)
#[test]
#[ignore]
fn worker_spawn_direct() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Test 6: Direct Worker::spawn (Library Thread Creation)      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let thread_count = num_cpus::get();
    let duration = Duration::from_secs(10);
    let batch_size = 262144u32;
    let hasher_type = HasherType::Sha2Crate;

    println!("Threads:     {}", thread_count);
    println!("Duration:    {:?}", duration);
    println!("Batch size:  {}", batch_size);
    println!("Hasher type: {:?}", hasher_type);
    println!();

    // Create shared state exactly like Coordinator does
    let stats = Arc::new(Stats::new());
    let running = Arc::new(AtomicBool::new(false));
    let notifier = Arc::new(Notifier::new());
    let (job_tx, _job_rx) = watch::channel(None::<Arc<Job>>);
    let (share_tx, _share_rx) = mpsc::channel(100);

    // Spawn workers using Worker::spawn (threads created INSIDE library)
    let mut handles = Vec::new();
    for id in 0..thread_count {
        let handle = Worker::spawn(
            id,
            Arc::clone(&stats),
            Arc::clone(&running),
            Arc::clone(&notifier),
            job_tx.subscribe(),
            share_tx.clone(),
            batch_size,
            hasher_type,
        ).expect("Failed to spawn worker");
        handles.push(handle);
    }

    // Create a job with hard target (no early exit)
    let mut job = Job::new("test-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0u8; 32];
    job.merkle_root_override = Some([0u8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.target = [0x00; 32]; // Hard target - no early exit
    job.clean_jobs = true;

    // Send job to workers
    let _ = job_tx.send(Some(Arc::new(job)));
    notifier.notify_all();

    // Start workers
    running.store(true, Ordering::Release);
    notifier.notify_all();

    let start = Instant::now();

    println!("Running...");
    while start.elapsed() < duration {
        std::thread::sleep(Duration::from_secs(2));
        let elapsed = start.elapsed().as_secs_f64();
        let h = stats.get_hashes();
        let rate = h as f64 / elapsed;
        println!("  {:>5.1}s: {} ({} hashes)", elapsed, format_hashrate(rate), h);
    }

    // Stop workers
    running.store(false, Ordering::Release);
    notifier.notify_all();

    // Wait for threads to finish
    for h in handles {
        let _ = h.join();
    }

    let final_hashes = stats.get_hashes();
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

    // Compare to Test 5 (same code but threads created in test)
    let test5_rate = 103e6; // 103 MH/s from Test 5
    let speedup = final_rate / test5_rate;

    println!("  Comparison:");
    println!("    vs Test 5 (test-created threads): {:.1}x", speedup);
    println!();

    if per_thread > 1e9 {
        println!("  ✓ GH/s - Worker::spawn is fast!");
        println!("    CONFIRMED: Library-created threads are optimized better");
    } else if per_thread > 100e6 {
        println!("  ~ 100+ MH/s - Moderate improvement");
    } else {
        println!("  ✗ <100 MH/s - Worker::spawn is also slow");
        println!("    DISPROVED: Thread creation location is NOT the issue");
    }
}
