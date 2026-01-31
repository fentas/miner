//! Single-thread Coordinator benchmark to verify hash counts
use std::time::{Duration, Instant};
use std::hint::black_box;
use btcminer::mining::{Coordinator, Job, HasherType};

fn main() {
    let duration = Duration::from_secs(5);
    let thread_count = 1; // Single thread for accurate measurement

    println!("Single-thread Coordinator benchmark ({} seconds per test)\n", duration.as_secs());

    let hasher_types = [
        (HasherType::Sha2Crate, "SHA2 Crate"),
        (HasherType::AsmV3, "AsmV3"),
    ];

    for (hasher_type, name) in hasher_types {
        println!("Testing {}...", name);

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

        coordinator.stop();

        println!("  {} hashes in {:.1}s = {:.2} MH/s", hashes, elapsed, rate / 1e6);
        black_box(hashes);
    }
}
