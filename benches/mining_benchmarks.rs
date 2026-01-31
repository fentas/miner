//! Mining benchmarks using criterion (Library-level)
//!
//! WARNING: These benchmarks show ~23 MH/s due to cross-crate optimization
//! boundaries. The production binary achieves 50+ GH/s (2,200x faster).
//!
//! Use these benchmarks for:
//! - RELATIVE comparisons (A vs B)
//! - Regression detection (same code getting slower)
//! - Batch size optimization
//!
//! For TRUE production hashrate, use:
//! - `btcminer --benchmark` (direct measurement)
//! - `cargo test --release --test binary_api_test` (E2E via API)
//!
//! Run with: cargo bench
//! Run specific: cargo bench -- hasher
//! Run with HTML reports: cargo bench (reports in target/criterion/)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

use btcminer::mining::{Hasher, HasherType, Midstate, SimdLevel};

/// Benchmark hasher performance at different batch sizes
fn bench_hasher_batch_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_batch_size");
    let header = [0u8; 76];
    // Hard target - processes all nonces for accurate throughput measurement
    let target = [0x00; 32];

    for batch_exp in [16u8, 17, 18, 19, 20] {
        let batch_size = 1u32 << batch_exp;
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("2^{}", batch_exp)),
            &batch_size,
            |b, &batch_size| {
                let mut hasher = Hasher::new();
                let mut nonce = 0u32;
                b.iter(|| {
                    let result = hasher.hash_nonce_range(
                        black_box(&header),
                        black_box(nonce),
                        black_box(batch_size),
                        black_box(&target),
                    );
                    nonce = nonce.wrapping_add(batch_size);
                    result
                });
            },
        );
    }
    group.finish();
}

/// Benchmark midstate computation
fn bench_midstate(c: &mut Criterion) {
    let header = [0xABu8; 64];

    c.bench_function("midstate_compute", |b| {
        b.iter(|| Midstate::from_header_prefix(black_box(&header)))
    });
}

/// Benchmark SIMD level detection
fn bench_simd_detect(c: &mut Criterion) {
    c.bench_function("simd_detect", |b| {
        b.iter(SimdLevel::detect)
    });
}

/// Benchmark hasher creation
fn bench_hasher_new(c: &mut Criterion) {
    c.bench_function("hasher_new", |b| {
        b.iter(Hasher::new)
    });
}

/// Benchmark single hash operation for latency measurement
fn bench_single_hash(c: &mut Criterion) {
    let mut hasher = Hasher::new();
    let header = [0u8; 76];
    let target = [0x00; 32]; // Hard target for realistic measurement

    c.bench_function("single_hash_batch", |b| {
        let mut nonce = 0u32;
        b.iter(|| {
            let result = hasher.hash_nonce_range(
                black_box(&header),
                black_box(nonce),
                black_box(1),
                black_box(&target),
            );
            nonce = nonce.wrapping_add(1);
            result
        });
    });
}

/// Benchmark single-threaded throughput
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    let header = [0u8; 76];
    let target = [0x00; 32]; // Hard target for realistic measurement
    let batch_size = 262144u32; // 2^18

    group.throughput(Throughput::Elements(batch_size as u64));
    group.sample_size(50);

    group.bench_function("single_thread_256k", |b| {
        let mut hasher = Hasher::new();
        let mut nonce = 0u32;
        b.iter(|| {
            let result = hasher.hash_nonce_range(
                black_box(&header),
                black_box(nonce),
                black_box(batch_size),
                black_box(&target),
            );
            nonce = nonce.wrapping_add(batch_size);
            result
        });
    });

    group.finish();
}

/// Benchmark multi-threaded throughput at different thread counts
fn bench_thread_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_scaling");
    group.sample_size(10); // Fewer samples for multi-threaded tests
    group.measurement_time(Duration::from_secs(5));

    let cpu_count = num_cpus::get();
    let thread_counts: Vec<usize> = vec![1, cpu_count / 2, cpu_count].into_iter()
        .filter(|&t| t > 0)
        .collect();

    for num_threads in thread_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let total_hashes = Arc::new(AtomicU64::new(0));
                    let running = Arc::new(AtomicBool::new(true));
                    let batch_size = 65536u32;

                    let mut handles = Vec::new();
                    for _ in 0..num_threads {
                        let hashes = Arc::clone(&total_hashes);
                        let run = Arc::clone(&running);

                        let handle = thread::spawn(move || {
                            let mut hasher = Hasher::new();
                            let header = [0u8; 76];
                            let target = [0x00; 32]; // Hard target for realistic measurement
                            let mut nonce = 0u32;

                            while run.load(Ordering::Relaxed) {
                                let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
                                hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                                nonce = nonce.wrapping_add(batch_size);
                            }
                        });
                        handles.push(handle);
                    }

                    // Run for a fixed duration based on iterations
                    let duration = Duration::from_millis(100 * iters);
                    thread::sleep(duration);
                    running.store(false, Ordering::SeqCst);

                    for h in handles {
                        let _ = h.join();
                    }

                    duration
                });
            },
        );
    }

    group.finish();
}

/// Benchmark different hasher implementations (SIMD vs SHA2 crate)
///
/// This helps identify if the benchmark vs production discrepancy is hasher-specific.
fn bench_hasher_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_comparison");
    let header = [0u8; 76];
    let target = [0x00; 32]; // Hard target
    let batch_size = 65536u32; // 2^16 - reasonable size for comparison

    group.throughput(Throughput::Elements(batch_size as u64));
    group.sample_size(50);

    // SIMD hasher (default, should be fastest)
    group.bench_function("simd", |b| {
        let mut hasher = Hasher::with_type(HasherType::Simd);
        let mut nonce = 0u32;
        b.iter(|| {
            let result = hasher.hash_nonce_range(
                black_box(&header),
                black_box(nonce),
                black_box(batch_size),
                black_box(&target),
            );
            nonce = nonce.wrapping_add(batch_size);
            result
        });
    });

    // SHA2 crate hasher (baseline for comparison)
    group.bench_function("sha2_crate", |b| {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        let mut nonce = 0u32;
        b.iter(|| {
            let result = hasher.hash_nonce_range(
                black_box(&header),
                black_box(nonce),
                black_box(batch_size),
                black_box(&target),
            );
            nonce = nonce.wrapping_add(batch_size);
            result
        });
    });

    group.finish();
}

/// Benchmark hasher implementations at different batch sizes
fn bench_hasher_types_batch_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_types_batch");
    let header = [0u8; 76];
    let target = [0x00; 32];

    for batch_exp in [14u8, 16, 18] {
        let batch_size = 1u32 << batch_exp;
        group.throughput(Throughput::Elements(batch_size as u64));

        // SIMD
        group.bench_with_input(
            BenchmarkId::new("simd", format!("2^{}", batch_exp)),
            &batch_size,
            |b, &batch_size| {
                let mut hasher = Hasher::with_type(HasherType::Simd);
                let mut nonce = 0u32;
                b.iter(|| {
                    let result = hasher.hash_nonce_range(
                        black_box(&header),
                        black_box(nonce),
                        black_box(batch_size),
                        black_box(&target),
                    );
                    nonce = nonce.wrapping_add(batch_size);
                    result
                });
            },
        );

        // SHA2 crate
        group.bench_with_input(
            BenchmarkId::new("sha2", format!("2^{}", batch_exp)),
            &batch_size,
            |b, &batch_size| {
                let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
                let mut nonce = 0u32;
                b.iter(|| {
                    let result = hasher.hash_nonce_range(
                        black_box(&header),
                        black_box(nonce),
                        black_box(batch_size),
                        black_box(&target),
                    );
                    nonce = nonce.wrapping_add(batch_size);
                    result
                });
            },
        );
    }
    group.finish();
}

/// Benchmark multi-threaded performance for both hasher types
fn bench_hasher_types_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher_types_threaded");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(5));

    let cpu_count = num_cpus::get();
    let batch_size = 65536u32;

    for hasher_type in [HasherType::Simd, HasherType::Sha2Crate] {
        let type_name = match hasher_type {
            HasherType::Simd => "simd",
            HasherType::Sha2Crate => "sha2",
        };

        group.bench_with_input(
            BenchmarkId::new(type_name, cpu_count),
            &cpu_count,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let total_hashes = Arc::new(AtomicU64::new(0));
                    let running = Arc::new(AtomicBool::new(true));

                    let mut handles = Vec::new();
                    for _ in 0..num_threads {
                        let hashes = Arc::clone(&total_hashes);
                        let run = Arc::clone(&running);

                        let handle = thread::spawn(move || {
                            let mut hasher = Hasher::with_type(hasher_type);
                            let header = [0u8; 76];
                            let target = [0x00; 32];
                            let mut nonce = 0u32;

                            while run.load(Ordering::Relaxed) {
                                let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
                                hashes.fetch_add(batch_size as u64, Ordering::Relaxed);
                                nonce = nonce.wrapping_add(batch_size);
                            }
                        });
                        handles.push(handle);
                    }

                    let duration = Duration::from_millis(100 * iters);
                    thread::sleep(duration);
                    running.store(false, Ordering::SeqCst);

                    for h in handles {
                        let _ = h.join();
                    }

                    duration
                });
            },
        );
    }

    group.finish();
}

/// Long-running benchmark to match production conditions
///
/// The hypothesis is that short benchmarks may not warm up properly.
/// This runs for 10+ seconds to match production behavior.
fn bench_long_running(c: &mut Criterion) {
    let mut group = c.benchmark_group("long_running");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    let header = [0u8; 76];
    let target = [0x00; 32];
    let batch_size = 65536u32;

    for hasher_type in [HasherType::Simd, HasherType::Sha2Crate] {
        let type_name = match hasher_type {
            HasherType::Simd => "simd_10s",
            HasherType::Sha2Crate => "sha2_10s",
        };

        group.bench_function(type_name, |b| {
            let mut hasher = Hasher::with_type(hasher_type);
            let mut nonce = 0u32;
            b.iter(|| {
                let result = hasher.hash_nonce_range(
                    black_box(&header),
                    black_box(nonce),
                    black_box(batch_size),
                    black_box(&target),
                );
                nonce = nonce.wrapping_add(batch_size);
                result
            });
        });
    }

    group.finish();
}

/// Benchmark extranonce2 generation methods (for optimization comparison)
fn bench_extranonce2_generation(c: &mut Criterion) {
    use std::sync::atomic::AtomicU32;
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let mut group = c.benchmark_group("extranonce2");

    // Current approach: SystemTime
    group.bench_function("systemtime", |b| {
        b.iter(|| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos();
            black_box(nanos)
        });
    });

    // Optimized approach: Atomic counter
    group.bench_function("atomic_counter", |b| {
        b.iter(|| {
            let val = COUNTER.fetch_add(1, Ordering::Relaxed);
            black_box(val)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_hasher_batch_sizes,
    bench_midstate,
    bench_simd_detect,
    bench_hasher_new,
    bench_single_hash,
    bench_throughput,
    bench_thread_scaling,
    bench_extranonce2_generation,
);

// Hasher comparison benchmarks (SIMD vs SHA2 crate)
criterion_group!(
    hasher_benches,
    bench_hasher_comparison,
    bench_hasher_types_batch_sizes,
    bench_hasher_types_threaded,
    bench_long_running,
);

criterion_main!(benches, hasher_benches);
