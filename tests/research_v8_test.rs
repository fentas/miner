//! Research Experiment v8: Multi-Thread Scaling Analysis
//!
//! Tests how hashrate scales with thread count.
//! Identifies diminishing returns and optimal thread count.
//!
//! Correlates to: docs/RESEARCH-test-log-v8.md
//! Run with: cargo test --release --test research_v8_test -- --nocapture --ignored

use std::time::{Instant, Duration};
use std::thread;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use btcminer::mining::{Hasher, HasherType};

/// Create a test header prefix
fn make_header_prefix(seed: u64) -> [u8; 76] {
    let mut prefix = [0u8; 76];
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        prefix[i] = (state >> 56) as u8;
    }
    prefix
}

/// Worker function that hashes until stopped
fn hash_worker(
    id: u32,
    header_prefix: [u8; 76],
    hash_count: Arc<AtomicU64>,
    should_stop: Arc<AtomicBool>,
) {
    let mut hasher = Hasher::with_type(HasherType::Simd);
    let hard_target = [0u8; 32];
    let batch_size = 10_000u32;

    // Each worker gets a different nonce range
    let mut nonce = id.wrapping_mul(0x10000000);

    while !should_stop.load(Ordering::Relaxed) {
        let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
        hash_count.fetch_add(batch_size as u64, Ordering::Relaxed);
        nonce = nonce.wrapping_add(batch_size);
    }
}

/// Benchmark with a specific number of threads
fn benchmark_threads(num_threads: usize, duration_secs: u64) -> f64 {
    let header_prefix = make_header_prefix(12345);
    let hash_count = Arc::new(AtomicU64::new(0));
    let should_stop = Arc::new(AtomicBool::new(false));

    // Spawn worker threads
    let mut handles = Vec::new();
    for id in 0..num_threads {
        let prefix = header_prefix;
        let count = Arc::clone(&hash_count);
        let stop = Arc::clone(&should_stop);

        handles.push(thread::spawn(move || {
            hash_worker(id as u32, prefix, count, stop);
        }));
    }

    // Wait for duration
    thread::sleep(Duration::from_secs(duration_secs));

    // Signal stop and collect results
    should_stop.store(true, Ordering::Relaxed);

    // Wait for all threads to finish
    for handle in handles {
        let _ = handle.join();
    }

    let total_hashes = hash_count.load(Ordering::Relaxed);
    total_hashes as f64 / duration_secs as f64 / 1_000_000.0
}

/// Test 1: Thread Scaling (this test takes a while, marked as ignored)
#[test]
#[ignore]
fn test_research_v8_thread_scaling() {
    println!("\n=== Research v8: Thread Scaling Analysis ===\n");

    let available_cpus = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    println!("Available CPUs: {}", available_cpus);
    println!("Testing thread counts: 1 to {}\n", available_cpus);

    let duration = 3; // seconds per test

    println!("| Threads | Rate (MH/s) | Per Thread | Efficiency | Scaling |");
    println!("|---------|-------------|------------|------------|---------|");

    let mut single_thread_rate = 0.0f64;

    for num_threads in 1..=available_cpus {
        let rate = benchmark_threads(num_threads, duration);

        if num_threads == 1 {
            single_thread_rate = rate;
        }

        let per_thread = rate / num_threads as f64;
        let efficiency = per_thread / single_thread_rate * 100.0;
        let scaling = rate / single_thread_rate;

        println!("| {:>7} | {:>11.2} | {:>10.2} | {:>9.1}% | {:>6.2}x |",
            num_threads, rate, per_thread, efficiency, scaling);
    }

    // Calculate Amdahl's Law fit
    if available_cpus > 1 {
        let final_rate = benchmark_threads(available_cpus, duration);
        let theoretical_max = single_thread_rate * available_cpus as f64;
        let actual_efficiency = final_rate / theoretical_max * 100.0;

        println!("\nTheoretical max ({} threads): {:.2} MH/s", available_cpus, theoretical_max);
        println!("Actual ({} threads): {:.2} MH/s", available_cpus, final_rate);
        println!("Parallel efficiency: {:.1}%", actual_efficiency);
    }
}

/// Test 2: Quick scaling test (fewer threads, shorter duration)
#[test]
fn test_research_v8_quick_scaling() {
    println!("\n=== Research v8: Quick Thread Scaling ===\n");

    let max_threads = thread::available_parallelism()
        .map(|n| n.get().min(8))  // Limit to 8 threads for quick test
        .unwrap_or(4);

    println!("Testing up to {} threads\n", max_threads);

    let duration = 2; // shorter duration

    println!("| Threads | Rate (MH/s) | Scaling |");
    println!("|---------|-------------|---------|");

    let single = benchmark_threads(1, duration);
    println!("| {:>7} | {:>11.2} | {:>6.2}x |", 1, single, 1.0);

    for &num in &[2, 4, max_threads] {
        if num > 1 && num <= max_threads {
            let rate = benchmark_threads(num, duration);
            let scaling = rate / single;
            println!("| {:>7} | {:>11.2} | {:>6.2}x |", num, rate, scaling);
        }
    }
}

/// Test 3: Hyperthreading impact
#[test]
fn test_research_v8_hyperthreading() {
    println!("\n=== Research v8: Hyperthreading Impact ===\n");

    let total_cpus = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Assume hyperthreading means logical = 2 * physical
    let physical_estimate = total_cpus / 2;

    if physical_estimate < 2 {
        println!("Not enough CPUs to test hyperthreading.");
        return;
    }

    let duration = 2;

    println!("Total logical CPUs: {}", total_cpus);
    println!("Estimated physical cores: {}\n", physical_estimate);

    let physical_rate = benchmark_threads(physical_estimate, duration);
    let logical_rate = benchmark_threads(total_cpus, duration);

    let ht_benefit = logical_rate / physical_rate;

    println!("| CPU Count | Rate (MH/s) | Relative |");
    println!("|-----------|-------------|----------|");
    println!("| {} (physical est.) | {:>11.2} | {:>7.2}x |", physical_estimate, physical_rate, 1.0);
    println!("| {} (logical)       | {:>11.2} | {:>7.2}x |", total_cpus, logical_rate, ht_benefit);

    println!("\nHyperthreading benefit: {:.1}% additional performance",
        (ht_benefit - 1.0) * 100.0);

    if ht_benefit < 1.1 {
        println!("Note: Minimal HT benefit - SHA-256 may be compute-bound");
    } else if ht_benefit > 1.5 {
        println!("Note: Good HT benefit - likely memory or cache limited");
    }
}

/// Test 4: Per-core consistency
#[test]
fn test_research_v8_per_core_consistency() {
    println!("\n=== Research v8: Per-Core Consistency ===\n");
    println!("Testing if all cores perform equally.\n");

    let num_cores = thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(4);

    let duration = 2;
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];

    println!("| Core | Rate (MH/s) | Deviation |");
    println!("|------|-------------|-----------|");

    let mut rates: Vec<f64> = Vec::new();

    for core_id in 0..num_cores {
        // Run single-threaded test on this logical core
        // (Note: we can't actually pin to a core without platform-specific code,
        // so this tests sequential isolation instead)

        let mut hasher = Hasher::with_type(HasherType::Simd);
        let batch_size = 10_000u32;
        let start = Instant::now();
        let mut total = 0u64;
        let mut nonce = (core_id as u32).wrapping_mul(0x10000000);

        while start.elapsed() < Duration::from_secs(duration) {
            let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
            total += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let rate = total as f64 / start.elapsed().as_secs_f64() / 1_000_000.0;
        rates.push(rate);
    }

    let avg: f64 = rates.iter().sum::<f64>() / rates.len() as f64;

    for (core_id, &rate) in rates.iter().enumerate() {
        let deviation = (rate - avg) / avg * 100.0;
        println!("| {:>4} | {:>11.2} | {:>+8.2}% |", core_id, rate, deviation);
    }

    let variance: f64 = rates.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / rates.len() as f64;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg * 100.0;

    println!("\nAverage: {:.2} MH/s", avg);
    println!("Std Dev: {:.2} MH/s", std_dev);
    println!("CV: {:.2}%", cv);

    if cv < 5.0 {
        println!("Result: CONSISTENT ✓");
    } else {
        println!("Result: INCONSISTENT - check for thermal throttling or P/E cores");
    }
}

/// Test 5: Contention analysis
#[test]
fn test_research_v8_contention() {
    println!("\n=== Research v8: Contention Analysis ===\n");
    println!("Testing if threads contend for shared resources.\n");

    let duration = 2;

    // Compare: N threads running vs N sequential runs
    let num_threads = 4;

    // Sequential: 4 runs, one at a time
    let mut sequential_total = 0.0f64;
    for _ in 0..num_threads {
        sequential_total += benchmark_threads(1, duration);
    }
    let sequential_rate = sequential_total; // Total if run in parallel without contention

    // Parallel: 4 threads at once
    let parallel_rate = benchmark_threads(num_threads, duration);

    let efficiency = parallel_rate / sequential_rate * 100.0;

    println!("| Mode | Rate (MH/s) | Efficiency |");
    println!("|------|-------------|------------|");
    println!("| {} × 1 thread (sequential) | {:>11.2} | 100.0% |", num_threads, sequential_rate);
    println!("| {} threads (parallel)       | {:>11.2} | {:>5.1}% |", num_threads, parallel_rate, efficiency);

    if efficiency > 95.0 {
        println!("\nNo significant contention detected ✓");
    } else if efficiency > 80.0 {
        println!("\nModerate contention - check memory bandwidth");
    } else {
        println!("\nHigh contention - investigate shared resources");
    }
}
