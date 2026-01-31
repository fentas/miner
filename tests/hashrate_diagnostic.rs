//! Hashrate Diagnostic Tests (Library-level)
//!
//! These tests diagnose hasher performance and identify bottlenecks.
//!
//! IMPORTANT - Cross-compilation-unit optimization:
//! - Tests that spawn threads DIRECTLY (Tests 2-4, 6 isolated): ~10-100 MH/s
//!   The threads run in the test binary, which cannot inline library functions.
//! - Tests using COORDINATOR (Tests 5-6 coordinator): ~73+ GH/s
//!   Coordinator spawns threads inside the library, allowing full optimization.
//!
//! ROOT CAUSE: #[target_feature] functions cannot be inlined across compilation
//! units. The sha2 crate uses runtime CPU dispatch without #[target_feature] on
//! its public API, so it benefits from cross-unit inlining when threads are
//! spawned inside the library.
//!
//! These tests are useful for:
//! - Comparing direct vs Coordinator performance (see root cause above)
//! - Relative performance comparisons (A vs B)
//! - Verifying SIMD detection works correctly
//!
//! Run with: cargo test --release --test hashrate_diagnostic -- --nocapture --ignored

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use btcminer::mining::{Coordinator, Hasher, Job, Midstate, SimdLevel};
use btcminer::mining::sha256_simd::mine_with_midstate;
use btcminer::stats::HashrateCalculator;

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

/// Test 1: Measure raw SIMD detection overhead
#[test]
#[ignore]
fn e2e_01_simd_detection_overhead() {
    println!("\n=== Test 1: SIMD Detection Overhead ===");

    let iterations = 10_000_000u64;

    // Measure SIMD detection time
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = SimdLevel::detect();
    }
    let elapsed = start.elapsed();

    let per_call_ns = elapsed.as_nanos() as f64 / iterations as f64;
    let calls_per_sec = 1e9 / per_call_ns;

    println!("SIMD detection:");
    println!("  {} iterations in {:?}", iterations, elapsed);
    println!("  {:.2} ns per call", per_call_ns);
    println!("  {:.2}M calls/sec", calls_per_sec / 1e6);

    // This should be < 10ns per call (cached by std)
    if per_call_ns > 50.0 {
        println!("  WARNING: SIMD detection is slow! Expected < 50ns, got {:.2}ns", per_call_ns);
    } else {
        println!("  OK: Detection is fast enough");
    }
}

/// Test 2: Isolated Hasher performance (no threading overhead)
#[test]
#[ignore]
fn e2e_02_isolated_hasher_performance() {
    println!("\n=== Test 2: Isolated Hasher Performance ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());

    let mut hasher = Hasher::new();
    let header = [0u8; 76];
    // Hard target - processes all nonces for accurate measurement
    let target = [0x00; 32];

    // Test different batch sizes
    let batch_sizes = [
        (1024, "1K"),
        (65536, "64K"),
        (262144, "256K (default)"),
        (1048576, "1M"),
    ];

    for (batch_size, name) in batch_sizes {
        // Warmup
        for _ in 0..10 {
            let _ = hasher.hash_nonce_range(&header, 0, batch_size, &target);
        }

        // Measure
        let duration = Duration::from_secs(3);
        let start = Instant::now();
        let mut nonce = 0u32;
        let mut total_hashes = 0u64;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let elapsed = start.elapsed();
        let hashrate = total_hashes as f64 / elapsed.as_secs_f64();

        println!("  Batch {}: {} ({} hashes in {:?})",
            name, format_hashrate(hashrate), total_hashes, elapsed);
    }
}

/// Test 3: Isolated mine_with_midstate (the core mining function)
#[test]
#[ignore]
fn e2e_03_mine_with_midstate_performance() {
    println!("\n=== Test 3: mine_with_midstate Performance ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());

    let header = [0u8; 64];
    let midstate = Midstate::from_header_prefix(&header);
    let tail = [0u8; 16];
    let target = [0x00; 32]; // Hard target for realistic measurement

    let batch_sizes = [
        (1024u32, "1K"),
        (65536u32, "64K"),
        (262144u32, "256K (default)"),
        (1048576u32, "1M"),
    ];

    for (batch_size, name) in batch_sizes {
        // Warmup
        for _ in 0..10 {
            let _ = mine_with_midstate(&midstate, &tail, 0, batch_size, &target);
        }

        // Measure
        let duration = Duration::from_secs(3);
        let start = Instant::now();
        let mut nonce = 0u32;
        let mut total_hashes = 0u64;

        while start.elapsed() < duration {
            let _ = mine_with_midstate(&midstate, &tail, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let elapsed = start.elapsed();
        let hashrate = total_hashes as f64 / elapsed.as_secs_f64();

        println!("  Batch {}: {} ({} hashes in {:?})",
            name, format_hashrate(hashrate), total_hashes, elapsed);
    }
}

/// Test 4: Multi-threaded hasher (simulating worker threads)
#[test]
#[ignore]
fn e2e_04_multithreaded_hasher() {
    println!("\n=== Test 4: Multi-threaded Hasher ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());

    let cpu_count = num_cpus::get();
    let thread_counts = if cpu_count >= 8 {
        vec![1, cpu_count / 2, cpu_count]
    } else {
        vec![1, cpu_count]
    };

    let batch_size = 262144u32; // Default batch size
    let duration = Duration::from_secs(5);

    for num_threads in thread_counts {
        let total_hashes = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let mut handles = Vec::new();
        for thread_id in 0..num_threads {
            let hashes = Arc::clone(&total_hashes);
            let run = Arc::clone(&running);

            let handle = thread::spawn(move || {
                let mut hasher = Hasher::new();
                let header = [0u8; 76];
                let target = [0x00; 32]; // Hard target for realistic measurement
                // Each thread starts at a different nonce range
                let mut nonce = (thread_id as u32) * (u32::MAX / num_threads as u32);

                while run.load(Ordering::Relaxed) {
                    let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
                    hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                    nonce = nonce.wrapping_add(batch_size);
                }
            });
            handles.push(handle);
        }

        let start = Instant::now();
        thread::sleep(duration);
        running.store(false, Ordering::SeqCst);

        for h in handles {
            let _ = h.join();
        }

        let hashes = total_hashes.load(Ordering::Relaxed);
        let elapsed = start.elapsed();
        let hashrate = hashes as f64 / elapsed.as_secs_f64();
        let per_thread = hashrate / num_threads as f64;

        println!("  {} threads: {} total ({}/thread)",
            num_threads, format_hashrate(hashrate), format_hashrate(per_thread));
    }
}

/// Test 5: Full Coordinator test (the real mining coordinator)
#[test]
#[ignore]
fn e2e_05_coordinator_performance() {
    println!("\n=== Test 5: Coordinator Performance (Real Mining) ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());

    let cpu_count = num_cpus::get();
    let thread_counts = if cpu_count >= 8 {
        vec![1, cpu_count / 2, cpu_count]
    } else {
        vec![1, cpu_count]
    };

    for num_threads in thread_counts {
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

        // Measure using stats
        let duration = Duration::from_secs(5);
        let start = Instant::now();

        // Let it run
        thread::sleep(duration);

        let stats = coordinator.stats().snapshot();
        let elapsed = start.elapsed();
        let hashrate = stats.hashes as f64 / elapsed.as_secs_f64();
        let per_thread = hashrate / num_threads as f64;

        coordinator.stop();

        println!("  {} threads: {} total ({}/thread) - {} hashes",
            num_threads, format_hashrate(hashrate), format_hashrate(per_thread), stats.hashes);
    }
}

/// Test 6: Compare coordinator vs isolated (identify overhead)
#[test]
#[ignore]
fn e2e_06_overhead_analysis() {
    println!("\n=== Test 6: Overhead Analysis ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());

    let num_threads = num_cpus::get();
    let batch_size = 262144u32;
    let duration = Duration::from_secs(5);

    // 1. Isolated multi-threaded hasher (baseline)
    let isolated_hashes = {
        let total_hashes = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let mut handles = Vec::new();
        for thread_id in 0..num_threads {
            let hashes = Arc::clone(&total_hashes);
            let run = Arc::clone(&running);

            let handle = thread::spawn(move || {
                let mut hasher = Hasher::new();
                let header = [0u8; 76];
                let target = [0x00; 32]; // Hard target for realistic measurement
                let mut nonce = (thread_id as u32) * (u32::MAX / num_threads as u32);

                while run.load(Ordering::Relaxed) {
                    let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
                    hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                    nonce = nonce.wrapping_add(batch_size);
                }
            });
            handles.push(handle);
        }

        thread::sleep(duration);
        running.store(false, Ordering::SeqCst);

        for h in handles {
            let _ = h.join();
        }

        total_hashes.load(Ordering::Relaxed)
    };

    let isolated_rate = isolated_hashes as f64 / duration.as_secs_f64();

    // 2. Coordinator (real mining path)
    let coordinator_hashes = {
        let coordinator = Coordinator::new(num_threads);

        let mut job = Job::new("test-job".to_string());
        job.version = 0x20000000;
        job.prev_hash = [0u8; 32];
        job.merkle_root_override = Some([0u8; 32]);
        job.ntime = 0x12345678;
        job.nbits = 0x1d00ffff;
        job.target = [0x00; 32]; // Hard target for realistic measurement
        job.clean_jobs = true;

        coordinator.update_job(job);
        coordinator.start();

        thread::sleep(duration);

        let stats = coordinator.stats().snapshot();
        coordinator.stop();

        stats.hashes
    };

    let coordinator_rate = coordinator_hashes as f64 / duration.as_secs_f64();

    // Analysis
    let overhead_pct = ((isolated_rate - coordinator_rate) / isolated_rate) * 100.0;

    println!("\nResults ({} threads, {} duration):", num_threads, duration.as_secs());
    println!("  Isolated hasher:   {}", format_hashrate(isolated_rate));
    println!("  Coordinator:       {}", format_hashrate(coordinator_rate));
    println!("  Overhead:          {:.1}%", overhead_pct);

    if overhead_pct > 20.0 {
        println!("\n  WARNING: High overhead detected! This indicates a bottleneck in the coordinator.");
    }

    // Expected performance check (realistic values with hard target)
    // SHA-NI: ~10 MH/s per thread sequential processing
    let expected_min = match SimdLevel::detect() {
        SimdLevel::ShaNi => 5_000_000.0 * num_threads as f64, // ~5 MH/s per thread minimum
        SimdLevel::Avx512 => 3_000_000.0 * num_threads as f64, // ~3 MH/s per thread
        SimdLevel::Avx2 => 2_000_000.0 * num_threads as f64, // ~2 MH/s per thread
        SimdLevel::Generic => 100_000.0 * num_threads as f64, // ~100 KH/s per thread
    };

    println!("\n  Expected minimum for {}: {}", SimdLevel::detect().name(), format_hashrate(expected_min));

    if coordinator_rate < expected_min {
        println!("  FAIL: Performance is below expected minimum!");
        println!("  Possible causes:");
        println!("    1. SIMD not being used (check SimdLevel::detect())");
        println!("    2. Thread contention or lock issues");
        println!("    3. Memory bandwidth limitation");
        println!("    4. Batch size too small");
    } else {
        println!("  OK: Performance meets expectations");
    }
}

/// Test 7: HashrateCalculator accuracy
#[test]
#[ignore]
fn e2e_07_hashrate_calculator_accuracy() {
    println!("\n=== Test 7: HashrateCalculator Accuracy ===");

    let known_rate = 1_000_000_000u64; // Simulate 1 GH/s
    let duration_secs = 5;
    let samples_per_sec = 2;

    let mut calc = HashrateCalculator::new(60);

    let hashes_per_sample = known_rate / samples_per_sec as u64;
    let mut cumulative = 0u64;

    for _ in 0..(duration_secs * samples_per_sec) {
        cumulative += hashes_per_sample;
        calc.add_sample(cumulative);
        thread::sleep(Duration::from_millis(1000 / samples_per_sec as u64));
    }

    let calculated_rate = calc.hashrate();
    let error_pct = ((calculated_rate - known_rate as f64) / known_rate as f64).abs() * 100.0;

    println!("  Expected:   {}", format_hashrate(known_rate as f64));
    println!("  Calculated: {}", format_hashrate(calculated_rate));
    println!("  Error:      {:.1}%", error_pct);

    if error_pct > 10.0 {
        println!("  WARNING: HashrateCalculator may have accuracy issues");
    } else {
        println!("  OK: Calculator is accurate");
    }
}

/// Test 8: Quick diagnostic - run all tests in sequence
#[test]
#[ignore]
fn e2e_full_diagnostic() {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           E2E Hashrate Diagnostic Suite                       ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    println!("\nSystem Info:");
    println!("  CPU cores: {}", num_cpus::get());
    println!("  SIMD level: {}", SimdLevel::detect().name());
    println!("  OS: {}", std::env::consts::OS);
    println!("  Arch: {}", std::env::consts::ARCH);

    e2e_01_simd_detection_overhead();
    e2e_02_isolated_hasher_performance();
    e2e_03_mine_with_midstate_performance();
    e2e_04_multithreaded_hasher();
    e2e_05_coordinator_performance();
    e2e_06_overhead_analysis();
    e2e_07_hashrate_calculator_accuracy();

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    Diagnostic Complete                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
}
