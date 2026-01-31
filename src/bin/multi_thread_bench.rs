//! Multi-thread Coordinator benchmark with verification
use std::time::{Duration, Instant};
use std::sync::atomic::Ordering;
use btcminer::mining::{Coordinator, Job, HasherType};

fn main() {
    let duration = Duration::from_secs(5);
    let thread_counts = [16, 17, 18, 19, 20];

    println!("Multi-thread Coordinator benchmark (SHA2 Crate)\n");

    for &thread_count in &thread_counts {
        // Give time for previous coordinator to fully stop
        std::thread::sleep(Duration::from_millis(1000));

        let coordinator = Coordinator::with_options(thread_count, 1 << 18, HasherType::Sha2Crate);

        // Verify stats start at 0
        let initial = coordinator.stats().get_hashes();
        if initial != 0 {
            println!("WARNING: Initial hash count is {} (should be 0)", initial);
        }

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
        std::thread::sleep(duration);

        let hashes = coordinator.stats().get_hashes();
        let elapsed = start.elapsed().as_secs_f64();
        let total_rate = hashes as f64 / elapsed;
        let per_thread = total_rate / thread_count as f64;

        coordinator.stop();

        // Wait for full stop
        std::thread::sleep(Duration::from_millis(500));

        // Check for GPU hashes (should be 0)
        let gpu_hashes = coordinator.stats().gpu_hashes.load(Ordering::Relaxed);
        if gpu_hashes != 0 {
            println!("WARNING: GPU hash count is {} (should be 0)", gpu_hashes);
        }

        // Calculate expected range
        let expected_min = thread_count as f64 * 15.0; // 15 MH/s min per thread
        let expected_max = thread_count as f64 * 30.0; // 30 MH/s max per thread
        let is_anomaly = total_rate / 1e6 < expected_min || total_rate / 1e6 > expected_max;

        println!("{:2} threads: {:>15} hashes = {:>10.2} MH/s total, {:>8.2} MH/s per thread {}",
                 thread_count, hashes, total_rate / 1e6, per_thread / 1e6,
                 if is_anomaly { "<-- ANOMALY" } else { "" });

        // Explicit drop
        drop(coordinator);
    }

    println!("\nExpected: per-thread rate should be roughly constant (~21-23 MH/s)");
    println!("If per-thread rate varies wildly, there's a bug in hash counting.");
}
