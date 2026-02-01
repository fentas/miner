//! Research Experiment v5: Near-Collision and Partial Match Analysis
//!
//! Tests whether "near misses" (partial matches) can guide the search.
//! If hashes that almost meet the target cluster around certain nonces,
//! we could focus our search more efficiently.
//!
//! Correlates to: docs/RESEARCH-test-log-v5.md
//! Run with: cargo test --release --test research_v5_test -- --nocapture

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

/// Count leading zero bits
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

/// Test 1: Near-Miss Neighbor Analysis
/// When we find a "good" hash (many leading zeros), are neighbors also good?
#[test]
fn test_research_v5_near_miss_neighbors() {
    println!("\n=== Research v5: Near-Miss Neighbor Analysis ===\n");
    println!("Question: If hash(N) has many zeros, do hash(N±k) also have more zeros?\n");

    let samples = 2_000_000u64;
    let threshold = 8u32; // 8+ leading zero bits = "good" hash

    // Find all "good" hashes and record their neighbors
    let mut good_neighbor_lz: Vec<u32> = Vec::new();  // LZ of neighbors of good hashes
    let mut random_lz: Vec<u32> = Vec::new();          // LZ of random hashes

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = leading_zero_bits(&hash);

        // Record for baseline
        random_lz.push(lz);

        if lz >= threshold {
            // Found a good hash, check its neighbors
            for offset in [-10i32, -5, -2, -1, 1, 2, 5, 10] {
                let neighbor_nonce = nonce.wrapping_add(offset as u32);
                let neighbor_header = header_with_nonce(sample, neighbor_nonce);
                let neighbor_hash = sha256d(&neighbor_header);
                let neighbor_lz = leading_zero_bits(&neighbor_hash);
                good_neighbor_lz.push(neighbor_lz);
            }
        }
    }

    // Calculate statistics
    let random_mean = random_lz.iter().map(|&x| x as f64).sum::<f64>() / random_lz.len() as f64;
    let neighbor_mean = if !good_neighbor_lz.is_empty() {
        good_neighbor_lz.iter().map(|&x| x as f64).sum::<f64>() / good_neighbor_lz.len() as f64
    } else {
        0.0
    };

    // Count how many neighbors also meet threshold
    let neighbor_good_count = good_neighbor_lz.iter().filter(|&&lz| lz >= threshold).count();
    let expected_good_rate = 1.0 / (2.0_f64.powi(threshold as i32));

    println!("| Metric | Value |");
    println!("|--------|-------|");
    println!("| Total samples | {} |", samples);
    println!("| Good hashes found (≥{} zero bits) | {} |", threshold, random_lz.iter().filter(|&&lz| lz >= threshold).count());
    println!("| Neighbors analyzed | {} |", good_neighbor_lz.len());
    println!("| Random baseline mean LZ | {:.4} |", random_mean);
    println!("| Good hash neighbor mean LZ | {:.4} |", neighbor_mean);
    println!("| Difference | {:+.4} |", neighbor_mean - random_mean);
    println!();
    println!("| Neighbors also meeting threshold | {} |", neighbor_good_count);
    println!("| Observed rate | {:.4}% |", neighbor_good_count as f64 / good_neighbor_lz.len() as f64 * 100.0);
    println!("| Expected rate (random) | {:.4}% |", expected_good_rate * 100.0);

    println!("\nIf near-misses clustered, neighbor mean would be higher than random.");
}

/// Test 2: Progressive Zero Analysis
/// Do hashes with N zeros have correlated neighbors with N-1 zeros?
#[test]
fn test_research_v5_progressive_zeros() {
    println!("\n=== Research v5: Progressive Zero Analysis ===\n");
    println!("Question: If hash has N zeros, is hash(nonce+1) more likely to have N-1 zeros?\n");

    let samples = 5_000_000u64;

    // Track: for each LZ count, what's the distribution of next nonce's LZ?
    let mut next_lz_given_current: HashMap<u32, Vec<u32>> = HashMap::new();

    for sample in 0..samples {
        let nonce = sample as u32;
        let header1 = header_with_nonce(sample, nonce);
        let hash1 = sha256d(&header1);
        let lz1 = leading_zero_bits(&hash1);

        let header2 = header_with_nonce(sample, nonce + 1);
        let hash2 = sha256d(&header2);
        let lz2 = leading_zero_bits(&hash2);

        next_lz_given_current.entry(lz1).or_default().push(lz2);
    }

    println!("| Current LZ | Samples | Mean Next LZ | Expected | Diff |");
    println!("|------------|---------|--------------|----------|------|");

    // Expected mean is always ~1.0 (geometric distribution with p=0.5)
    let expected_mean = 1.0_f64;

    for lz in 0..=12 {
        if let Some(next_lzs) = next_lz_given_current.get(&lz) {
            if next_lzs.len() > 1000 {
                let mean = next_lzs.iter().map(|&x| x as f64).sum::<f64>() / next_lzs.len() as f64;
                let diff = mean - expected_mean;
                println!("| {:>10} | {:>7} | {:>12.4} | {:>8.2} | {:>+5.4} |",
                    lz, next_lzs.len(), mean, expected_mean, diff);
            }
        }
    }

    println!("\nIf zeros were progressive, higher current LZ would mean higher next LZ.");
}

/// Test 3: Partial Hash Match Analysis
/// If first N bits match target, does that predict remaining bits?
#[test]
fn test_research_v5_partial_match_prediction() {
    println!("\n=== Research v5: Partial Match Prediction ===\n");
    println!("Question: If first byte is 0x00, are remaining bytes more likely to be low?\n");

    let samples = 5_000_000u64;

    // Group hashes by first byte value
    let mut second_byte_by_first: HashMap<u8, Vec<u8>> = HashMap::new();

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);

        second_byte_by_first.entry(hash[0]).or_default().push(hash[1]);
    }

    println!("| First Byte | Samples | Mean Second Byte | Expected |");
    println!("|------------|---------|------------------|----------|");

    // Expected mean for any byte is 127.5
    let expected_mean = 127.5_f64;

    for first_byte in [0x00, 0x01, 0x10, 0x80, 0xFF] {
        if let Some(second_bytes) = second_byte_by_first.get(&first_byte) {
            let mean = second_bytes.iter().map(|&x| x as f64).sum::<f64>() / second_bytes.len() as f64;
            println!("| 0x{:02X}       | {:>7} | {:>16.2} | {:>8.1} |",
                first_byte, second_bytes.len(), mean, expected_mean);
        }
    }

    println!("\nIf first byte predicted second, 0x00 would have lower mean second byte.");
}

/// Test 4: Nonce Distance from Good Hash
/// After finding a good hash, how far until the next good hash?
#[test]
fn test_research_v5_good_hash_spacing() {
    println!("\n=== Research v5: Good Hash Spacing Analysis ===\n");
    println!("Question: Are good hashes evenly spaced or do they cluster?\n");

    let samples = 10_000_000u64;
    let threshold = 8u32;

    let mut good_nonces: Vec<u32> = Vec::new();

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = leading_zero_bits(&hash);

        if lz >= threshold {
            good_nonces.push(nonce);
        }
    }

    if good_nonces.len() < 10 {
        println!("Not enough good hashes found for analysis.");
        return;
    }

    // Calculate gaps between consecutive good hashes
    let mut gaps: Vec<u32> = Vec::new();
    for i in 1..good_nonces.len() {
        gaps.push(good_nonces[i] - good_nonces[i - 1]);
    }

    gaps.sort();

    let mean_gap = gaps.iter().map(|&x| x as f64).sum::<f64>() / gaps.len() as f64;
    let expected_gap = 2.0_f64.powi(threshold as i32);

    // Calculate variance
    let variance = gaps.iter().map(|&x| (x as f64 - mean_gap).powi(2)).sum::<f64>() / gaps.len() as f64;
    let std_dev = variance.sqrt();

    // For a geometric distribution, variance = (1-p)/p^2, std_dev ≈ mean
    let expected_std_dev = expected_gap; // Approximately

    println!("| Metric | Observed | Expected (Geometric) |");
    println!("|--------|----------|---------------------|");
    println!("| Good hashes found | {} | ~{:.0} |", good_nonces.len(), samples as f64 / expected_gap);
    println!("| Mean gap | {:.1} | {:.1} |", mean_gap, expected_gap);
    println!("| Std deviation | {:.1} | ~{:.1} |", std_dev, expected_std_dev);
    println!("| Min gap | {} | 1 |", gaps.first().unwrap_or(&0));
    println!("| Max gap | {} | varies |", gaps.last().unwrap_or(&0));
    println!("| Median gap | {} | ~{:.0} |", gaps[gaps.len() / 2], expected_gap * 0.693); // ln(2) * mean

    // Check if gaps follow geometric distribution
    // If clustering existed, we'd see many small gaps
    let small_gaps = gaps.iter().filter(|&&g| g < expected_gap as u32 / 4).count();
    let small_gap_rate = small_gaps as f64 / gaps.len() as f64;

    println!();
    println!("| Small gaps (< expected/4) | {} ({:.1}%) |", small_gaps, small_gap_rate * 100.0);
    println!("| Expected rate | ~22% |"); // P(X < mean/4) for geometric ≈ 22%

    println!("\nIf good hashes clustered, we'd see more small gaps than expected.");
}

/// Test 5: Multi-Target Analysis
/// For different difficulty levels, do "almost" matches provide information?
#[test]
fn test_research_v5_multi_target() {
    println!("\n=== Research v5: Multi-Target Analysis ===\n");
    println!("Question: If hash almost meets hard target, does it help find easier target?\n");

    let samples = 2_000_000u64;

    // Find hashes with 6 leading zero bits (hard target)
    // Check if they're more likely to also have 4 leading zero bits (easy target) in neighbors

    let hard_threshold = 6u32;
    let easy_threshold = 4u32;

    let mut near_hard_neighbor_easy_rate = 0u64;
    let mut near_hard_neighbor_count = 0u64;
    let mut random_easy_count = 0u64;

    for sample in 0..samples {
        let nonce = sample as u32;
        let header = header_with_nonce(sample, nonce);
        let hash = sha256d(&header);
        let lz = leading_zero_bits(&hash);

        // Track random baseline for easy target
        if lz >= easy_threshold {
            random_easy_count += 1;
        }

        // If this is a "hard" hash
        if lz >= hard_threshold {
            // Check neighbors for easy target
            for offset in 1..=10 {
                let neighbor_nonce = nonce.wrapping_add(offset);
                let neighbor_header = header_with_nonce(sample, neighbor_nonce);
                let neighbor_hash = sha256d(&neighbor_header);
                let neighbor_lz = leading_zero_bits(&neighbor_hash);

                near_hard_neighbor_count += 1;
                if neighbor_lz >= easy_threshold {
                    near_hard_neighbor_easy_rate += 1;
                }
            }
        }
    }

    let random_easy_rate = random_easy_count as f64 / samples as f64;
    let neighbor_easy_rate = if near_hard_neighbor_count > 0 {
        near_hard_neighbor_easy_rate as f64 / near_hard_neighbor_count as f64
    } else {
        0.0
    };

    println!("| Metric | Rate |");
    println!("|--------|------|");
    println!("| Random hash meets easy target (≥{} zeros) | {:.4}% |", easy_threshold, random_easy_rate * 100.0);
    println!("| Expected (1/2^{}) | {:.4}% |", easy_threshold, 100.0 / 2.0_f64.powi(easy_threshold as i32));
    println!("| Neighbor of hard hash meets easy target | {:.4}% |", neighbor_easy_rate * 100.0);
    println!("| Difference | {:+.4}% |", (neighbor_easy_rate - random_easy_rate) * 100.0);

    println!("\nIf near-misses helped, neighbor rate would be higher than random.");
}

/// Test 6: Hash Gradient Analysis
/// When moving from high to low hash value, is there a "gradient"?
#[test]
fn test_research_v5_hash_gradient() {
    println!("\n=== Research v5: Hash Gradient Analysis ===\n");
    println!("Question: Can we detect a 'gradient' pointing toward low hashes?\n");

    let samples = 500_000u64;

    // For each nonce, compute gradient: hash(nonce+1) - hash(nonce)
    // If gradient pointed toward zeros, we could follow it

    let mut gradient_helps = 0u64;  // Cases where gradient points toward lower hash
    let mut gradient_hurts = 0u64;  // Cases where gradient points away

    for sample in 0..samples {
        let nonce = sample as u32;

        let h0 = sha256d(&header_with_nonce(sample, nonce.wrapping_sub(1)));
        let h1 = sha256d(&header_with_nonce(sample, nonce));
        let h2 = sha256d(&header_with_nonce(sample, nonce.wrapping_add(1)));

        let lz0 = leading_zero_bits(&h0);
        let lz1 = leading_zero_bits(&h1);
        let lz2 = leading_zero_bits(&h2);

        // Check if "gradient" (lz1 - lz0) predicts improvement (lz2 > lz1)
        let gradient = lz1 as i32 - lz0 as i32;
        let improvement = lz2 as i32 - lz1 as i32;

        if gradient > 0 && improvement > 0 {
            gradient_helps += 1;  // Gradient pointed right, we improved
        } else if gradient > 0 && improvement < 0 {
            gradient_hurts += 1;  // Gradient pointed right, we got worse
        }
    }

    let total_positive_gradient = gradient_helps + gradient_hurts;
    let help_rate = if total_positive_gradient > 0 {
        gradient_helps as f64 / total_positive_gradient as f64
    } else {
        0.0
    };

    println!("| Metric | Count | Rate |");
    println!("|--------|-------|------|");
    println!("| Positive gradient, continued improving | {} | {:.2}% |", gradient_helps, help_rate * 100.0);
    println!("| Positive gradient, got worse | {} | {:.2}% |", gradient_hurts, (1.0 - help_rate) * 100.0);
    println!("| Expected (random) | 50%/50% | |");

    println!("\nIf gradient existed, 'continued improving' would be > 50%.");
}
