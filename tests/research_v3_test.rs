//! Research Experiment v3: Input Space Reduction / Nonce Bias Analysis
//!
//! Tests whether certain nonce values or patterns are more likely to produce low hashes.
//! If bias exists, we could prioritize "good" nonce ranges.
//!
//! Correlates to: docs/RESEARCH-test-log-v3.md
//! Run with: cargo test --release --test research_v3_test -- --nocapture

use sha2::{Sha256, Digest};

/// Double SHA-256 of 80-byte header
fn sha256d(data: &[u8; 80]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Generate Bitcoin-like header with given nonce
/// Uses a realistic structure similar to actual block headers
fn bitcoin_header_with_nonce(seed: u64, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];

    // Version (4 bytes) - typically 0x20000000 or similar
    header[0..4].copy_from_slice(&0x20000004u32.to_le_bytes());

    // Previous block hash (32 bytes) - pseudo-random based on seed
    let mut state = seed;
    for i in 4..36 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }

    // Merkle root (32 bytes) - pseudo-random
    for i in 36..68 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }

    // Timestamp (4 bytes) - realistic timestamp
    let timestamp = 1706745600u32 + (seed as u32 % 86400); // ~Feb 2024 + up to 1 day
    header[68..72].copy_from_slice(&timestamp.to_le_bytes());

    // Bits/difficulty (4 bytes) - mainnet difficulty target
    header[72..76].copy_from_slice(&0x17034219u32.to_le_bytes());

    // Nonce (4 bytes)
    header[76..80].copy_from_slice(&nonce.to_le_bytes());

    header
}

/// Count leading zero bits in hash (for difficulty comparison)
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

/// Test 1: Nonce Modulo Distribution
/// Check if certain nonce % N values produce more leading zeros
#[test]
fn test_research_v3_nonce_modulo_bias() {
    println!("\n=== Research v3: Nonce Modulo Bias Analysis ===\n");
    println!("Question: Do certain nonce % N values produce more leading zeros?\n");

    let samples_per_bucket = 100_000;
    let modulos_to_test = [16, 256, 1024];

    for modulo in modulos_to_test {
        println!("Testing modulo {}", modulo);

        // Collect leading zeros for each bucket
        let mut bucket_zeros: Vec<Vec<u32>> = vec![Vec::new(); modulo];

        for sample in 0..(samples_per_bucket * modulo as u64) {
            let nonce = sample as u32;
            let header = bitcoin_header_with_nonce(sample / modulo as u64, nonce);
            let hash = sha256d(&header);
            let lz = leading_zero_bits(&hash);
            bucket_zeros[(nonce as usize) % modulo].push(lz);
        }

        // Calculate mean leading zeros per bucket
        let bucket_means: Vec<f64> = bucket_zeros.iter()
            .map(|b| b.iter().map(|&x| x as f64).sum::<f64>() / b.len() as f64)
            .collect();

        let overall_mean: f64 = bucket_means.iter().sum::<f64>() / modulo as f64;
        let variance: f64 = bucket_means.iter()
            .map(|m| (m - overall_mean).powi(2))
            .sum::<f64>() / modulo as f64;

        let min_mean = bucket_means.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_mean = bucket_means.iter().cloned().fold(0.0_f64, f64::max);
        let min_bucket = bucket_means.iter().position(|&m| m == min_mean).unwrap();
        let max_bucket = bucket_means.iter().position(|&m| m == max_mean).unwrap();

        println!("  Overall mean: {:.4} leading zero bits", overall_mean);
        println!("  Bucket variance: {:.6}", variance);
        println!("  Min bucket: {} (mean: {:.4})", min_bucket, min_mean);
        println!("  Max bucket: {} (mean: {:.4})", max_bucket, max_mean);
        println!("  Spread: {:.4} bits\n", max_mean - min_mean);

        // Chi-squared test
        // Expected: uniform distribution of leading zeros
        // If variance is close to 0, no bias
        assert!(variance < 0.01, "Unexpected variance detected");
    }
}

/// Test 2: Nonce Bit Pattern Analysis
/// Check if certain bit patterns in nonce correlate with leading zeros
#[test]
fn test_research_v3_nonce_bit_patterns() {
    println!("\n=== Research v3: Nonce Bit Pattern Analysis ===\n");
    println!("Question: Do nonce bit patterns correlate with leading zeros?\n");

    let samples = 2_000_000u64;

    // Track statistics for each bit position in nonce
    let mut bit_set_zeros: [u64; 32] = [0; 32];       // Sum of leading zeros when bit is set
    let mut bit_set_count: [u64; 32] = [0; 32];       // Count when bit is set
    let mut bit_clear_zeros: [u64; 32] = [0; 32];     // Sum when bit is clear
    let mut bit_clear_count: [u64; 32] = [0; 32];     // Count when bit is clear

    for sample in 0..samples {
        let nonce = (sample * 1000003) as u32; // Pseudo-random nonce distribution
        let header = bitcoin_header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = leading_zero_bits(&hash) as u64;

        for bit in 0..32 {
            if (nonce >> bit) & 1 == 1 {
                bit_set_zeros[bit] += lz;
                bit_set_count[bit] += 1;
            } else {
                bit_clear_zeros[bit] += lz;
                bit_clear_count[bit] += 1;
            }
        }
    }

    println!("| Bit Position | Mean LZ (bit=1) | Mean LZ (bit=0) | Difference |");
    println!("|--------------|-----------------|-----------------|------------|");

    let mut max_diff = 0.0_f64;
    for bit in 0..32 {
        let mean_set = bit_set_zeros[bit] as f64 / bit_set_count[bit] as f64;
        let mean_clear = bit_clear_zeros[bit] as f64 / bit_clear_count[bit] as f64;
        let diff = (mean_set - mean_clear).abs();
        max_diff = max_diff.max(diff);

        // Only print bits with notable differences (> 0.01)
        if diff > 0.01 {
            println!("| {:>12} | {:>15.4} | {:>15.4} | {:>10.4} |",
                bit, mean_set, mean_clear, diff);
        }
    }

    println!("\nMax difference across all bits: {:.4}", max_diff);
    println!("Expected (random): ~0");

    // Assert no significant bias
    assert!(max_diff < 0.1, "Unexpected bit pattern bias detected");
}

/// Test 3: High-Value Nonce Analysis
/// Check if high/low nonce values have different leading zero distributions
#[test]
fn test_research_v3_nonce_range_bias() {
    println!("\n=== Research v3: Nonce Range Bias Analysis ===\n");
    println!("Question: Do high vs low nonces produce different leading zeros?\n");

    let samples_per_range = 500_000u64;

    // Define ranges to test
    let ranges: [(u32, u32, &str); 4] = [
        (0x00000000, 0x3FFFFFFF, "Low (0x00-0x3F)"),
        (0x40000000, 0x7FFFFFFF, "Mid-low (0x40-0x7F)"),
        (0x80000000, 0xBFFFFFFF, "Mid-high (0x80-0xBF)"),
        (0xC0000000, 0xFFFFFFFF, "High (0xC0-0xFF)"),
    ];

    println!("| Range | Mean Leading Zeros | Variance |");
    println!("|-------|-------------------|----------|");

    let mut all_means: Vec<f64> = Vec::new();

    for (start, _end, name) in &ranges {
        let mut total_lz = 0u64;
        let mut total_lz_sq = 0u64;

        for i in 0..samples_per_range {
            let nonce = start.wrapping_add((i * 8191) as u32);
            let header = bitcoin_header_with_nonce(i, nonce);
            let hash = sha256d(&header);
            let lz = leading_zero_bits(&hash) as u64;
            total_lz += lz;
            total_lz_sq += lz * lz;
        }

        let mean = total_lz as f64 / samples_per_range as f64;
        let variance = (total_lz_sq as f64 / samples_per_range as f64) - mean * mean;
        all_means.push(mean);

        println!("| {:>17} | {:>17.4} | {:>8.4} |", name, mean, variance);
    }

    // Check if ranges differ significantly
    let overall_mean: f64 = all_means.iter().sum::<f64>() / all_means.len() as f64;
    let range_variance: f64 = all_means.iter()
        .map(|m| (m - overall_mean).powi(2))
        .sum::<f64>() / all_means.len() as f64;

    println!("\nOverall mean: {:.4}", overall_mean);
    println!("Variance between ranges: {:.6}", range_variance);
    println!("Expected (random): ~0");

    assert!(range_variance < 0.01, "Unexpected range bias detected");
}

/// Test 4: Special Nonce Patterns
/// Check if "round" numbers, powers of 2, etc. are special
#[test]
fn test_research_v3_special_nonces() {
    println!("\n=== Research v3: Special Nonce Pattern Analysis ===\n");
    println!("Question: Are powers of 2, round numbers, or palindromes special?\n");

    let samples = 1_000_000u64;
    let mut total_lz = 0u64;
    let mut count = 0u64;

    // Calculate baseline
    for sample in 0..samples {
        let nonce = sample as u32;
        let header = bitcoin_header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        total_lz += leading_zero_bits(&hash) as u64;
        count += 1;
    }
    let baseline_mean = total_lz as f64 / count as f64;

    // Test special patterns
    let test_patterns: Vec<(&str, Vec<u32>)> = vec![
        ("Powers of 2", (0..32).map(|i| 1u32 << i).collect()),
        ("Round thousands", (0..1000).map(|i| i * 1000).collect()),
        ("All 1s pattern", (0..32).map(|i| (1u32 << i) - 1).collect()),
        ("Alternating bits", vec![0x55555555, 0xAAAAAAAA, 0x33333333, 0xCCCCCCCC]),
        ("Palindromes (hex)", vec![0x00000000, 0x11111111, 0x12211221, 0xABCDDCBA, 0xDEADDAED]),
    ];

    println!("Baseline mean leading zeros: {:.4}\n", baseline_mean);
    println!("| Pattern | Samples | Mean LZ | Diff from Baseline |");
    println!("|---------|---------|---------|-------------------|");

    for (name, nonces) in test_patterns {
        let mut pattern_lz = 0u64;
        let mut pattern_count = 0u64;

        for seed in 0..1000u64 {
            for &nonce in &nonces {
                let header = bitcoin_header_with_nonce(seed, nonce);
                let hash = sha256d(&header);
                pattern_lz += leading_zero_bits(&hash) as u64;
                pattern_count += 1;
            }
        }

        let pattern_mean = pattern_lz as f64 / pattern_count as f64;
        let diff = pattern_mean - baseline_mean;

        println!("| {:>17} | {:>7} | {:>7.4} | {:>+17.4} |",
            name, pattern_count, pattern_mean, diff);
    }

    println!("\nNote: Differences should be near zero for random oracle.");
}

/// Test 5: Nonce + Header Interaction
/// Check if certain header states interact with nonce to produce bias
#[test]
fn test_research_v3_header_nonce_interaction() {
    println!("\n=== Research v3: Header-Nonce Interaction Analysis ===\n");
    println!("Question: Does the fixed header structure create nonce bias?\n");

    let samples = 500_000u64;

    // Test with different header configurations
    let configs: Vec<(&str, Box<dyn Fn(u32) -> [u8; 80]>)> = vec![
        ("All zeros header", Box::new(|nonce| {
            let mut h = [0u8; 80];
            h[76..80].copy_from_slice(&nonce.to_le_bytes());
            h
        })),
        ("All ones header", Box::new(|nonce| {
            let mut h = [0xFF; 80];
            h[76..80].copy_from_slice(&nonce.to_le_bytes());
            h
        })),
        ("Realistic header", Box::new(|nonce| {
            bitcoin_header_with_nonce(12345, nonce)
        })),
        ("High entropy header", Box::new(|nonce| {
            let mut h = [0u8; 80];
            for i in 0..76 {
                h[i] = ((i * 17 + i * i) % 256) as u8;
            }
            h[76..80].copy_from_slice(&nonce.to_le_bytes());
            h
        })),
    ];

    println!("| Header Type | Mean LZ | Std Dev |");
    println!("|-------------|---------|---------|");

    for (name, header_fn) in &configs {
        let mut total_lz = 0u64;
        let mut total_lz_sq = 0u64;

        for sample in 0..samples {
            let nonce = sample as u32;
            let header = header_fn(nonce);
            let hash = sha256d(&header);
            let lz = leading_zero_bits(&hash) as u64;
            total_lz += lz;
            total_lz_sq += lz * lz;
        }

        let mean = total_lz as f64 / samples as f64;
        let variance = (total_lz_sq as f64 / samples as f64) - mean * mean;
        let std_dev = variance.sqrt();

        println!("| {:>17} | {:>7.4} | {:>7.4} |", name, mean, std_dev);
    }

    println!("\nNote: All configurations should produce identical statistics.");
}

/// Test 6: Chi-Squared Uniformity Test
/// Formal statistical test for uniform distribution
#[test]
fn test_research_v3_chi_squared_uniformity() {
    println!("\n=== Research v3: Chi-Squared Uniformity Test ===\n");
    println!("Testing if leading zero distribution matches theoretical expectation.\n");

    let samples = 5_000_000u64;
    let mut lz_counts: [u64; 33] = [0; 33]; // 0-32 leading zero bits

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = bitcoin_header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = leading_zero_bits(&hash).min(32) as usize;
        lz_counts[lz] += 1;
    }

    // Expected distribution: P(k leading zeros) = 2^(-k-1) for k < 256
    // For k = 0: 50%, k = 1: 25%, k = 2: 12.5%, etc.
    let mut chi_squared = 0.0_f64;

    println!("| LZ Bits | Observed | Expected | Ratio |");
    println!("|---------|----------|----------|-------|");

    for k in 0..=16 {
        let observed = lz_counts[k] as f64;
        let expected = samples as f64 / (2.0_f64.powi(k as i32 + 1));
        let ratio = observed / expected;

        println!("| {:>7} | {:>8.0} | {:>8.0} | {:>5.3} |",
            k, observed, expected, ratio);

        if expected > 5.0 {
            chi_squared += (observed - expected).powi(2) / expected;
        }
    }

    // Degrees of freedom ~ 16 (categories with expected > 5)
    // Critical value for p=0.05, df=16: 26.3
    println!("\nChi-squared statistic: {:.2}", chi_squared);
    println!("Critical value (p=0.05, df=16): 26.3");
    println!("Result: {}", if chi_squared < 26.3 { "PASS (uniform)" } else { "FAIL (non-uniform)" });

    assert!(chi_squared < 50.0, "Distribution significantly non-uniform");
}
