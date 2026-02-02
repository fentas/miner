//! Research v13: SHA-256 Pattern Analysis - "Attractor" and "Logical Track" Theory
//!
//! Investigating whether double SHA-256 creates exploitable patterns:
//! 1. IV bit distribution analysis
//! 2. K constant patterns
//! 3. Midstate clustering
//! 4. Output "attractor" hypothesis
//! 5. Leading zero distribution (mining relevance)
//!
//! Run: cargo test --release --test research_v13_test -- --nocapture --ignored

use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// SHA-256 Initial Values (derived from sqrt of first 8 primes)
const H: [u32; 8] = [
    0x6a09e667, // frac(√2)
    0xbb67ae85, // frac(√3)
    0x3c6ef372, // frac(√5)
    0xa54ff53a, // frac(√7)
    0x510e527f, // frac(√11)
    0x9b05688c, // frac(√13)
    0x1f83d9ab, // frac(√17)
    0x5be0cd19, // frac(√19)
];

/// SHA-256 Round Constants (derived from cbrt of first 64 primes)
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn hamming_weight(x: u32) -> u32 {
    x.count_ones()
}

fn leading_zeros(hash: &[u8; 32]) -> u32 {
    let mut zeros = 0u32;
    for byte in hash.iter() {
        if *byte == 0 {
            zeros += 8;
        } else {
            zeros += byte.leading_zeros();
            break;
        }
    }
    zeros
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn sha256d(data: &[u8]) -> [u8; 32] {
    sha256(&sha256(data))
}

/// Test 1: Analyze IV bit distributions
#[test]
#[ignore]
fn test_iv_bit_distribution() {
    println!("\n=== Test 1: IV Bit Distribution ===\n");

    let mut total_ones = 0u32;
    let mut total_bits = 0u32;

    println!("| Register | Hex        | Hamming | Bit 31 | Bit 0 |");
    println!("|----------|------------|---------|--------|-------|");

    for (i, &h) in H.iter().enumerate() {
        let hw = hamming_weight(h);
        let msb = (h >> 31) & 1;
        let lsb = h & 1;
        total_ones += hw;
        total_bits += 32;

        println!("| h{}       | 0x{:08x} | {:2}/32   | {}      | {}     |",
            i, h, hw, msb, lsb);
    }

    let ratio = (total_ones as f64) / (total_bits as f64) * 100.0;
    println!("\nTotal: {}/{} bits set ({:.2}%)", total_ones, total_bits, ratio);
    println!("Expected (random): 50.00%");
    println!("Deviation: {:.2}%", ratio - 50.0);

    // Check bit position bias
    println!("\n--- Bit Position Analysis ---");
    let mut position_counts = [0u32; 32];
    for &h in H.iter() {
        for bit in 0..32 {
            if (h >> bit) & 1 == 1 {
                position_counts[bit] += 1;
            }
        }
    }

    println!("Bits with all 8 IVs set: {:?}",
        position_counts.iter().enumerate()
            .filter(|(_, &c)| c == 8)
            .map(|(i, _)| i)
            .collect::<Vec<_>>());

    println!("Bits with 0 IVs set: {:?}",
        position_counts.iter().enumerate()
            .filter(|(_, &c)| c == 0)
            .map(|(i, _)| i)
            .collect::<Vec<_>>());
}

/// Test 2: Analyze K constant patterns
#[test]
#[ignore]
fn test_k_constant_patterns() {
    println!("\n=== Test 2: K Constant Patterns ===\n");

    let mut total_ones = 0u32;
    let mut total_bits = 0u32;
    let mut hw_distribution: HashMap<u32, u32> = HashMap::new();

    for &k in K.iter() {
        let hw = hamming_weight(k);
        total_ones += hw;
        total_bits += 32;
        *hw_distribution.entry(hw).or_insert(0) += 1;
    }

    let ratio = (total_ones as f64) / (total_bits as f64) * 100.0;
    println!("K constants: {}/{} bits set ({:.2}%)", total_ones, total_bits, ratio);

    println!("\nHamming weight distribution:");
    let mut sorted: Vec<_> = hw_distribution.iter().collect();
    sorted.sort_by_key(|&(k, _)| k);
    for (hw, count) in sorted {
        println!("  HW={}: {} constants", hw, count);
    }

    // Check for sequential patterns
    println!("\n--- K Constant Differences ---");
    let mut diffs: Vec<u32> = Vec::new();
    for i in 1..K.len() {
        diffs.push(K[i].wrapping_sub(K[i-1]));
    }

    // Check if any differences repeat
    let mut diff_counts: HashMap<u32, u32> = HashMap::new();
    for d in &diffs {
        *diff_counts.entry(*d).or_insert(0) += 1;
    }

    let repeated: Vec<_> = diff_counts.iter().filter(|(_, &c)| c > 1).collect();
    if repeated.is_empty() {
        println!("No repeated differences (good - appears random)");
    } else {
        println!("Repeated differences found: {:?}", repeated);
    }
}

/// Test 3: Midstate clustering analysis
#[test]
#[ignore]
fn test_midstate_clustering() {
    println!("\n=== Test 3: Midstate Clustering ===\n");

    // Simulate block headers with varying data
    // Bitcoin block header: version(4) + prev_hash(32) + merkle(32) + time(4) + bits(4) + nonce(4) = 80 bytes
    // Midstate processes first 64 bytes

    let samples = 10000;
    let mut midstate_first_bytes: HashMap<u8, u32> = HashMap::new();
    let mut midstate_hamming: HashMap<u32, u32> = HashMap::new();

    println!("Sampling {} simulated block headers...", samples);

    for i in 0..samples {
        // Create pseudo-random 64-byte block header prefix
        let mut header = [0u8; 64];

        // Version (little-endian, usually 0x20000000 range)
        header[0..4].copy_from_slice(&(0x20000000u32 + (i as u32 % 16)).to_le_bytes());

        // Prev hash (varies per block)
        for j in 4..36 {
            header[j] = ((i * 17 + j * 13) % 256) as u8;
        }

        // Merkle root (varies per block)
        for j in 36..64 {
            header[j] = ((i * 23 + j * 7) % 256) as u8;
        }

        let midstate = sha256(&header);

        // Track first byte distribution
        *midstate_first_bytes.entry(midstate[0]).or_insert(0) += 1;

        // Track hamming weight of full midstate
        let hw: u32 = midstate.iter().map(|b| b.count_ones()).sum();
        *midstate_hamming.entry(hw).or_insert(0) += 1;
    }

    // Analyze first byte distribution
    println!("\n--- First Byte Distribution ---");
    let expected = samples as f64 / 256.0;
    let mut max_deviation = 0.0f64;
    let mut most_common = (0u8, 0u32);

    for (&byte, &count) in &midstate_first_bytes {
        let deviation = ((count as f64 - expected) / expected * 100.0).abs();
        if deviation > max_deviation {
            max_deviation = deviation;
        }
        if count > most_common.1 {
            most_common = (byte, count);
        }
    }

    println!("Expected per byte: {:.1}", expected);
    println!("Max deviation: {:.2}%", max_deviation);
    println!("Most common first byte: 0x{:02x} ({} times)", most_common.0, most_common.1);
    println!("Unique first bytes seen: {}/256", midstate_first_bytes.len());

    // Analyze hamming weight distribution
    println!("\n--- Midstate Hamming Weight ---");
    let hw_values: Vec<u32> = midstate_hamming.keys().cloned().collect();
    let min_hw = *hw_values.iter().min().unwrap();
    let max_hw = *hw_values.iter().max().unwrap();
    let avg_hw: f64 = midstate_hamming.iter()
        .map(|(&hw, &count)| hw as f64 * count as f64)
        .sum::<f64>() / samples as f64;

    println!("Hamming weight range: {} - {} (expected ~128)", min_hw, max_hw);
    println!("Average hamming weight: {:.2} (expected 128.0)", avg_hw);
    println!("Deviation from expected: {:.2}%", (avg_hw - 128.0) / 128.0 * 100.0);
}

/// Test 4: Attractor hypothesis - do outputs cluster?
#[test]
#[ignore]
fn test_attractor_hypothesis() {
    println!("\n=== Test 4: Attractor Hypothesis ===\n");

    let samples = 100000;

    // Track first 2 bytes of output (65536 buckets)
    let mut output_buckets: HashMap<u16, u32> = HashMap::new();

    // Track outputs that are "close" to common values
    let mut near_zero_count = 0u32;  // first byte < 16
    let mut near_ff_count = 0u32;    // first byte > 240

    println!("Testing {} random inputs with SHA-256² ...", samples);

    for i in 0..samples {
        let input = format!("test_input_{}", i);
        let hash = sha256d(input.as_bytes());

        let bucket = ((hash[0] as u16) << 8) | (hash[1] as u16);
        *output_buckets.entry(bucket).or_insert(0) += 1;

        if hash[0] < 16 {
            near_zero_count += 1;
        }
        if hash[0] > 240 {
            near_ff_count += 1;
        }
    }

    // Analyze distribution
    let expected_per_bucket = samples as f64 / 65536.0;
    let mut collision_count = 0u32;
    let mut max_bucket_count = 0u32;

    for &count in output_buckets.values() {
        if count > 1 {
            collision_count += count - 1;
        }
        if count > max_bucket_count {
            max_bucket_count = count;
        }
    }

    println!("\n--- Output Distribution ---");
    println!("Unique first-2-byte values: {}/65536", output_buckets.len());
    println!("Expected per bucket: {:.4}", expected_per_bucket);
    println!("Max bucket count: {} (expected ~{:.0})", max_bucket_count, expected_per_bucket + 3.0);
    println!("Collisions: {} ({:.3}%)", collision_count, collision_count as f64 / samples as f64 * 100.0);

    println!("\n--- Near-Boundary Analysis ---");
    let expected_near = samples as f64 * 16.0 / 256.0;
    println!("Near-zero outputs (byte[0] < 0x10): {} (expected {:.0})", near_zero_count, expected_near);
    println!("Near-0xFF outputs (byte[0] > 0xF0): {} (expected {:.0})", near_ff_count, expected_near);

    let zero_deviation = ((near_zero_count as f64 - expected_near) / expected_near * 100.0).abs();
    let ff_deviation = ((near_ff_count as f64 - expected_near) / expected_near * 100.0).abs();
    println!("Deviation from expected: {:.2}% / {:.2}%", zero_deviation, ff_deviation);

    if zero_deviation > 10.0 || ff_deviation > 10.0 {
        println!("\n⚠️  POTENTIAL BIAS DETECTED!");
    } else {
        println!("\n✓ No significant attractor bias detected");
    }
}

/// Test 5: Leading zeros distribution (mining-relevant)
#[test]
#[ignore]
fn test_leading_zeros_distribution() {
    println!("\n=== Test 5: Leading Zeros Distribution ===\n");

    let samples = 1_000_000;
    let mut lz_counts: HashMap<u32, u32> = HashMap::new();

    println!("Testing {} hashes for leading zero distribution...", samples);

    for i in 0u64..samples {
        let input = i.to_le_bytes();
        let hash = sha256d(&input);
        let lz = leading_zeros(&hash);
        *lz_counts.entry(lz).or_insert(0) += 1;
    }

    println!("\n| Leading Zeros | Count    | Expected | Ratio  |");
    println!("|---------------|----------|----------|--------|");

    for lz in 0..=24 {
        let count = *lz_counts.get(&lz).unwrap_or(&0);
        // P(exactly n leading zeros) = (1/2)^n * (1/2) for n < 256
        // P(at least n) = (1/2)^n
        // So P(exactly n) ≈ samples / 2^(n+1)
        let expected = samples as f64 / (1u64 << (lz + 1)) as f64;
        let ratio = if expected > 0.5 { count as f64 / expected } else { 0.0 };

        if count > 0 || expected >= 1.0 {
            println!("|      {:2}       | {:8} | {:8.1} | {:.4}  |",
                lz, count, expected, ratio);
        }
    }

    // Mining-relevant: how many hashes to find n leading zeros?
    println!("\n--- Mining Difficulty Analysis ---");
    for target_lz in [8, 16, 24, 32, 40, 48, 56, 64u32] {
        let expected_hashes = 1u64 << target_lz;
        println!("{} leading zeros: ~{} hashes ({:.2e})",
            target_lz, expected_hashes, expected_hashes as f64);
    }
}

/// Test 6: Single vs Double SHA-256 comparison
#[test]
#[ignore]
fn test_single_vs_double_comparison() {
    println!("\n=== Test 6: Single vs Double SHA-256 ===\n");

    let samples = 100000;

    // Compare distributions
    let mut single_hw_sum = 0u64;
    let mut double_hw_sum = 0u64;
    let mut single_lz_sum = 0u64;
    let mut double_lz_sum = 0u64;

    println!("Comparing {} samples...", samples);

    for i in 0..samples {
        let input = format!("comparison_test_{}", i);

        let single = sha256(input.as_bytes());
        let double = sha256(&single);

        let single_hw: u32 = single.iter().map(|b| b.count_ones()).sum();
        let double_hw: u32 = double.iter().map(|b| b.count_ones()).sum();

        single_hw_sum += single_hw as u64;
        double_hw_sum += double_hw as u64;

        single_lz_sum += leading_zeros(&single) as u64;
        double_lz_sum += leading_zeros(&double) as u64;
    }

    println!("\n--- Hamming Weight Comparison ---");
    println!("Single SHA-256 avg HW: {:.4}", single_hw_sum as f64 / samples as f64);
    println!("Double SHA-256 avg HW: {:.4}", double_hw_sum as f64 / samples as f64);
    println!("Expected (random): 128.0000");

    println!("\n--- Leading Zeros Comparison ---");
    println!("Single SHA-256 avg LZ: {:.4}", single_lz_sum as f64 / samples as f64);
    println!("Double SHA-256 avg LZ: {:.4}", double_lz_sum as f64 / samples as f64);
    println!("Expected (geometric): 1.0000");

    // Check if double hashing "amplifies" any bias
    let single_hw_deviation = ((single_hw_sum as f64 / samples as f64) - 128.0).abs();
    let double_hw_deviation = ((double_hw_sum as f64 / samples as f64) - 128.0).abs();

    if double_hw_deviation > single_hw_deviation * 1.5 {
        println!("\n⚠️  Double hashing may amplify bias!");
    } else {
        println!("\n✓ No amplification detected in double hashing");
    }
}

/// Test 7: Fixed point search (H(x) = x)
#[test]
#[ignore]
fn test_fixed_point_search() {
    println!("\n=== Test 7: Fixed Point Search ===\n");

    // Obviously we won't find a true fixed point, but we can look for
    // outputs that are "close" to inputs (low hamming distance)

    let samples = 100000;
    let mut min_distance = 256u32;
    let mut closest_input = String::new();

    println!("Searching for near-fixed-points in {} samples...", samples);

    for i in 0..samples {
        // Use 32-byte inputs to match output size
        let mut input = [0u8; 32];
        for j in 0..32 {
            input[j] = ((i * 17 + j * 13) % 256) as u8;
        }

        let output = sha256d(&input);

        // Calculate hamming distance between input and output
        let distance: u32 = input.iter().zip(output.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        if distance < min_distance {
            min_distance = distance;
            closest_input = format!("{:?}", &input[0..8]);
        }
    }

    println!("Minimum hamming distance found: {}/256 bits", min_distance);
    println!("Expected minimum (random): ~100-110 bits");
    println!("Closest input prefix: {}", closest_input);

    if min_distance < 90 {
        println!("\n⚠️  Unusually close match found!");
    } else {
        println!("\n✓ No near-fixed-points detected (as expected)");
    }
}

/// Run all pattern analysis tests
#[test]
#[ignore]
fn test_all_v13_patterns() {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Research v13: SHA-256 Pattern Analysis                       ║");
    println!("║  Testing 'Attractor' and 'Logical Track' Theories             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    test_iv_bit_distribution();
    test_k_constant_patterns();
    test_midstate_clustering();
    test_attractor_hypothesis();
    test_leading_zeros_distribution();
    test_single_vs_double_comparison();
    test_fixed_point_search();

    println!("\n");
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    PATTERN ANALYSIS COMPLETE                   ");
    println!("═══════════════════════════════════════════════════════════════");
}
