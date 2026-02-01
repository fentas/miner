//! Research Experiment v2: Differential/Consecutive Nonce Analysis
//!
//! Tests whether knowing hash(nonce=N) provides information about hash(nonce=N+1).
//! If correlation exists, we could use previous hash to guide search.
//!
//! Correlates to: docs/RESEARCH-test-log-v2.md
//! Run with: cargo test --release --test research_v2_test -- --nocapture

use sha2::{Sha256, Digest};

/// Double SHA-256 of 80-byte header
fn sha256d(data: &[u8; 80]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Generate deterministic header with given nonce
fn header_with_nonce(seed: u64, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];

    // Fill with deterministic pseudo-random based on seed
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }

    // Set nonce at bytes 76-79 (little-endian)
    header[76..80].copy_from_slice(&nonce.to_le_bytes());

    header
}

/// Count leading zero bits in hash
fn leading_zero_bits(hash: &[u8; 32]) -> u32 {
    let mut count = 0;
    for byte in hash.iter() {
        if *byte == 0 {
            count += 8;
        } else {
            count += byte.leading_zeros();
            break;
        }
    }
    count
}

/// Hamming distance between two hashes (number of differing bits)
fn hamming_distance(a: &[u8; 32], b: &[u8; 32]) -> u32 {
    let mut dist = 0;
    for i in 0..32 {
        dist += (a[i] ^ b[i]).count_ones();
    }
    dist
}

#[derive(Debug, Default)]
struct DifferentialStats {
    total_pairs: u64,

    // Leading zeros correlation
    both_have_zeros: u64,      // N and N+1 both have ≥1 leading zero byte
    neither_has_zeros: u64,
    only_first: u64,
    only_second: u64,

    // Bit flip statistics
    total_bit_flips: u64,      // Cumulative hamming distance
    min_flips: u32,
    max_flips: u32,

    // Leading zero bit correlation
    leading_bits_same: u64,    // Same number of leading zero bits
    leading_bits_close: u64,   // Within 2 bits

    // First byte correlation
    first_byte_same: u64,
    first_byte_close: u64,     // Within ±16
}

impl DifferentialStats {
    fn avg_bit_flips(&self) -> f64 {
        self.total_bit_flips as f64 / self.total_pairs as f64
    }

    fn leading_zeros_correlation(&self) -> f64 {
        (self.both_have_zeros + self.neither_has_zeros) as f64 / self.total_pairs as f64
    }
}

/// Run differential analysis between consecutive nonces
fn run_differential_experiment(samples: u64) -> DifferentialStats {
    let mut stats = DifferentialStats {
        min_flips: u32::MAX,
        ..Default::default()
    };

    for seed in 0..samples {
        // Use seed as both the header seed and a nonce base
        let nonce_base = (seed * 1000) as u32;

        let header1 = header_with_nonce(seed, nonce_base);
        let header2 = header_with_nonce(seed, nonce_base + 1);

        let hash1 = sha256d(&header1);
        let hash2 = sha256d(&header2);

        stats.total_pairs += 1;

        // Leading zeros analysis
        let has_zero1 = hash1[0] == 0;
        let has_zero2 = hash2[0] == 0;

        match (has_zero1, has_zero2) {
            (true, true) => stats.both_have_zeros += 1,
            (false, false) => stats.neither_has_zeros += 1,
            (true, false) => stats.only_first += 1,
            (false, true) => stats.only_second += 1,
        }

        // Bit flip analysis
        let flips = hamming_distance(&hash1, &hash2);
        stats.total_bit_flips += flips as u64;
        stats.min_flips = stats.min_flips.min(flips);
        stats.max_flips = stats.max_flips.max(flips);

        // Leading zero bits
        let lz1 = leading_zero_bits(&hash1);
        let lz2 = leading_zero_bits(&hash2);
        if lz1 == lz2 {
            stats.leading_bits_same += 1;
        }
        if (lz1 as i32 - lz2 as i32).abs() <= 2 {
            stats.leading_bits_close += 1;
        }

        // First byte correlation
        if hash1[0] == hash2[0] {
            stats.first_byte_same += 1;
        }
        if (hash1[0] as i32 - hash2[0] as i32).abs() <= 16 {
            stats.first_byte_close += 1;
        }
    }

    stats
}

/// Test if consecutive "good" hashes cluster
fn run_clustering_experiment(samples: u64, threshold: u8) -> (u64, u64, f64) {
    let mut consecutive_good = 0u64;
    let mut total_good = 0u64;

    for seed in 0..samples {
        let nonce_base = (seed * 1000) as u32;

        let header1 = header_with_nonce(seed, nonce_base);
        let header2 = header_with_nonce(seed, nonce_base + 1);

        let hash1 = sha256d(&header1);
        let hash2 = sha256d(&header2);

        let good1 = hash1[0] < threshold;
        let good2 = hash2[0] < threshold;

        if good1 {
            total_good += 1;
            if good2 {
                consecutive_good += 1;
            }
        }
    }

    let expected_rate = threshold as f64 / 256.0;
    let observed_rate = if total_good > 0 {
        consecutive_good as f64 / total_good as f64
    } else {
        0.0
    };

    (consecutive_good, total_good, observed_rate / expected_rate)
}

/// Test nonce step sizes other than 1
fn run_step_analysis(samples: u64, steps: &[u32]) {
    println!("\n=== Nonce Step Size Analysis ===\n");
    println!("| Step Size | Avg Bit Flips | First Byte Same | Leading Bits Same |");
    println!("|-----------|---------------|-----------------|-------------------|");

    for &step in steps {
        let mut total_flips = 0u64;
        let mut first_same = 0u64;
        let mut leading_same = 0u64;

        for seed in 0..samples {
            let nonce_base = (seed * 1000) as u32;

            let header1 = header_with_nonce(seed, nonce_base);
            let header2 = header_with_nonce(seed, nonce_base.wrapping_add(step));

            let hash1 = sha256d(&header1);
            let hash2 = sha256d(&header2);

            total_flips += hamming_distance(&hash1, &hash2) as u64;
            if hash1[0] == hash2[0] { first_same += 1; }
            if leading_zero_bits(&hash1) == leading_zero_bits(&hash2) { leading_same += 1; }
        }

        println!(
            "| {:>9} | {:>13.2} | {:>14.2}% | {:>17.2}% |",
            step,
            total_flips as f64 / samples as f64,
            first_same as f64 / samples as f64 * 100.0,
            leading_same as f64 / samples as f64 * 100.0
        );
    }
}

#[test]
fn test_differential_analysis() {
    println!("\n=== Differential/Consecutive Nonce Analysis ===\n");

    let samples = 1_000_000;
    println!("Samples: {}\n", samples);

    let stats = run_differential_experiment(samples);

    println!("## Bit Flip Statistics");
    println!("  Average bit flips between consecutive nonces: {:.2}", stats.avg_bit_flips());
    println!("  Expected (random): 128.0 (50% of 256 bits)");
    println!("  Min flips: {}", stats.min_flips);
    println!("  Max flips: {}", stats.max_flips);
    println!();

    println!("## Leading Zero Byte Correlation");
    println!("  Both have leading zero byte: {} ({:.4}%)",
        stats.both_have_zeros,
        stats.both_have_zeros as f64 / samples as f64 * 100.0);
    println!("  Neither has leading zero: {} ({:.4}%)",
        stats.neither_has_zeros,
        stats.neither_has_zeros as f64 / samples as f64 * 100.0);
    println!("  Only first: {} ({:.4}%)",
        stats.only_first,
        stats.only_first as f64 / samples as f64 * 100.0);
    println!("  Only second: {} ({:.4}%)",
        stats.only_second,
        stats.only_second as f64 / samples as f64 * 100.0);
    println!("  Correlation: {:.4}%", stats.leading_zeros_correlation() * 100.0);
    println!("  Expected (independent): ~{:.4}%",
        (1.0_f64/256.0).powi(2) * 100.0 + (255.0_f64/256.0).powi(2) * 100.0);
    println!();

    println!("## First Byte Correlation");
    println!("  Exactly same: {} ({:.4}%)",
        stats.first_byte_same,
        stats.first_byte_same as f64 / samples as f64 * 100.0);
    println!("  Expected (random): {:.4}%", 1.0/256.0 * 100.0);
    println!("  Within ±16: {} ({:.4}%)",
        stats.first_byte_close,
        stats.first_byte_close as f64 / samples as f64 * 100.0);
    println!("  Expected (random): {:.4}%", 33.0/256.0 * 100.0);
    println!();

    println!("## Leading Zero Bits");
    println!("  Exactly same count: {} ({:.4}%)",
        stats.leading_bits_same,
        stats.leading_bits_same as f64 / samples as f64 * 100.0);
    println!("  Within ±2 bits: {} ({:.4}%)",
        stats.leading_bits_close,
        stats.leading_bits_close as f64 / samples as f64 * 100.0);
}

#[test]
fn test_clustering_analysis() {
    println!("\n=== Good Hash Clustering Analysis ===\n");
    println!("Question: If hash(N) is 'good', is hash(N+1) more likely to be 'good'?\n");

    let samples = 5_000_000;

    println!("| Threshold | Total Good | Consecutive Good | Observed Rate | Expected | Clustering Ratio |");
    println!("|-----------|------------|------------------|---------------|----------|------------------|");

    for threshold in [0x01u8, 0x02, 0x04, 0x08, 0x10, 0x20] {
        let (consecutive, total, ratio) = run_clustering_experiment(samples, threshold);
        let expected_rate = threshold as f64 / 256.0;
        let observed_rate = if total > 0 { consecutive as f64 / total as f64 } else { 0.0 };

        println!(
            "| 0x{:02x}      | {:>10} | {:>16} | {:>12.4}% | {:>7.4}% | {:>16.4} |",
            threshold,
            total,
            consecutive,
            observed_rate * 100.0,
            expected_rate * 100.0,
            ratio
        );
    }

    println!("\nClustering Ratio = 1.0 means no clustering (independent)");
    println!("Clustering Ratio > 1.0 would mean good hashes cluster together");
}

#[test]
fn test_nonce_step_sizes() {
    let samples = 500_000;
    run_step_analysis(samples, &[1, 2, 4, 8, 16, 256, 65536, 0x01000000]);
}

#[test]
fn test_bit_position_analysis() {
    println!("\n=== Which Bits Flip Most Between Consecutive Nonces? ===\n");

    let samples = 500_000;
    let mut bit_flip_counts = [0u64; 256];

    for seed in 0..samples {
        let nonce_base = (seed * 1000) as u32;

        let header1 = header_with_nonce(seed, nonce_base);
        let header2 = header_with_nonce(seed, nonce_base + 1);

        let hash1 = sha256d(&header1);
        let hash2 = sha256d(&header2);

        for byte_idx in 0..32 {
            let diff = hash1[byte_idx] ^ hash2[byte_idx];
            for bit in 0..8 {
                if (diff >> bit) & 1 == 1 {
                    bit_flip_counts[byte_idx * 8 + bit] += 1;
                }
            }
        }
    }

    // Check if any bit position flips significantly more/less than 50%
    println!("Bit positions with unusual flip rates (expected: 50%):\n");

    let mut unusual_count = 0;
    for (pos, &count) in bit_flip_counts.iter().enumerate() {
        let rate = count as f64 / samples as f64;
        if (rate - 0.5).abs() > 0.01 {  // More than 1% deviation
            println!("  Bit {:>3}: {:.2}% flip rate", pos, rate * 100.0);
            unusual_count += 1;
        }
    }

    if unusual_count == 0 {
        println!("  None found - all bits flip at ~50% rate (independent/random)");
    }

    // Summary statistics
    let rates: Vec<f64> = bit_flip_counts.iter().map(|&c| c as f64 / samples as f64).collect();
    let min_rate = rates.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_rate = rates.iter().cloned().fold(0.0, f64::max);
    let avg_rate: f64 = rates.iter().sum::<f64>() / 256.0;

    println!("\nSummary:");
    println!("  Min flip rate: {:.4}%", min_rate * 100.0);
    println!("  Max flip rate: {:.4}%", max_rate * 100.0);
    println!("  Avg flip rate: {:.4}%", avg_rate * 100.0);
}

#[test]
fn test_direction_correlation() {
    println!("\n=== Does First Byte Trend Direction Persist? ===\n");
    println!("Question: If hash(N)[0] > hash(N-1)[0], is hash(N+1)[0] > hash(N)[0]?\n");

    let samples = 1_000_000;
    let mut up_then_up = 0u64;
    let mut up_then_down = 0u64;
    let mut down_then_up = 0u64;
    let mut down_then_down = 0u64;

    for seed in 0..samples {
        let nonce_base = (seed * 1000) as u32;

        let h0 = sha256d(&header_with_nonce(seed, nonce_base));
        let h1 = sha256d(&header_with_nonce(seed, nonce_base + 1));
        let h2 = sha256d(&header_with_nonce(seed, nonce_base + 2));

        let trend1 = (h1[0] as i32) - (h0[0] as i32); // positive = up
        let trend2 = (h2[0] as i32) - (h1[0] as i32);

        match (trend1 > 0, trend2 > 0) {
            (true, true) => up_then_up += 1,
            (true, false) => up_then_down += 1,
            (false, true) => down_then_up += 1,
            (false, false) => down_then_down += 1,
        }
    }

    let total = samples as f64;
    println!("| First Trend | Second Trend | Count    | Percentage |");
    println!("|-------------|--------------|----------|------------|");
    println!("| Up          | Up           | {:>8} | {:>9.2}% |", up_then_up, up_then_up as f64 / total * 100.0);
    println!("| Up          | Down         | {:>8} | {:>9.2}% |", up_then_down, up_then_down as f64 / total * 100.0);
    println!("| Down        | Up           | {:>8} | {:>9.2}% |", down_then_up, down_then_up as f64 / total * 100.0);
    println!("| Down        | Down         | {:>8} | {:>9.2}% |", down_then_down, down_then_down as f64 / total * 100.0);

    println!("\nExpected (random): 25% each");
    println!("If trends persist: Up→Up and Down→Down would be > 25%");
}
