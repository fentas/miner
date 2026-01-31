//! Isolated single-config Coordinator benchmark
//! Run with: cargo run --release --bin isolated_bench -- <thread_count> [hasher]
//! hasher: sha2 (default), simd, asm, v2, v3
use std::env;
use std::time::{Duration, Instant};
use btcminer::mining::{Coordinator, Job, HasherType};

fn main() {
    let args: Vec<String> = env::args().collect();
    let thread_count: usize = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    let hasher_type = match args.get(2).map(|s| s.as_str()) {
        Some("simd") => HasherType::Simd,
        Some("asm") => HasherType::Assembly,
        Some("v2") => HasherType::AsmV2,
        Some("v3") => HasherType::AsmV3,
        _ => HasherType::Sha2Crate,
    };

    println!("Isolated benchmark: {} threads, {:?}, 5 seconds\n", thread_count, hasher_type);

    let coordinator = Coordinator::with_options(thread_count, 1 << 18, hasher_type);

    // Verify stats start at 0
    let initial = coordinator.stats().get_hashes();
    println!("Initial hash count: {} (should be 0)", initial);

    let mut job = Job::new("bench-job".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0xABu8; 32];
    job.merkle_root_override = Some([0xCDu8; 32]);
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.target = [0x00; 32];
    job.clean_jobs = true;

    coordinator.update_job(job);
    coordinator.start();

    let start = Instant::now();

    // Print progress every second
    for i in 1..=5 {
        std::thread::sleep(Duration::from_secs(1));
        let hashes = coordinator.stats().get_hashes();
        let elapsed = start.elapsed().as_secs_f64();
        let rate = hashes as f64 / elapsed / 1e6;
        println!("  {}s: {} hashes = {:.2} MH/s total", i, hashes, rate);
    }

    let hashes = coordinator.stats().get_hashes();
    let elapsed = start.elapsed().as_secs_f64();
    let total_rate = hashes as f64 / elapsed;
    let per_thread = total_rate / thread_count as f64;

    coordinator.stop();

    println!("\nFinal: {} hashes in {:.1}s", hashes, elapsed);
    println!("Total rate: {:.2} MH/s", total_rate / 1e6);
    println!("Per-thread: {:.2} MH/s", per_thread / 1e6);

    // Expected: ~22 MH/s per thread
    let expected = thread_count as f64 * 22.0;
    if total_rate / 1e6 > expected * 10.0 {
        println!("\nANOMALY DETECTED: Rate is {:.0}x higher than expected!", total_rate / 1e6 / expected);
    }
}
