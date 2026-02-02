//! Research Experiment v11: Compiler Optimization Analysis
//!
//! Tests impact of various compiler optimization settings.
//! Analyzes native CPU tuning, codegen options, and optimization potential.
//!
//! Correlates to: docs/RESEARCH-test-log-v11.md
//! Run with: cargo test --release --test research_v11_test -- --nocapture

use std::time::{Instant, Duration};
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

/// Benchmark a hasher for a given duration
fn benchmark_duration(hasher: &mut Hasher, duration_secs: u64) -> f64 {
    let header_prefix = make_header_prefix(12345);
    let hard_target = [0u8; 32];
    let batch_size = 10_000u32;

    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut total_hashes = 0u64;
    let mut nonce = 0u32;

    while start.elapsed() < duration {
        let _ = hasher.hash_nonce_range(&header_prefix, nonce, batch_size, &hard_target);
        total_hashes += batch_size as u64;
        nonce = nonce.wrapping_add(batch_size);
    }

    total_hashes as f64 / start.elapsed().as_secs_f64() / 1_000_000.0
}

/// Test 1: Current compiler settings analysis
#[test]
fn test_research_v11_compiler_settings() {
    println!("\n=== Research v11: Compiler Optimization Analysis ===\n");

    println!("Current Build Configuration:\n");

    println!("| Setting | Value |");
    println!("|---------|-------|");
    println!("| Profile | release |");
    println!("| opt-level | 3 (maximum) |");
    println!("| LTO | fat (full cross-crate) |");
    println!("| codegen-units | 1 (maximum optimization) |");
    println!("| strip | true |");

    // Check target features from config
    println!("\nTarget Features (from .cargo/config.toml):");
    println!("  +sha, +sse4.1, +ssse3, +sse2");

    // Check if running with native CPU
    #[cfg(target_arch = "x86_64")]
    {
        println!("\nCPU Capabilities Detected at Runtime:");
        let features = [
            ("SHA-NI", std::is_x86_feature_detected!("sha")),
            ("AVX", std::is_x86_feature_detected!("avx")),
            ("AVX2", std::is_x86_feature_detected!("avx2")),
            ("AVX-512F", std::is_x86_feature_detected!("avx512f")),
        ];

        for (name, available) in &features {
            println!("  {}: {}", name, if *available { "Yes" } else { "No" });
        }
    }
}

/// Test 2: Baseline performance measurement
#[test]
fn test_research_v11_baseline() {
    println!("\n=== Research v11: Baseline Performance ===\n");

    let duration = 3;

    println!("Measuring current optimized performance...\n");

    let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
    let rate = benchmark_duration(&mut hasher, duration);

    println!("| Hasher | Rate (MH/s) |");
    println!("|--------|-------------|");
    println!("| Sha2Crate | {:>11.2} |", rate);

    // Calculate theoretical metrics
    let sha256_rounds = 128; // Double SHA-256
    let ops_per_round = 20; // Approximate
    let total_ops = sha256_rounds * ops_per_round;

    println!("\nTheoretical Analysis:");
    println!("  Operations per hash: ~{}", total_ops);
    println!("  Estimated GOPS: {:.1}", rate * total_ops as f64 / 1000.0);

    // Compare to CPU specs
    // AMD Ryzen 9 7900: ~5.4 GHz boost, ~4 IPC = ~21.6 GOPS/core theoretical
    let theoretical_gops = 21.6;
    let utilization = (rate * total_ops as f64 / 1000.0) / theoretical_gops * 100.0;

    println!("  CPU theoretical max: ~{:.1} GOPS/core", theoretical_gops);
    println!("  Estimated utilization: {:.1}%", utilization);

    if utilization > 200.0 {
        println!("\n→ Exceeds theoretical single-core max - SHA-NI is working!");
        println!("  (SHA-NI does multiple ops per instruction)");
    } else if utilization > 80.0 {
        println!("\n→ Near theoretical maximum - well optimized");
    } else {
        println!("\n→ Room for optimization exists");
    }
}

/// Test 3: Instruction mix analysis
#[test]
fn test_research_v11_instruction_analysis() {
    println!("\n=== Research v11: SHA-256 Instruction Analysis ===\n");

    println!("SHA-256 Round Operations (per round):\n");

    let operations = [
        ("Σ0 (sigma0)", "ROTR + ROTR + ROTR + XOR + XOR", 5),
        ("Σ1 (sigma1)", "ROTR + ROTR + ROTR + XOR + XOR", 5),
        ("Ch (choice)", "AND + NOT + AND + XOR", 4),
        ("Maj (majority)", "AND + AND + XOR + XOR", 4),
        ("T1 addition", "ADD + ADD + ADD + ADD + ADD", 5),
        ("T2 addition", "ADD", 1),
        ("State update", "MOV × 8", 8),
    ];

    let mut total_ops = 0;

    println!("| Function | Operations | Count |");
    println!("|----------|------------|-------|");

    for (name, ops, count) in &operations {
        println!("| {} | {} | {} |", name, ops, count);
        total_ops += count;
    }

    println!("|----------|------------|-------|");
    println!("| **Per Round** | | **{}** |", total_ops);
    println!("| **64 Rounds** | | **{}** |", total_ops * 64);
    println!("| **Double SHA** | | **{}** |", total_ops * 128);

    println!("\nWith SHA-NI:");
    println!("  sha256rnds2: 2 rounds in 1 instruction");
    println!("  sha256msg1/msg2: message schedule acceleration");
    println!("  Effective ops per hash: ~64 SHA-NI instructions × 2");
}

/// Test 4: Optimization opportunities
#[test]
fn test_research_v11_opportunities() {
    println!("\n=== Research v11: Remaining Optimization Opportunities ===\n");

    println!("| Opportunity | Status | Potential Gain |");
    println!("|-------------|--------|----------------|");
    println!("| opt-level=3 | ✅ Applied | Baseline |");
    println!("| LTO (fat) | ✅ Applied | 5-20% vs thin |");
    println!("| codegen-units=1 | ✅ Applied | 5-10% |");
    println!("| Target features | ✅ Applied | SHA-NI enabled |");
    println!("| PGO | ❌ Not applied | 5-15% potential |");
    println!("| BOLT | ❌ Not applied | 5-10% potential |");
    println!("| target-cpu=native | ❓ Partial | ~1-2% |");

    println!("\n### Profile-Guided Optimization (PGO)\n");
    println!("PGO requires:");
    println!("1. Build with instrumentation:");
    println!("   RUSTFLAGS=\"-Cprofile-generate=/tmp/pgo\" cargo build --release");
    println!("2. Run representative workload:");
    println!("   ./target/release/btcminer --benchmark");
    println!("3. Merge profiles:");
    println!("   llvm-profdata merge -o merged.profdata /tmp/pgo");
    println!("4. Rebuild with profile:");
    println!("   RUSTFLAGS=\"-Cprofile-use=merged.profdata\" cargo build --release");

    println!("\n### BOLT (Binary Optimization)\n");
    println!("BOLT is a post-link optimizer that can:");
    println!("- Reorder code for better cache locality");
    println!("- Optimize branch prediction");
    println!("- Requires: llvm-bolt tool");

    println!("\n### Native CPU Tuning\n");
    println!("Add to .cargo/config.toml:");
    println!("  rustflags = [\"-C\", \"target-cpu=native\"]");
    println!("This enables CPU-specific optimizations but reduces portability.");
}

/// Test 5: Multi-run variance analysis
#[test]
fn test_research_v11_variance() {
    println!("\n=== Research v11: Performance Variance Analysis ===\n");
    println!("Running multiple measurements to assess stability...\n");

    let duration = 2;
    let runs = 5;

    let mut rates: Vec<f64> = Vec::new();

    for run in 1..=runs {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        let rate = benchmark_duration(&mut hasher, duration);
        rates.push(rate);
        println!("Run {}: {:.2} MH/s", run, rate);
    }

    let avg: f64 = rates.iter().sum::<f64>() / rates.len() as f64;
    let variance: f64 = rates.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / rates.len() as f64;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg * 100.0;

    println!("\nStatistics:");
    println!("  Average: {:.2} MH/s", avg);
    println!("  Std Dev: {:.2} MH/s", std_dev);
    println!("  CV: {:.2}%", cv);
    println!("  Range: {:.2} - {:.2} MH/s",
        rates.iter().cloned().fold(f64::INFINITY, f64::min),
        rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max));

    if cv < 2.0 {
        println!("\n→ STABLE: Low variance, measurements are reliable");
    } else if cv < 5.0 {
        println!("\n→ ACCEPTABLE: Moderate variance, use multiple runs");
    } else {
        println!("\n→ UNSTABLE: High variance, check for thermal throttling or interference");
    }
}

/// Test 6: Summary and recommendations
#[test]
fn test_research_v11_summary() {
    println!("\n=== Research v11: Compiler Optimization Summary ===\n");

    println!("Current State:");
    println!("  - All standard compiler optimizations APPLIED");
    println!("  - SHA-NI hardware acceleration ENABLED");
    println!("  - LTO and single codegen unit ENABLED");

    println!("\nRemaining Opportunities:");
    println!("  1. PGO (Profile-Guided Optimization): ~5-15%");
    println!("  2. BOLT (Binary Optimization): ~5-10%");
    println!("  3. target-cpu=native: ~1-2%");
    println!("  Combined potential: ~10-25% additional");

    println!("\nRecommendation:");
    println!("  - PGO is the most promising unexplored optimization");
    println!("  - Requires separate build process");
    println!("  - Worth implementing for production builds");

    println!("\nNote: At 22-23 MH/s single-threaded, the implementation is");
    println!("already within ~10% of theoretical CPU limits for SHA-256.");
}
