//! Research Experiment v4: Round Function Algebraic Analysis
//!
//! Explores the algebraic structure of SHA-256 to find exploitable patterns.
//! Tests linear approximation, carry propagation, and output bit dependencies.
//!
//! Correlates to: docs/RESEARCH-test-log-v4.md
//! Run with: cargo test --release --test research_v4_test -- --nocapture

use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// Double SHA-256 of 80-byte header
fn sha256d(data: &[u8; 80]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(&first);
    let mut result = [0u8; 32];
    result.copy_from_slice(&second);
    result
}

/// Generate header with given nonce
fn header_with_nonce(seed: u64, nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    let mut state = seed;
    for i in 0..76 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    header
}

/// Test 1: Linear Approximation Analysis
/// SHA-256 uses non-linear operations. How "linear" is it in practice?
/// A perfectly random function has 50% linear correlation with any linear function.
#[test]
fn test_research_v4_linear_approximation() {
    println!("\n=== Research v4: Linear Approximation Analysis ===\n");
    println!("Question: Can any linear function of input bits predict output bits?\n");

    let samples = 500_000u64;

    // Test if XOR of specific input bits correlates with XOR of output bits
    // For a random oracle, correlation should be exactly 50%

    // Test: XOR of nonce bits 0,1,2,3 vs XOR of hash bits 0,1,2,3
    let mut matches_4bit = 0u64;
    // Test: XOR of all nonce bits vs XOR of first hash byte
    let mut matches_full_nonce = 0u64;
    // Test: Single bit correlation (nonce bit 0 vs hash bit 0)
    let mut matches_single = 0u64;

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);

        // XOR of low 4 nonce bits
        let nonce_parity_4 = (nonce & 0xF).count_ones() % 2;
        // XOR of first 4 hash bits
        let hash_parity_4 = (hash[0] & 0xF).count_ones() % 2;
        if nonce_parity_4 == hash_parity_4 {
            matches_4bit += 1;
        }

        // Full nonce XOR vs first byte XOR
        let nonce_parity_full = nonce.count_ones() % 2;
        let hash_parity_byte = hash[0].count_ones() % 2;
        if nonce_parity_full == hash_parity_byte {
            matches_full_nonce += 1;
        }

        // Single bit
        let nonce_bit0 = nonce & 1;
        let hash_bit0 = hash[0] as u32 & 1;
        if nonce_bit0 == hash_bit0 {
            matches_single += 1;
        }
    }

    let rate_4bit = matches_4bit as f64 / samples as f64;
    let rate_full = matches_full_nonce as f64 / samples as f64;
    let rate_single = matches_single as f64 / samples as f64;

    println!("| Linear Approximation | Match Rate | Expected | Bias |");
    println!("|---------------------|------------|----------|------|");
    println!("| XOR(nonce[0:3]) vs XOR(hash[0:3]) | {:.4}% | 50.00% | {:+.4}% |",
        rate_4bit * 100.0, (rate_4bit - 0.5) * 100.0);
    println!("| XOR(all nonce) vs XOR(hash[0:7]) | {:.4}% | 50.00% | {:+.4}% |",
        rate_full * 100.0, (rate_full - 0.5) * 100.0);
    println!("| nonce[0] vs hash[0] | {:.4}% | 50.00% | {:+.4}% |",
        rate_single * 100.0, (rate_single - 0.5) * 100.0);

    println!("\nNote: Bias should be ~0 for a secure hash function.");

    // Assert no significant linear correlation
    assert!((rate_4bit - 0.5).abs() < 0.01, "Unexpected linear correlation");
    assert!((rate_full - 0.5).abs() < 0.01, "Unexpected linear correlation");
    assert!((rate_single - 0.5).abs() < 0.01, "Unexpected linear correlation");
}

/// Test 2: Carry Propagation Analysis
/// SHA-256 uses modular addition which creates carry chains.
/// Can we predict hash based on carry propagation patterns?
#[test]
fn test_research_v4_carry_propagation() {
    println!("\n=== Research v4: Carry Propagation Analysis ===\n");
    println!("Question: Do nonces with more 1-bits (more carries) produce different hashes?\n");

    let samples = 1_000_000u64;

    // Group nonces by popcount (number of 1 bits)
    let mut lz_by_popcount: HashMap<u32, Vec<u32>> = HashMap::new();

    for sample in 0..samples {
        let nonce = sample as u32;
        let popcount = nonce.count_ones();
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = hash.iter().take_while(|&&b| b == 0).count() as u32 * 8
            + hash.iter().skip_while(|&&b| b == 0).next().map_or(0, |b| b.leading_zeros());

        lz_by_popcount.entry(popcount).or_default().push(lz);
    }

    println!("| Nonce Popcount | Samples | Mean LZ | Std Dev |");
    println!("|----------------|---------|---------|---------|");

    let mut means: Vec<f64> = Vec::new();
    for popcount in 0..=32 {
        if let Some(lzs) = lz_by_popcount.get(&popcount) {
            if lzs.len() > 100 {
                let mean = lzs.iter().map(|&x| x as f64).sum::<f64>() / lzs.len() as f64;
                let variance = lzs.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / lzs.len() as f64;
                let std_dev = variance.sqrt();
                means.push(mean);
                println!("| {:>14} | {:>7} | {:>7.4} | {:>7.4} |", popcount, lzs.len(), mean, std_dev);
            }
        }
    }

    let overall_mean: f64 = means.iter().sum::<f64>() / means.len() as f64;
    let variance: f64 = means.iter().map(|m| (m - overall_mean).powi(2)).sum::<f64>() / means.len() as f64;

    println!("\nVariance between popcount groups: {:.6}", variance);
    println!("Expected (random): ~0");

    assert!(variance < 0.01, "Unexpected carry propagation bias");
}

/// Test 3: Output Bit Independence
/// In a perfect hash, each output bit should be independent.
/// Test if pairs of output bits are correlated.
#[test]
fn test_research_v4_output_bit_independence() {
    println!("\n=== Research v4: Output Bit Independence ===\n");
    println!("Question: Are output bits correlated with each other?\n");

    let samples = 500_000u64;

    // Track correlation between first few output bits
    let mut bit_correlations: [[u64; 8]; 8] = [[0; 8]; 8];

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let first_byte = hash[0];

        for i in 0..8 {
            for j in 0..8 {
                let bit_i = (first_byte >> i) & 1;
                let bit_j = (first_byte >> j) & 1;
                if bit_i == bit_j {
                    bit_correlations[i][j] += 1;
                }
            }
        }
    }

    println!("Correlation matrix (first 8 output bits):");
    println!("Expected: 50% for i≠j, 100% for i=j\n");

    print!("     ");
    for j in 0..8 {
        print!("  Bit{} ", j);
    }
    println!();

    let mut max_off_diagonal = 0.0_f64;
    for i in 0..8 {
        print!("Bit{} ", i);
        for j in 0..8 {
            let rate = bit_correlations[i][j] as f64 / samples as f64;
            if i != j {
                max_off_diagonal = max_off_diagonal.max((rate - 0.5).abs());
            }
            print!(" {:>5.1}%", rate * 100.0);
        }
        println!();
    }

    println!("\nMax off-diagonal deviation from 50%: {:.4}%", max_off_diagonal * 100.0);
    println!("Expected (random): ~0%");

    assert!(max_off_diagonal < 0.01, "Unexpected bit correlation");
}

/// Test 4: Message Schedule Dependency
/// SHA-256 expands 16 words to 64 words. Does expansion create exploitable structure?
#[test]
fn test_research_v4_message_schedule_dependency() {
    println!("\n=== Research v4: Message Schedule Dependency ===\n");
    println!("Question: Does the nonce's position in message schedule create patterns?\n");

    // The nonce is in word 19 (bytes 76-79) of the 80-byte header
    // After padding, it's in the second SHA-256 block
    // Its influence propagates through the message schedule

    let samples = 500_000u64;

    // Test if flipping specific nonce bits has predictable effects
    let mut bit_influence: [f64; 32] = [0.0; 32];

    for sample in 0..samples {
        let nonce = sample as u32;
        let header1 = header_with_nonce(sample, nonce);
        let hash1 = sha256d(&header1);

        // Flip each bit and measure effect
        for bit in 0..32 {
            let flipped_nonce = nonce ^ (1 << bit);
            let header2 = header_with_nonce(sample, flipped_nonce);
            let hash2 = sha256d(&header2);

            // Count bit differences in output
            let diff: u32 = hash1.iter().zip(hash2.iter())
                .map(|(a, b)| (a ^ b).count_ones())
                .sum();

            bit_influence[bit] += diff as f64;
        }
    }

    // Normalize
    for bit in 0..32 {
        bit_influence[bit] /= samples as f64;
    }

    println!("| Nonce Bit | Avg Output Bits Flipped | Expected |");
    println!("|-----------|------------------------|----------|");

    let mut min_influence = f64::INFINITY;
    let mut max_influence = 0.0_f64;

    for bit in 0..32 {
        let influence = bit_influence[bit];
        min_influence = min_influence.min(influence);
        max_influence = max_influence.max(influence);

        if bit % 8 == 0 || bit == 31 {  // Print every 8th bit + last
            println!("| {:>9} | {:>22.2} | ~128 |", bit, influence);
        }
    }

    println!("\nMin influence: {:.2}", min_influence);
    println!("Max influence: {:.2}", max_influence);
    println!("Spread: {:.2}", max_influence - min_influence);
    println!("Expected spread (random): ~0");

    // All bits should have equal influence (~128 bits = 50% of 256)
    assert!((max_influence - min_influence) < 2.0, "Unexpected influence asymmetry");
}

/// Test 5: Hash Distribution Entropy
/// Measure the entropy of hash output distribution
#[test]
fn test_research_v4_entropy_analysis() {
    println!("\n=== Research v4: Hash Distribution Entropy ===\n");
    println!("Question: Is the hash output truly maximum entropy?\n");

    let samples = 1_000_000u64;

    // Measure byte-level entropy of first hash byte
    let mut byte_counts: [u64; 256] = [0; 256];

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        byte_counts[hash[0] as usize] += 1;
    }

    // Calculate entropy
    let mut entropy = 0.0_f64;
    for count in byte_counts.iter() {
        if *count > 0 {
            let p = *count as f64 / samples as f64;
            entropy -= p * p.log2();
        }
    }

    let max_entropy = 8.0; // log2(256) = 8 bits for perfect uniform
    let efficiency = entropy / max_entropy;

    println!("First byte entropy: {:.6} bits", entropy);
    println!("Maximum entropy: {:.6} bits", max_entropy);
    println!("Entropy efficiency: {:.6}%", efficiency * 100.0);

    // Check for byte value anomalies
    let expected_count = samples as f64 / 256.0;
    let max_deviation = byte_counts.iter()
        .map(|&c| (c as f64 - expected_count).abs() / expected_count)
        .fold(0.0_f64, f64::max);

    println!("\nExpected count per byte value: {:.1}", expected_count);
    println!("Max deviation from expected: {:.2}%", max_deviation * 100.0);

    // Entropy should be very close to 8.0
    assert!(entropy > 7.99, "Entropy too low - distribution not uniform");
    assert!(efficiency > 0.999, "Entropy efficiency too low");
}

/// Test 6: Algebraic Attack Feasibility
/// Test if any simple algebraic relationship exists between input and output
#[test]
fn test_research_v4_algebraic_relationships() {
    println!("\n=== Research v4: Algebraic Relationship Analysis ===\n");
    println!("Question: Are there any simple algebraic relationships?\n");

    let samples = 500_000u64;

    // Test various algebraic hypotheses
    let mut tests: Vec<(&str, u64)> = vec![
        ("nonce & hash = 0", 0),
        ("nonce ^ hash = 0", 0),
        ("nonce + hash_int = round", 0),
        ("hash first 4 bytes divisible by nonce", 0),
    ];

    for sample in 1..samples {  // Start from 1 to avoid division by zero
        let nonce = (sample * 1000003) as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);

        // Extract first 4 bytes of hash as u32
        let hash_int = u32::from_le_bytes([hash[0], hash[1], hash[2], hash[3]]);

        // Test 1: nonce & hash_int == 0 (would mean opposite bits)
        if nonce & hash_int == 0 {
            tests[0].1 += 1;
        }

        // Test 2: nonce ^ hash_int == 0 (would mean identical bits)
        if nonce ^ hash_int == 0 {
            tests[1].1 += 1;
        }

        // Test 3: Sum is a round number (power of 2)
        let sum = nonce.wrapping_add(hash_int);
        if sum.count_ones() == 1 {
            tests[2].1 += 1;
        }

        // Test 4: hash_int divisible by nonce
        if nonce != 0 && hash_int % nonce == 0 {
            tests[3].1 += 1;
        }
    }

    println!("| Relationship | Occurrences | Rate | Expected (random) |");
    println!("|--------------|-------------|------|-------------------|");

    for (name, count) in tests {
        let rate = count as f64 / samples as f64;
        let expected = match name {
            "nonce & hash = 0" => "~0%",
            "nonce ^ hash = 0" => "~0%",
            "nonce + hash_int = round" => "~0.003%",
            "hash first 4 bytes divisible by nonce" => "varies",
            _ => "varies",
        };
        println!("| {:>40} | {:>11} | {:>7.4}% | {:>17} |",
            name, count, rate * 100.0, expected);
    }

    println!("\nNote: No algebraic relationship should hold more than randomly expected.");
}
