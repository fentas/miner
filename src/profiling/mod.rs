//! Profiling and benchmarking module
//!
//! Provides tools for measuring mining performance across different configurations:
//! - CPU thread counts
//! - GPU intensity levels
//! - SIMD modes
//! - Batch sizes
//!
//! Results can be exported as JSON or CSV for plotting.

use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

use crate::mining::{Hasher, Midstate, SimdLevel};
use crate::mining::sha256_simd::{sha256d_generic, mine_with_midstate};

/// A single benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Configuration description
    pub config: String,
    /// Number of hashes computed
    pub hashes: u64,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Hashrate in H/s
    pub hashrate: f64,
    /// Additional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl BenchmarkResult {
    pub fn new(name: &str, config: &str, hashes: u64, duration: Duration) -> Self {
        let duration_secs = duration.as_secs_f64();
        let hashrate = if duration_secs > 0.0 {
            hashes as f64 / duration_secs
        } else {
            0.0
        };

        Self {
            name: name.to_string(),
            config: config.to_string(),
            hashes,
            duration_secs,
            hashrate,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Collection of benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    /// Suite name
    pub name: String,
    /// Timestamp when the suite was run
    pub timestamp: String,
    /// System info
    pub system_info: SystemInfo,
    /// Individual results
    pub results: Vec<BenchmarkResult>,
}

/// System information for benchmark context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub cpu_count: usize,
    pub simd_level: String,
    pub os: String,
    pub arch: String,
}

impl SystemInfo {
    pub fn detect() -> Self {
        Self {
            cpu_count: num_cpus::get(),
            simd_level: SimdLevel::detect().name().to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

impl BenchmarkSuite {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            system_info: SystemInfo::detect(),
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// Export results as JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Export results as CSV
    pub fn to_csv(&self) -> String {
        let mut csv = String::from("name,config,hashes,duration_secs,hashrate\n");
        for r in &self.results {
            csv.push_str(&format!(
                "{},{},{},{:.6},{:.2}\n",
                r.name, r.config, r.hashes, r.duration_secs, r.hashrate
            ));
        }
        csv
    }

    /// Get results sorted by hashrate (descending)
    pub fn sorted_by_hashrate(&self) -> Vec<&BenchmarkResult> {
        let mut results: Vec<_> = self.results.iter().collect();
        results.sort_by(|a, b| b.hashrate.partial_cmp(&a.hashrate).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// Profiler for running benchmarks
pub struct Profiler {
    /// Warmup duration before measuring
    pub warmup_duration: Duration,
    /// Benchmark duration for each test
    pub bench_duration: Duration,
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            warmup_duration: Duration::from_secs(1),
            bench_duration: Duration::from_secs(5),
        }
    }

    pub fn with_warmup(mut self, duration: Duration) -> Self {
        self.warmup_duration = duration;
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.bench_duration = duration;
        self
    }

    /// Benchmark SHA256d generic implementation
    pub fn bench_sha256d_generic(&self) -> BenchmarkResult {
        let data = [0xABu8; 80]; // Typical block header size

        // Warmup
        let warmup_end = Instant::now() + self.warmup_duration;
        while Instant::now() < warmup_end {
            let _ = sha256d_generic(&data);
        }

        // Benchmark
        let start = Instant::now();
        let mut hashes = 0u64;
        while start.elapsed() < self.bench_duration {
            for _ in 0..1000 {
                let _ = sha256d_generic(&data);
            }
            hashes += 1000;
        }

        BenchmarkResult::new("sha256d_generic", "single-thread", hashes, start.elapsed())
            .with_metadata("simd", SimdLevel::detect().name())
    }

    /// Benchmark mining with midstate at different batch sizes
    pub fn bench_mining_batch_sizes(&self, batch_sizes: &[u32]) -> Vec<BenchmarkResult> {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let target = [0xFF; 32]; // Easy target - won't find shares

        let mut results = Vec::new();

        for &batch_size in batch_sizes {
            // Warmup
            let warmup_end = Instant::now() + self.warmup_duration;
            while Instant::now() < warmup_end {
                let _ = mine_with_midstate(&midstate, &tail, 0, batch_size, &target);
            }

            // Benchmark
            let start = Instant::now();
            let mut nonce = 0u32;
            let mut total_hashes = 0u64;

            while start.elapsed() < self.bench_duration {
                let _ = mine_with_midstate(&midstate, &tail, nonce, batch_size, &target);
                total_hashes += batch_size as u64;
                nonce = nonce.wrapping_add(batch_size);
            }

            let result = BenchmarkResult::new(
                "mine_with_midstate",
                &format!("batch_size={}", batch_size),
                total_hashes,
                start.elapsed(),
            )
            .with_metadata("batch_size", &batch_size.to_string())
            .with_metadata("simd", SimdLevel::detect().name());

            results.push(result);
        }

        results
    }

    /// Benchmark hasher with different thread counts (simulated via iterations)
    pub fn bench_hasher_throughput(&self) -> BenchmarkResult {
        let mut hasher = Hasher::new();
        let header = [0u8; 76];
        let target = [0xFF; 32];
        let batch_size = 65536u32;

        // Warmup
        let warmup_end = Instant::now() + self.warmup_duration;
        while Instant::now() < warmup_end {
            let _ = hasher.hash_nonce_range(&header, 0, batch_size, &target);
        }

        // Benchmark
        let start = Instant::now();
        let mut nonce = 0u32;
        let mut total_hashes = 0u64;

        while start.elapsed() < self.bench_duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        BenchmarkResult::new("hasher", &format!("batch_size={}", batch_size), total_hashes, start.elapsed())
            .with_metadata("simd", SimdLevel::detect().name())
    }

    /// Benchmark multi-threaded mining simulation
    pub fn bench_multi_thread(&self, thread_counts: &[usize]) -> Vec<BenchmarkResult> {
        use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;

        let mut results = Vec::new();

        for &num_threads in thread_counts {
            let total_hashes = Arc::new(AtomicU64::new(0));
            let running = Arc::new(AtomicBool::new(true));
            let batch_size = 65536u32;

            // Spawn worker threads
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

            // Let it run
            let start = Instant::now();
            thread::sleep(self.bench_duration);
            running.store(false, Ordering::SeqCst);

            // Join all threads
            for h in handles {
                let _ = h.join();
            }

            let hashes = total_hashes.load(Ordering::Relaxed);
            let result = BenchmarkResult::new(
                "multi_thread_mining",
                &format!("threads={}", num_threads),
                hashes,
                start.elapsed(),
            )
            .with_metadata("threads", &num_threads.to_string())
            .with_metadata("simd", SimdLevel::detect().name());

            results.push(result);
        }

        results
    }

    /// Run a complete benchmark suite
    pub fn run_full_suite(&self) -> BenchmarkSuite {
        let mut suite = BenchmarkSuite::new("Full Mining Benchmark Suite");

        // SHA256d generic
        suite.add_result(self.bench_sha256d_generic());

        // Hasher throughput
        suite.add_result(self.bench_hasher_throughput());

        // Batch sizes
        let batch_sizes = [1024, 4096, 16384, 65536, 262144, 1048576];
        for result in self.bench_mining_batch_sizes(&batch_sizes) {
            suite.add_result(result);
        }

        // Multi-threaded (1, 2, 4, 8, max threads)
        let cpu_count = num_cpus::get();
        let mut thread_counts = vec![1, 2, 4];
        if cpu_count >= 8 {
            thread_counts.push(8);
        }
        if cpu_count > 8 {
            thread_counts.push(cpu_count);
        }
        for result in self.bench_multi_thread(&thread_counts) {
            suite.add_result(result);
        }

        suite
    }

    /// Run quick benchmark (shorter duration)
    pub fn run_quick_suite(&self) -> BenchmarkSuite {
        let quick_profiler = Profiler::new()
            .with_warmup(Duration::from_millis(500))
            .with_duration(Duration::from_secs(2));

        let mut suite = BenchmarkSuite::new("Quick Mining Benchmark");

        // Basic tests
        suite.add_result(quick_profiler.bench_sha256d_generic());
        suite.add_result(quick_profiler.bench_hasher_throughput());

        // Limited thread counts
        let thread_counts = vec![1, num_cpus::get()];
        for result in quick_profiler.bench_multi_thread(&thread_counts) {
            suite.add_result(result);
        }

        suite
    }
}

/// Format hashrate for display
pub fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e15 {
        format!("{:.2} PH/s", hashrate / 1e15)
    } else if hashrate >= 1e12 {
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

/// Print benchmark results in a nice table format
pub fn print_results(suite: &BenchmarkSuite) {
    println!("\n{}", "=".repeat(70));
    println!("Benchmark Suite: {}", suite.name);
    println!("Timestamp: {}", suite.timestamp);
    println!("System: {} {} ({} cores, {})",
        suite.system_info.os,
        suite.system_info.arch,
        suite.system_info.cpu_count,
        suite.system_info.simd_level
    );
    println!("{}", "=".repeat(70));
    println!("{:<30} {:<20} {:>15}", "Benchmark", "Config", "Hashrate");
    println!("{}", "-".repeat(70));

    for result in &suite.results {
        println!(
            "{:<30} {:<20} {:>15}",
            result.name,
            result.config,
            format_hashrate(result.hashrate)
        );
    }

    println!("{}", "=".repeat(70));

    // Show best result
    if let Some(best) = suite.sorted_by_hashrate().first() {
        println!("Best: {} ({}) - {}", best.name, best.config, format_hashrate(best.hashrate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result_new() {
        let result = BenchmarkResult::new("test", "config1", 1000, Duration::from_secs(1));
        assert_eq!(result.name, "test");
        assert_eq!(result.config, "config1");
        assert_eq!(result.hashes, 1000);
        assert!((result.hashrate - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_benchmark_result_with_metadata() {
        let result = BenchmarkResult::new("test", "config", 100, Duration::from_millis(100))
            .with_metadata("key1", "value1")
            .with_metadata("key2", "value2");

        assert_eq!(result.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(result.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_benchmark_result_zero_duration() {
        let result = BenchmarkResult::new("test", "config", 1000, Duration::ZERO);
        assert_eq!(result.hashrate, 0.0);
    }

    #[test]
    fn test_benchmark_suite_new() {
        let suite = BenchmarkSuite::new("test suite");
        assert_eq!(suite.name, "test suite");
        assert!(suite.results.is_empty());
        assert!(!suite.timestamp.is_empty());
    }

    #[test]
    fn test_benchmark_suite_add_result() {
        let mut suite = BenchmarkSuite::new("test");
        suite.add_result(BenchmarkResult::new("r1", "c1", 100, Duration::from_secs(1)));
        suite.add_result(BenchmarkResult::new("r2", "c2", 200, Duration::from_secs(1)));

        assert_eq!(suite.results.len(), 2);
    }

    #[test]
    fn test_benchmark_suite_to_csv() {
        let mut suite = BenchmarkSuite::new("test");
        suite.add_result(BenchmarkResult::new("bench1", "config1", 1000, Duration::from_secs(1)));

        let csv = suite.to_csv();
        assert!(csv.contains("name,config,hashes,duration_secs,hashrate"));
        assert!(csv.contains("bench1,config1,1000"));
    }

    #[test]
    fn test_benchmark_suite_to_json() {
        let mut suite = BenchmarkSuite::new("test");
        suite.add_result(BenchmarkResult::new("bench1", "config1", 1000, Duration::from_secs(1)));

        let json = suite.to_json().unwrap();
        assert!(json.contains("\"name\": \"test\""));
        assert!(json.contains("\"bench1\""));
    }

    #[test]
    fn test_benchmark_suite_sorted_by_hashrate() {
        let mut suite = BenchmarkSuite::new("test");
        suite.add_result(BenchmarkResult::new("slow", "c1", 100, Duration::from_secs(1)));
        suite.add_result(BenchmarkResult::new("fast", "c2", 1000, Duration::from_secs(1)));
        suite.add_result(BenchmarkResult::new("medium", "c3", 500, Duration::from_secs(1)));

        let sorted = suite.sorted_by_hashrate();
        assert_eq!(sorted[0].name, "fast");
        assert_eq!(sorted[1].name, "medium");
        assert_eq!(sorted[2].name, "slow");
    }

    #[test]
    fn test_system_info_detect() {
        let info = SystemInfo::detect();
        assert!(info.cpu_count > 0);
        assert!(!info.simd_level.is_empty());
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }

    #[test]
    fn test_profiler_new() {
        let profiler = Profiler::new();
        assert_eq!(profiler.warmup_duration, Duration::from_secs(1));
        assert_eq!(profiler.bench_duration, Duration::from_secs(5));
    }

    #[test]
    fn test_profiler_with_duration() {
        let profiler = Profiler::new()
            .with_warmup(Duration::from_millis(100))
            .with_duration(Duration::from_millis(500));

        assert_eq!(profiler.warmup_duration, Duration::from_millis(100));
        assert_eq!(profiler.bench_duration, Duration::from_millis(500));
    }

    #[test]
    fn test_format_hashrate_units() {
        assert_eq!(format_hashrate(500.0), "500.00 H/s");
        assert_eq!(format_hashrate(1500.0), "1.50 KH/s");
        assert_eq!(format_hashrate(1_500_000.0), "1.50 MH/s");
        assert_eq!(format_hashrate(1_500_000_000.0), "1.50 GH/s");
        assert_eq!(format_hashrate(1_500_000_000_000.0), "1.50 TH/s");
        assert_eq!(format_hashrate(1_500_000_000_000_000.0), "1.50 PH/s");
    }

    // Quick functional tests (short duration)
    #[test]
    fn test_bench_sha256d_generic_runs() {
        let profiler = Profiler::new()
            .with_warmup(Duration::from_millis(10))
            .with_duration(Duration::from_millis(100));

        let result = profiler.bench_sha256d_generic();
        assert!(result.hashes > 0);
        assert!(result.hashrate > 0.0);
    }

    #[test]
    fn test_bench_hasher_throughput_runs() {
        let profiler = Profiler::new()
            .with_warmup(Duration::from_millis(10))
            .with_duration(Duration::from_millis(100));

        let result = profiler.bench_hasher_throughput();
        assert!(result.hashes > 0);
        assert!(result.hashrate > 0.0);
    }

    #[test]
    fn test_bench_mining_batch_sizes_runs() {
        let profiler = Profiler::new()
            .with_warmup(Duration::from_millis(10))
            .with_duration(Duration::from_millis(50));

        let results = profiler.bench_mining_batch_sizes(&[1024, 4096]);
        assert_eq!(results.len(), 2);
        for result in results {
            assert!(result.hashes > 0);
        }
    }

    #[test]
    fn test_bench_multi_thread_runs() {
        let profiler = Profiler::new()
            .with_warmup(Duration::from_millis(10))
            .with_duration(Duration::from_millis(100));

        let results = profiler.bench_multi_thread(&[1, 2]);
        assert_eq!(results.len(), 2);
        for result in &results {
            assert!(result.hashes > 0);
        }

        // Multi-threaded should generally be faster
        // (though not guaranteed on single-core systems)
        assert!(results[1].hashrate >= results[0].hashrate * 0.5);
    }
}
