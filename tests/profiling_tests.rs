//! Unit tests for the profiling module
//!
//! Performance benchmarks have been moved to benches/mining_benchmarks.rs
//! Run benchmarks with: cargo bench

use std::time::Duration;
use btcminer::profiling::{Profiler, BenchmarkSuite, BenchmarkResult};

#[test]
fn test_profiling_module_loads() {
    let profiler = Profiler::new();
    assert_eq!(profiler.warmup_duration, Duration::from_secs(1));
    assert_eq!(profiler.bench_duration, Duration::from_secs(5));
}

#[test]
fn test_benchmark_result_creation() {
    let result = BenchmarkResult::new("test", "config", 1000000, Duration::from_secs(1));
    assert_eq!(result.name, "test");
    assert_eq!(result.config, "config");
    assert_eq!(result.hashes, 1000000);
    assert!((result.hashrate - 1000000.0).abs() < 0.1);
}

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

#[test]
fn test_benchmark_result_with_metadata() {
    let result = BenchmarkResult::new("test", "config", 1000, Duration::from_millis(100))
        .with_metadata("threads", "4")
        .with_metadata("batch_size", "65536");

    assert_eq!(result.metadata.get("threads"), Some(&"4".to_string()));
    assert_eq!(result.metadata.get("batch_size"), Some(&"65536".to_string()));
}

#[test]
fn test_suite_sorted_by_hashrate() {
    let mut suite = BenchmarkSuite::new("test");
    suite.add_result(BenchmarkResult::new("slow", "cfg", 100, Duration::from_secs(1)));
    suite.add_result(BenchmarkResult::new("fast", "cfg", 1000, Duration::from_secs(1)));
    suite.add_result(BenchmarkResult::new("medium", "cfg", 500, Duration::from_secs(1)));

    let sorted = suite.sorted_by_hashrate();
    assert_eq!(sorted[0].name, "fast");
    assert_eq!(sorted[1].name, "medium");
    assert_eq!(sorted[2].name, "slow");
}

#[test]
fn test_suite_json_export() {
    let mut suite = BenchmarkSuite::new("json_test");
    suite.add_result(BenchmarkResult::new("bench", "cfg", 1000, Duration::from_secs(1)));

    let json = suite.to_json().expect("Failed to serialize JSON");
    // serde_json::to_string_pretty uses spaces after colons
    assert!(json.contains("\"name\": \"json_test\""));
    assert!(json.contains("\"results\""));
}
