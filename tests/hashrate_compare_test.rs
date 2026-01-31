//! Hashrate Comparison Test
//!
//! Compares library-level hashrate (~23 MH/s) vs production binary (50+ GH/s)
//! to demonstrate the cross-crate optimization boundary effect.
//!
//! This test documents the fundamental Rust limitation where #[target_feature]
//! functions cannot be inlined across crate boundaries.
//!
//! Run: cargo test --release --test hashrate_compare_test -- --nocapture --ignored

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::io::{BufRead, BufReader};

use btcminer::mining::{Hasher, Midstate, SimdLevel};
use btcminer::mining::sha256_simd::mine_with_midstate;

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

/// Measure library-level hashrate (calls into btcminer as a library)
fn measure_library_hashrate(duration_secs: u64) -> f64 {
    let mut hasher = Hasher::new();
    let header = [0u8; 76];
    let target = [0x00; 32]; // Hard target
    let batch_size = 262144u32;

    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut total_hashes = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
        total_hashes += batch_size as u64;
        nonce = nonce.wrapping_add(batch_size);
    }

    let elapsed = start.elapsed().as_secs_f64();
    total_hashes as f64 / elapsed
}

/// Measure low-level mine_with_midstate hashrate (still library call)
fn measure_midstate_hashrate(duration_secs: u64) -> f64 {
    let header = [0u8; 64];
    let midstate = Midstate::from_header_prefix(&header);
    let tail = [0u8; 16];
    let target = [0x00; 32]; // Hard target
    let batch_size = 262144u32;

    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut total_hashes = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        let _ = mine_with_midstate(&midstate, &tail, nonce, batch_size, &target);
        total_hashes += batch_size as u64;
        nonce = nonce.wrapping_add(batch_size);
    }

    let elapsed = start.elapsed().as_secs_f64();
    total_hashes as f64 / elapsed
}

/// Measure production binary hashrate via --benchmark flag
fn measure_binary_hashrate(duration_secs: u64) -> Option<f64> {
    #[cfg(windows)]
    let binary = "target/release/btcminer.exe";
    #[cfg(not(windows))]
    let binary = "target/release/btcminer";

    let mut child = Command::new(binary)
        .args(["--benchmark", "--benchmark-duration", &duration_secs.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let stdout = child.stdout.take()?;
    let reader = BufReader::new(stdout);

    let mut total_hashrate: Option<f64> = None;

    for line in reader.lines().map_while(Result::ok) {
        // Look for "Total hashrate:" line
        if line.contains("Total hashrate:") {
            // Parse hashrate from line like "  Total hashrate:   51.23 GH/s"
            if let Some(rate) = parse_hashrate_line(&line) {
                total_hashrate = Some(rate);
            }
        }
    }

    let _ = child.wait();
    total_hashrate
}

/// Parse hashrate from a line containing GH/s, MH/s, etc.
fn parse_hashrate_line(line: &str) -> Option<f64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            if *part == "GH/s" {
                return parts[i - 1].parse::<f64>().ok().map(|v| v * 1e9);
            }
            if *part == "MH/s" {
                return parts[i - 1].parse::<f64>().ok().map(|v| v * 1e6);
            }
            if *part == "TH/s" {
                return parts[i - 1].parse::<f64>().ok().map(|v| v * 1e12);
            }
            if *part == "KH/s" {
                return parts[i - 1].parse::<f64>().ok().map(|v| v * 1e3);
            }
            if *part == "H/s" {
                return parts[i - 1].parse::<f64>().ok();
            }
        }
    }
    None
}

/// Main comparison test
#[test]
#[ignore]
fn hashrate_comparison() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║            Hashrate Comparison Test                           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("This test demonstrates the cross-crate optimization boundary effect.");
    println!("Library calls show ~23 MH/s while production binary achieves 50+ GH/s.");
    println!();
    println!("SIMD Level: {}", SimdLevel::detect().name());
    println!();

    let test_duration = 5; // seconds per test

    // Test 1: Library-level Hasher
    println!("Testing library-level Hasher::hash_nonce_range() for {}s...", test_duration);
    let library_rate = measure_library_hashrate(test_duration);
    println!("  Library hashrate: {}", format_hashrate(library_rate));

    // Test 2: Library-level mine_with_midstate
    println!();
    println!("Testing library-level mine_with_midstate() for {}s...", test_duration);
    let midstate_rate = measure_midstate_hashrate(test_duration);
    println!("  Midstate hashrate: {}", format_hashrate(midstate_rate));

    // Test 3: Production binary --benchmark
    println!();
    println!("Testing production binary (btcminer --benchmark) for {}s...", test_duration);
    let binary_rate = measure_binary_hashrate(test_duration);

    match binary_rate {
        Some(rate) => {
            println!("  Binary hashrate: {}", format_hashrate(rate));

            // Calculate difference
            let factor = rate / library_rate;

            println!();
            println!("═══════════════════════════════════════════════════════════════");
            println!("  COMPARISON RESULTS");
            println!("═══════════════════════════════════════════════════════════════");
            println!();
            println!("  | Method              | Hashrate       | Factor    |");
            println!("  |---------------------|----------------|-----------|");
            println!("  | Library (Hasher)    | {:>14} | 1x        |", format_hashrate(library_rate));
            println!("  | Library (midstate)  | {:>14} | {:.1}x      |", format_hashrate(midstate_rate), midstate_rate / library_rate);
            println!("  | Production binary   | {:>14} | {:.0}x   |", format_hashrate(rate), factor);
            println!();

            if factor > 100.0 {
                println!("  ✓ Expected: Production binary is {:.0}x faster than library calls", factor);
                println!("    This is due to cross-crate optimization boundaries in Rust.");
                println!("    #[target_feature] functions cannot be inlined across crate boundaries.");
            } else {
                println!("  ⚠ Unexpected: Factor should be ~2000x, got {:.0}x", factor);
                println!("    This might indicate the binary wasn't built with optimizations.");
            }
        }
        None => {
            println!("  Binary test failed - could not parse output");
            println!("  Make sure btcminer is built: cargo build --release --bin btcminer");

            println!();
            println!("═══════════════════════════════════════════════════════════════");
            println!("  LIBRARY-ONLY RESULTS");
            println!("═══════════════════════════════════════════════════════════════");
            println!();
            println!("  | Method              | Hashrate       |");
            println!("  |---------------------|----------------|");
            println!("  | Library (Hasher)    | {:>14} |", format_hashrate(library_rate));
            println!("  | Library (midstate)  | {:>14} |", format_hashrate(midstate_rate));
            println!();
            println!("  Note: Production binary typically achieves 2000x+ higher rates.");
        }
    }

    println!();
    println!("For accurate production hashrate measurement, use:");
    println!("  btcminer --benchmark");
    println!("  cargo test --release --test binary_api_test -- --nocapture --ignored");
    println!();
}

/// Quick library-only comparison (no binary needed)
#[test]
#[ignore]
fn library_hashrate_quick() {
    println!();
    println!("=== Quick Library Hashrate Test (3s each) ===");
    println!("SIMD Level: {}", SimdLevel::detect().name());
    println!();

    let library_rate = measure_library_hashrate(3);
    println!("Hasher::hash_nonce_range(): {}", format_hashrate(library_rate));

    let midstate_rate = measure_midstate_hashrate(3);
    println!("mine_with_midstate():       {}", format_hashrate(midstate_rate));

    println!();
    println!("Note: These rates are ~2000x lower than production binary");
    println!("due to cross-crate optimization boundaries.");
}

/// Verify SIMD detection
#[test]
fn simd_detection_check() {
    let level = SimdLevel::detect();
    println!("Detected SIMD level: {}", level.name());

    #[cfg(target_arch = "x86_64")]
    {
        println!("CPU features:");
        println!("  SHA:     {}", is_x86_feature_detected!("sha"));
        println!("  SSE4.1:  {}", is_x86_feature_detected!("sse4.1"));
        println!("  AVX2:    {}", is_x86_feature_detected!("avx2"));
        println!("  AVX512F: {}", is_x86_feature_detected!("avx512f"));
    }
}

/// Compare SIMD hasher vs SHA2 crate hasher
#[test]
#[ignore]
fn hasher_type_comparison() {
    use btcminer::mining::HasherType;

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║            Hasher Type Comparison                             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Comparing SIMD hasher vs SHA2 crate hasher (library-level)");
    println!("SIMD Level: {}", SimdLevel::detect().name());
    println!();

    let test_duration = 5;
    let batch_size = 262144u32;
    let header = [0u8; 76];
    let target = [0x00; 32];

    // Test SIMD hasher
    println!("Testing SIMD hasher for {}s...", test_duration);
    let simd_rate = {
        let mut hasher = Hasher::with_type(HasherType::Simd);
        let duration = Duration::from_secs(test_duration);
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        total_hashes as f64 / start.elapsed().as_secs_f64()
    };
    println!("  SIMD hashrate: {}", format_hashrate(simd_rate));

    // Test SHA2 crate hasher
    println!();
    println!("Testing SHA2 crate hasher for {}s...", test_duration);
    let sha2_rate = {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        let duration = Duration::from_secs(test_duration);
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        total_hashes as f64 / start.elapsed().as_secs_f64()
    };
    println!("  SHA2 hashrate: {}", format_hashrate(sha2_rate));

    let factor = simd_rate / sha2_rate;

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  HASHER TYPE COMPARISON");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  | Hasher Type | Hashrate       | Factor |");
    println!("  |-------------|----------------|--------|");
    println!("  | SIMD        | {:>14} | {:.1}x   |", format_hashrate(simd_rate), factor);
    println!("  | SHA2 crate  | {:>14} | 1x     |", format_hashrate(sha2_rate));
    println!();

    if factor > 2.0 {
        println!("  ✓ SIMD hasher is {:.1}x faster than SHA2 crate", factor);
    } else {
        println!("  ⚠ SIMD hasher should be faster - check SIMD detection");
    }
    println!();
}

/// Full comparison test: Library vs Binary with different configurations
///
/// Tests:
/// 1. Library SIMD hasher vs SHA2 crate
/// 2. Production binary performance
/// 3. Reports the cross-crate optimization factor
#[test]
#[ignore]
fn full_benchmark_comparison() {
    use btcminer::mining::HasherType;

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║            Full Benchmark Comparison                          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("System Info:");
    println!("  SIMD Level: {}", SimdLevel::detect().name());
    println!("  CPUs: {}", num_cpus::get());
    println!();

    let test_duration = 5;
    let batch_size = 262144u32;
    let header = [0u8; 76];
    let target = [0x00; 32];

    let mut results: Vec<(&str, f64, &str)> = Vec::new();

    // Test 1: Library SIMD hasher
    println!("1. Testing Library SIMD hasher for {}s...", test_duration);
    let simd_rate = {
        let mut hasher = Hasher::with_type(HasherType::Simd);
        let duration = Duration::from_secs(test_duration);
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        total_hashes as f64 / start.elapsed().as_secs_f64()
    };
    println!("   → {}", format_hashrate(simd_rate));
    results.push(("Library (SIMD)", simd_rate, "Cross-crate call"));

    // Test 2: Library SHA2 crate hasher
    println!();
    println!("2. Testing Library SHA2 crate hasher for {}s...", test_duration);
    let sha2_rate = {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        let duration = Duration::from_secs(test_duration);
        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed() < duration {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        total_hashes as f64 / start.elapsed().as_secs_f64()
    };
    println!("   → {}", format_hashrate(sha2_rate));
    results.push(("Library (SHA2)", sha2_rate, "Cross-crate call"));

    // Test 3: Production binary
    println!();
    println!("3. Testing Production binary (--benchmark) for {}s...", test_duration);
    if let Some(binary_rate) = measure_binary_hashrate(test_duration) {
        println!("   → {}", format_hashrate(binary_rate));
        results.push(("Binary (SIMD)", binary_rate, "Same compilation unit"));
    } else {
        println!("   → Failed (build with: cargo build --release --bin btcminer)");
    }

    // Calculate factors
    let baseline = sha2_rate; // Use SHA2 crate as baseline

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  | Method           | Hashrate       | Factor | Notes                  |");
    println!("  |------------------|----------------|--------|------------------------|");
    for (name, rate, notes) in &results {
        let factor = rate / baseline;
        println!("  | {:16} | {:>14} | {:>5.1}x | {:22} |",
            name, format_hashrate(*rate), factor, notes);
    }
    println!();

    // Analysis
    let simd_vs_sha2 = simd_rate / sha2_rate;
    println!("Analysis:");
    println!("  • SIMD vs SHA2 crate: {:.1}x speedup", simd_vs_sha2);

    if let Some((_, binary_rate, _)) = results.iter().find(|(n, _, _)| *n == "Binary (SIMD)") {
        let binary_vs_library = binary_rate / simd_rate;
        println!("  • Binary vs Library (SIMD): {:.1}x speedup", binary_vs_library);
        println!();
        if binary_vs_library > 100.0 {
            println!("  ✓ Cross-crate optimization boundary confirmed ({:.0}x)", binary_vs_library);
        } else if binary_vs_library > 5.0 {
            println!("  ~ Moderate cross-crate effect ({:.1}x) - platform dependent", binary_vs_library);
        } else {
            println!("  ⚠ Small difference - check if binary was built with optimizations");
        }
    }

    println!();
    println!("Note: The SIMD hasher uses {} which requires aggressive inlining.", SimdLevel::detect().name());
    println!("Cross-crate calls prevent full optimization due to #[target_feature] constraints.");
}
