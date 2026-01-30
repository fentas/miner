//! Profiling tests for mining performance
//!
//! Run with: cargo test --release profiling -- --nocapture --ignored
//! Or: cargo test --release profile_ -- --nocapture --ignored
//!
//! These tests are marked as #[ignore] because they take time to run.
//! Use --ignored flag to run them explicitly.

use std::time::Duration;
use std::fs;
use btcminer::profiling::{Profiler, BenchmarkSuite, BenchmarkResult, print_results, format_hashrate};
use btcminer::mining::{SimdLevel, Hasher, Midstate};

/// Run a quick profiling test (good for CI)
#[test]
#[ignore]
fn profile_quick_benchmark() {
    println!("\n=== Quick Profiling Test ===\n");

    let profiler = Profiler::new()
        .with_warmup(Duration::from_millis(500))
        .with_duration(Duration::from_secs(2));

    let suite = profiler.run_quick_suite();
    print_results(&suite);

    // Verify all results have positive hashrate
    for result in &suite.results {
        assert!(result.hashrate > 0.0, "Benchmark {} should have positive hashrate", result.name);
    }
}

/// Run full benchmark suite
#[test]
#[ignore]
fn profile_full_benchmark() {
    println!("\n=== Full Profiling Test ===\n");

    let profiler = Profiler::new()
        .with_warmup(Duration::from_secs(1))
        .with_duration(Duration::from_secs(5));

    let suite = profiler.run_full_suite();
    print_results(&suite);

    // Export results
    let json = suite.to_json().expect("Failed to serialize JSON");
    let csv = suite.to_csv();

    println!("\n--- JSON Output ---");
    println!("{}", json);

    println!("\n--- CSV Output ---");
    println!("{}", csv);

    // Save to files
    let _ = fs::write("benchmark_results.json", &json);
    let _ = fs::write("benchmark_results.csv", &csv);
    println!("\nResults saved to benchmark_results.json and benchmark_results.csv");
}

/// Profile different thread counts
#[test]
#[ignore]
fn profile_thread_scaling() {
    println!("\n=== Thread Scaling Profile ===\n");

    let cpu_count = num_cpus::get();
    let mut thread_counts: Vec<usize> = (1..=cpu_count).collect();

    // Also test hyperthreading if available
    if cpu_count > 1 {
        thread_counts.push(cpu_count);
    }

    let profiler = Profiler::new()
        .with_warmup(Duration::from_millis(500))
        .with_duration(Duration::from_secs(3));

    let results = profiler.bench_multi_thread(&thread_counts);

    let mut suite = BenchmarkSuite::new("Thread Scaling");
    for result in results {
        suite.add_result(result);
    }

    print_results(&suite);

    // Print scaling efficiency
    println!("\n--- Scaling Efficiency ---");
    if let Some(single) = suite.results.first() {
        let single_hashrate = single.hashrate;
        for result in &suite.results {
            let threads: usize = result.metadata.get("threads")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            let efficiency = (result.hashrate / (single_hashrate * threads as f64)) * 100.0;
            println!(
                "{} threads: {} ({:.1}% efficiency)",
                threads,
                format_hashrate(result.hashrate),
                efficiency
            );
        }
    }

    // Save CSV for plotting
    let csv = suite.to_csv();
    let _ = fs::write("thread_scaling.csv", &csv);
    println!("\nResults saved to thread_scaling.csv");
}

/// Profile different batch sizes
#[test]
#[ignore]
fn profile_batch_sizes() {
    println!("\n=== Batch Size Profile ===\n");

    let batch_sizes: Vec<u32> = vec![
        256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
        131072, 262144, 524288, 1048576,
    ];

    let profiler = Profiler::new()
        .with_warmup(Duration::from_millis(300))
        .with_duration(Duration::from_secs(2));

    let results = profiler.bench_mining_batch_sizes(&batch_sizes);

    let mut suite = BenchmarkSuite::new("Batch Size Comparison");
    for result in results {
        suite.add_result(result);
    }

    print_results(&suite);

    // Find optimal batch size
    if let Some(best) = suite.sorted_by_hashrate().first() {
        let batch_size = best.metadata.get("batch_size").cloned().unwrap_or_default();
        println!("\nOptimal batch size: {} ({})", batch_size, format_hashrate(best.hashrate));
    }

    // Save CSV for plotting
    let csv = suite.to_csv();
    let _ = fs::write("batch_sizes.csv", &csv);
    println!("Results saved to batch_sizes.csv");
}

/// Profile SIMD capabilities
#[test]
#[ignore]
fn profile_simd_info() {
    println!("\n=== SIMD Capabilities ===\n");

    let level = SimdLevel::detect();
    println!("Detected SIMD level: {}", level.name());

    #[cfg(target_arch = "x86_64")]
    {
        println!("\nCPU Features:");
        println!("  SHA-NI:  {}", if is_x86_feature_detected!("sha") { "Yes" } else { "No" });
        println!("  SSE4.1: {}", if is_x86_feature_detected!("sse4.1") { "Yes" } else { "No" });
        println!("  AVX2:   {}", if is_x86_feature_detected!("avx2") { "Yes" } else { "No" });
        println!("  AVX512: {}", if is_x86_feature_detected!("avx512f") { "Yes" } else { "No" });
    }

    println!("\nCPU cores: {}", num_cpus::get());
    println!("Physical cores: {}", num_cpus::get_physical());
}

/// Profile hasher performance at different intensities
#[test]
#[ignore]
fn profile_hasher_performance() {
    println!("\n=== Hasher Performance Profile ===\n");

    let mut suite = BenchmarkSuite::new("Hasher Performance");

    // Test with different batch sizes to simulate intensity
    let batch_sizes = [16384, 32768, 65536, 131072, 262144];

    for &batch_size in &batch_sizes {
        let mut hasher = Hasher::new();
        let header = [0u8; 76];
        let target = [0xFF; 32];

        // Warmup
        for _ in 0..10 {
            let _ = hasher.hash_nonce_range(&header, 0, batch_size, &target);
        }

        // Benchmark
        let start = std::time::Instant::now();
        let mut nonce = 0u32;
        let iterations = 100u64;

        for _ in 0..iterations {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            nonce = nonce.wrapping_add(batch_size);
        }

        let elapsed = start.elapsed();
        let total_hashes = iterations * batch_size as u64;

        let result = BenchmarkResult::new(
            "hasher",
            &format!("batch={}", batch_size),
            total_hashes,
            elapsed,
        )
        .with_metadata("batch_size", &batch_size.to_string());

        suite.add_result(result);
    }

    print_results(&suite);

    let csv = suite.to_csv();
    let _ = fs::write("hasher_performance.csv", &csv);
    println!("\nResults saved to hasher_performance.csv");
}

/// Profile sustained mining over time
#[test]
#[ignore]
fn profile_sustained_mining() {
    println!("\n=== Sustained Mining Profile (30 seconds) ===\n");

    use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;

    let duration = Duration::from_secs(30);
    let sample_interval = Duration::from_secs(1);

    let total_hashes = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // Use all available threads
    let num_threads = num_cpus::get();
    let batch_size = 65536u32;

    // Spawn workers
    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let hashes = Arc::clone(&total_hashes);
        let run = Arc::clone(&running);

        let handle = thread::spawn(move || {
            let mut hasher = Hasher::new();
            let header = [0u8; 76];
            let target = [0xFF; 32];
            let mut nonce = 0u32;

            while run.load(Ordering::Relaxed) {
                let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
                hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                nonce = nonce.wrapping_add(batch_size);
            }
        });
        handles.push(handle);
    }

    // Collect samples
    let mut samples: Vec<(f64, f64)> = Vec::new(); // (time_secs, hashrate)
    let start = std::time::Instant::now();
    let mut last_hashes = 0u64;
    let mut last_time = start;

    println!("Time(s)  Hashrate");
    println!("-------  --------");

    while start.elapsed() < duration {
        thread::sleep(sample_interval);

        let now = std::time::Instant::now();
        let current_hashes = total_hashes.load(Ordering::Relaxed);
        let delta_hashes = current_hashes - last_hashes;
        let delta_time = now.duration_since(last_time).as_secs_f64();

        let hashrate = delta_hashes as f64 / delta_time;
        let elapsed = start.elapsed().as_secs_f64();

        samples.push((elapsed, hashrate));
        println!("{:>7.1}  {}", elapsed, format_hashrate(hashrate));

        last_hashes = current_hashes;
        last_time = now;
    }

    running.store(false, Ordering::SeqCst);

    for h in handles {
        let _ = h.join();
    }

    // Calculate statistics
    let hashrates: Vec<f64> = samples.iter().map(|(_, h)| *h).collect();
    let avg_hashrate: f64 = hashrates.iter().sum::<f64>() / hashrates.len() as f64;
    let max_hashrate = hashrates.iter().cloned().fold(0.0, f64::max);
    let min_hashrate = hashrates.iter().cloned().fold(f64::MAX, f64::min);
    let variance: f64 = hashrates.iter().map(|h| (h - avg_hashrate).powi(2)).sum::<f64>() / hashrates.len() as f64;
    let stddev = variance.sqrt();

    println!("\n--- Statistics ---");
    println!("Threads:     {}", num_threads);
    println!("Duration:    {:.1}s", duration.as_secs_f64());
    println!("Average:     {}", format_hashrate(avg_hashrate));
    println!("Maximum:     {}", format_hashrate(max_hashrate));
    println!("Minimum:     {}", format_hashrate(min_hashrate));
    println!("Std Dev:     {}", format_hashrate(stddev));
    println!("Variation:   {:.2}%", (stddev / avg_hashrate) * 100.0);

    // Save time series data for plotting
    let mut csv = String::from("time_secs,hashrate\n");
    for (t, h) in &samples {
        csv.push_str(&format!("{:.1},{:.2}\n", t, h));
    }
    let _ = fs::write("sustained_mining.csv", &csv);
    println!("\nTime series saved to sustained_mining.csv");
}

/// Profile midstate computation
#[test]
#[ignore]
fn profile_midstate_computation() {
    println!("\n=== Midstate Computation Profile ===\n");

    let header = [0xABu8; 64];
    let iterations = 1_000_000u64;

    // Warmup
    for _ in 0..10000 {
        let _ = Midstate::from_header_prefix(&header);
    }

    // Benchmark
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = Midstate::from_header_prefix(&header);
    }
    let elapsed = start.elapsed();

    let rate = iterations as f64 / elapsed.as_secs_f64();
    println!("Midstate computations: {} in {:.3}s", iterations, elapsed.as_secs_f64());
    println!("Rate: {:.2} midstates/sec", rate);
    println!("Time per midstate: {:.2} ns", (elapsed.as_nanos() as f64) / (iterations as f64));
}

/// Generate a comparison report between different configurations
#[test]
#[ignore]
fn profile_comparison_report() {
    println!("\n=== Configuration Comparison Report ===\n");

    let profiler = Profiler::new()
        .with_warmup(Duration::from_millis(500))
        .with_duration(Duration::from_secs(3));

    let mut all_results: Vec<BenchmarkResult> = Vec::new();

    // Single thread baseline
    println!("Testing single thread...");
    let single = profiler.bench_multi_thread(&[1]);
    all_results.extend(single);

    // Half threads
    let half = num_cpus::get() / 2;
    if half > 1 {
        println!("Testing {} threads...", half);
        let results = profiler.bench_multi_thread(&[half]);
        all_results.extend(results);
    }

    // All threads
    println!("Testing {} threads (all cores)...", num_cpus::get());
    let full = profiler.bench_multi_thread(&[num_cpus::get()]);
    all_results.extend(full);

    // Batch size comparison at full threads
    println!("Testing batch sizes...");
    let batch_results = profiler.bench_mining_batch_sizes(&[16384, 65536, 262144]);
    all_results.extend(batch_results);

    // Create final report
    let mut suite = BenchmarkSuite::new("Configuration Comparison");
    for result in all_results {
        suite.add_result(result);
    }

    print_results(&suite);

    // JSON report
    let json = suite.to_json().expect("Failed to serialize");
    let _ = fs::write("comparison_report.json", &json);
    println!("\nFull report saved to comparison_report.json");
}

/// Simple non-ignored test to verify profiling module loads
#[test]
fn test_profiling_module_loads() {
    let profiler = Profiler::new();
    assert_eq!(profiler.warmup_duration, Duration::from_secs(1));
    assert_eq!(profiler.bench_duration, Duration::from_secs(5));
}

/// Test benchmark result creation
#[test]
fn test_benchmark_result_creation() {
    let result = BenchmarkResult::new("test", "config", 1000000, Duration::from_secs(1));
    assert_eq!(result.name, "test");
    assert_eq!(result.config, "config");
    assert_eq!(result.hashes, 1000000);
    assert!((result.hashrate - 1000000.0).abs() < 0.1);
}

/// Test suite CSV export
#[test]
fn test_suite_csv_export() {
    let mut suite = BenchmarkSuite::new("test");
    suite.add_result(BenchmarkResult::new("bench1", "cfg1", 1000, Duration::from_secs(1)));
    suite.add_result(BenchmarkResult::new("bench2", "cfg2", 2000, Duration::from_secs(1)));

    let csv = suite.to_csv();
    assert!(csv.contains("name,config,hashes"));
    assert!(csv.contains("bench1,cfg1,1000"));
    assert!(csv.contains("bench2,cfg2,2000"));
}
