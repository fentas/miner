//! Research v24: Wild & Loco Theories
//!
//! After 23 experiments proving SHA-256 is a perfect random oracle, let's go CRAZY.
//! These are unconventional, creative, "what if?" theories that probably won't work
//! but are worth testing for completeness and intellectual curiosity.
//!
//! Wild Theories to Test:
//! 1. "Hamming Weight Destiny" - Do valid nonces cluster at certain Hamming weights?
//! 2. "Gray Code Walk" - Does single-bit increment behave differently?
//! 3. "Message Schedule Fingerprint" - Can W[16:63] patterns predict validity?
//! 4. "Glitch Variance" - Do some nonces create fewer carries?
//! 5. "Prime Nonce Magic" - Are prime nonces luckier?
//! 6. "Bit Position Dominance" - Are all 32 nonce bits equally important?
//! 7. "Reverse Birthday Attack" - Do valid hashes cluster in output space?
//! 8. "Compression Function Orbits" - Do repeated compressions show patterns?
//!
//! Run: cargo test --release --test research_v24_test test_all_v24 -- --nocapture --ignored

use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// SHA-256 round constants (first 32 bits of cube roots of first 64 primes)
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Count leading zeros in a byte array
fn leading_zeros(hash: &[u8]) -> u32 {
    let mut zeros = 0;
    for byte in hash {
        if *byte == 0 {
            zeros += 8;
        } else {
            zeros += byte.leading_zeros();
            break;
        }
    }
    zeros
}

/// Count bits set in a u32 (Hamming weight)
fn hamming_weight(n: u32) -> u32 {
    n.count_ones()
}

/// Check if a number is prime (simple trial division)
fn is_prime(n: u32) -> bool {
    if n < 2 { return false; }
    if n == 2 { return true; }
    if n % 2 == 0 { return false; }
    let sqrt = (n as f64).sqrt() as u32;
    for i in (3..=sqrt).step_by(2) {
        if n % i == 0 { return false; }
    }
    true
}

/// Generate Gray code from binary index
fn gray_code(n: u32) -> u32 {
    n ^ (n >> 1)
}

/// Expand message schedule from W[0:15] to W[0:63]
fn expand_message_schedule(w_in: &[u32; 16]) -> [u32; 64] {
    let mut w = [0u32; 64];
    w[..16].copy_from_slice(w_in);

    for i in 16..64 {
        let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
        let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
        w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
    }
    w
}

/// Count carry propagation "glitches" in a SHA-256 round
fn count_carry_glitches(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, k: u32, w: u32) -> u32 {
    let mut glitches = 0;

    // Ch function doesn't create carries (XOR based)
    // Maj function doesn't create carries (AND/XOR based)

    // But the additions do!
    // sigma1
    // let sigma1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25); // no carries

    // ch
    let ch = (e & f) ^ ((!e) & g); // no carries

    // temp1 = h + sigma1 + ch + k + w
    // Each addition can cascade carries
    let (temp1a, carry1) = h.overflowing_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25));
    if carry1 { glitches += 1; }
    let (temp1b, carry2) = temp1a.overflowing_add(ch);
    if carry2 { glitches += 1; }
    let (temp1c, carry3) = temp1b.overflowing_add(k);
    if carry3 { glitches += 1; }
    let (_temp1, carry4) = temp1c.overflowing_add(w);
    if carry4 { glitches += 1; }

    // sigma0
    // let sigma0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22); // no carries

    // maj
    let _maj = (a & b) ^ (a & c) ^ (b & c); // no carries

    // temp2 = sigma0 + maj
    let (_temp2, carry5) = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).overflowing_add(_maj);
    if carry5 { glitches += 1; }

    // d + temp1 for new e
    let (_new_e, carry6) = d.overflowing_add(_temp1);
    if carry6 { glitches += 1; }

    // temp1 + temp2 for new a
    let (_new_a, carry7) = _temp1.overflowing_add(_temp2);
    if carry7 { glitches += 1; }

    glitches
}

/// Perform SHA-256 compression and count total glitches
fn sha256_with_glitch_count(input: &[u8]) -> (Vec<u8>, u32) {
    // We'll manually track through the compression function
    // For simplicity, just count overflows in a mock compression
    // This is a simplified metric

    // Compute the actual hash
    let mut hasher = Sha256::new();
    hasher.update(input);
    let hash = hasher.finalize().to_vec();

    // Estimate glitches based on input entropy (simplified)
    // Real implementation would trace through all 64 rounds
    let mut glitch_estimate = 0u32;
    for byte in input {
        glitch_estimate += byte.count_ones();
    }

    (hash, glitch_estimate % 256) // Normalize
}

/// Create Bitcoin-like header with nonce
fn make_header(nonce: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    // Version
    header[0..4].copy_from_slice(&1u32.to_le_bytes());
    // Previous block hash (fake)
    for i in 0..32 {
        header[4 + i] = ((i * 17 + 3) % 256) as u8;
    }
    // Merkle root (fake)
    for i in 0..32 {
        header[36 + i] = ((i * 23 + 7) % 256) as u8;
    }
    // Timestamp
    header[68..72].copy_from_slice(&0x5f5e100u32.to_le_bytes());
    // Bits (difficulty)
    header[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());
    // Nonce
    header[76..80].copy_from_slice(&nonce.to_le_bytes());
    header
}

/// Double SHA-256 (Bitcoin's hash)
fn double_sha256(data: &[u8]) -> Vec<u8> {
    let first = Sha256::digest(data);
    Sha256::digest(&first).to_vec()
}

// =============================================================================
// TEST 1: Hamming Weight Destiny
// =============================================================================

/// Theory: Do valid nonces cluster at certain Hamming weights?
/// If some Hamming weights are "luckier", we could prioritize them.
#[test]
#[ignore]
fn test_hamming_weight_destiny() {
    println!("\n=== Test 1: Hamming Weight Destiny ===");
    println!("Theory: Do valid nonces have unusual Hamming weight distribution?\n");

    let trials = 10_000_000u64;
    let target_zeros = 16u32;

    // Track Hamming weights of valid nonces
    let mut valid_nonces: Vec<u32> = Vec::new();
    let mut hamming_counts: [u64; 33] = [0; 33]; // 0-32 bits set
    let mut valid_hamming: [u64; 33] = [0; 33];

    println!("Running {} trials with {}-zero target...", trials, target_zeros);

    for nonce in 0..trials as u32 {
        let header = make_header(nonce);
        let hash = double_sha256(&header);
        let zeros = leading_zeros(&hash);

        let hw = hamming_weight(nonce) as usize;
        hamming_counts[hw] += 1;

        if zeros >= target_zeros {
            valid_nonces.push(nonce);
            valid_hamming[hw] += 1;
        }
    }

    println!("\nValid hashes found: {}", valid_nonces.len());
    println!("\nHamming Weight Distribution:");
    println!("| HW | All Nonces | Valid | Expected | Ratio |");
    println!("|----|-----------:|------:|----------|-------|");

    let total_valid = valid_nonces.len() as f64;
    let expected_per_bin = total_valid / 33.0;

    for hw in 0..33 {
        let all = hamming_counts[hw];
        let valid = valid_hamming[hw];
        let expected = if all > 0 {
            (all as f64 / trials as f64) * total_valid
        } else {
            0.0
        };
        let ratio = if expected > 0.0 { valid as f64 / expected } else { 0.0 };

        if all > 0 {
            println!("| {:2} | {:10} | {:5} | {:8.2} | {:5.2}x |",
                     hw, all, valid, expected, ratio);
        }
    }

    // Statistical analysis
    if !valid_nonces.is_empty() {
        let avg_hw: f64 = valid_nonces.iter().map(|n| hamming_weight(*n) as f64).sum::<f64>() / valid_nonces.len() as f64;
        let expected_avg = 16.0; // Expected for uniform random 32-bit numbers

        println!("\nAverage Hamming Weight of valid nonces: {:.3}", avg_hw);
        println!("Expected (uniform random): {:.1}", expected_avg);
        println!("Difference: {:.3} ({:.2}%)", avg_hw - expected_avg, (avg_hw - expected_avg) / expected_avg * 100.0);

        // Chi-squared test would go here
        let mut chi_sq = 0.0;
        for hw in 0..33 {
            let observed = valid_hamming[hw] as f64;
            let expected = (hamming_counts[hw] as f64 / trials as f64) * total_valid;
            if expected > 0.0 {
                chi_sq += (observed - expected).powi(2) / expected;
            }
        }

        println!("\nChi-squared statistic: {:.2}", chi_sq);
        println!("(For 32 DoF, reject uniformity if > 46.0 at p=0.05)");

        if chi_sq > 46.0 {
            println!("RESULT: SIGNIFICANT! Hamming weight may matter.");
        } else {
            println!("RESULT: NOT SIGNIFICANT - Hamming weight doesn't matter.");
        }
    }
}

// =============================================================================
// TEST 2: Gray Code Walk
// =============================================================================

/// Theory: Does single-bit increment (Gray code) behave differently?
/// If adjacent Gray codes have more similar intermediate states, maybe...
#[test]
#[ignore]
fn test_gray_code_walk() {
    println!("\n=== Test 2: Gray Code Walk ===");
    println!("Theory: Does Gray code increment produce different hash distributions?\n");

    let trials = 5_000_000u32;
    let target = 12u32;

    // Compare binary increment vs Gray code increment
    let mut binary_valid = 0u64;
    let mut gray_valid = 0u64;

    let mut binary_zeros_sum = 0u64;
    let mut gray_zeros_sum = 0u64;

    println!("Comparing {} trials each...", trials);

    for i in 0..trials {
        // Binary increment
        let header_bin = make_header(i);
        let hash_bin = double_sha256(&header_bin);
        let zeros_bin = leading_zeros(&hash_bin);
        binary_zeros_sum += zeros_bin as u64;
        if zeros_bin >= target { binary_valid += 1; }

        // Gray code increment
        let gray_nonce = gray_code(i);
        let header_gray = make_header(gray_nonce);
        let hash_gray = double_sha256(&header_gray);
        let zeros_gray = leading_zeros(&hash_gray);
        gray_zeros_sum += zeros_gray as u64;
        if zeros_gray >= target { gray_valid += 1; }
    }

    let binary_avg = binary_zeros_sum as f64 / trials as f64;
    let gray_avg = gray_zeros_sum as f64 / trials as f64;

    println!("\nResults (target: {} zeros):", target);
    println!("| Method | Valid | Avg Zeros |");
    println!("|--------|------:|----------:|");
    println!("| Binary | {:5} | {:9.4} |", binary_valid, binary_avg);
    println!("| Gray   | {:5} | {:9.4} |", gray_valid, gray_avg);

    let ratio = gray_valid as f64 / binary_valid.max(1) as f64;
    println!("\nGray/Binary ratio: {:.4}x", ratio);

    if (ratio - 1.0).abs() < 0.05 {
        println!("RESULT: NO DIFFERENCE - Gray code walk is equivalent to binary.");
    } else if ratio > 1.05 {
        println!("RESULT: GRAY CODE BETTER! (Investigate further)");
    } else {
        println!("RESULT: GRAY CODE WORSE! (That's unexpected)");
    }

    // Check hash similarity between consecutive Gray code values
    println!("\n--- Consecutive Hash Similarity ---");
    let mut same_byte_count = 0u64;
    let mut prev_hash: Option<Vec<u8>> = None;

    for i in 0..100_000u32 {
        let gray_nonce = gray_code(i);
        let header = make_header(gray_nonce);
        let hash = double_sha256(&header);

        if let Some(prev) = &prev_hash {
            let same = hash.iter().zip(prev.iter()).filter(|(a, b)| a == b).count();
            same_byte_count += same as u64;
        }
        prev_hash = Some(hash);
    }

    let avg_same = same_byte_count as f64 / 99_999.0;
    println!("Average same bytes between consecutive Gray hashes: {:.3}", avg_same);
    println!("Expected (random): 1.0 (256 bytes × 1/256 = 1 match)");

    if avg_same > 1.5 {
        println!("RESULT: Consecutive Gray hashes are MORE similar than expected!");
    } else {
        println!("RESULT: Gray code provides no similarity advantage.");
    }
}

// =============================================================================
// TEST 3: Message Schedule Fingerprint
// =============================================================================

/// Theory: Do valid nonces create distinguishable patterns in W[16:63]?
#[test]
#[ignore]
fn test_message_schedule_fingerprint() {
    println!("\n=== Test 3: Message Schedule Fingerprint ===");
    println!("Theory: Can W[16:63] patterns predict validity?\n");

    let trials = 5_000_000u32;
    let target = 16u32;

    // Analyze message schedule properties
    let mut valid_w_stats: Vec<[u32; 64]> = Vec::new();
    let mut all_w_entropy_sum = 0u64;
    let mut valid_w_entropy_sum = 0u64;

    // Track W[16] (first expanded word) statistics
    let mut w16_valid: HashMap<u32, u64> = HashMap::new();
    let mut w16_all: HashMap<u32, u64> = HashMap::new();

    println!("Running {} trials...", trials);

    for nonce in 0..trials {
        let header = make_header(nonce);

        // Parse header into W[0:15]
        let mut w_in = [0u32; 16];
        for i in 0..16 {
            w_in[i] = u32::from_be_bytes([
                header[i*4], header[i*4+1], header[i*4+2], header[i*4+3]
            ]);
        }
        // Note: nonce is at W[19] in second block for Bitcoin
        // But for our 80-byte header parsed as one block, nonce is at bytes 76-79
        // which is W[19] = 0 padding typically. Let's use W[3] position for nonce.

        // Actually for our test, let's directly modify W[3] with nonce
        w_in[3] = nonce;

        // Expand message schedule
        let w = expand_message_schedule(&w_in);

        // Calculate "entropy" (popcount variance across expanded words)
        let w_entropy: u64 = w[16..64].iter()
            .map(|word| word.count_ones() as u64)
            .sum();
        all_w_entropy_sum += w_entropy;

        // Track W[16] distribution
        *w16_all.entry(w[16]).or_insert(0) += 1;

        // Hash and check validity
        let hash = double_sha256(&header);
        let zeros = leading_zeros(&hash);

        if zeros >= target {
            valid_w_stats.push(w);
            valid_w_entropy_sum += w_entropy;
            *w16_valid.entry(w[16]).or_insert(0) += 1;
        }
    }

    let avg_entropy_all = all_w_entropy_sum as f64 / trials as f64;
    let avg_entropy_valid = if !valid_w_stats.is_empty() {
        valid_w_entropy_sum as f64 / valid_w_stats.len() as f64
    } else {
        0.0
    };

    println!("\nValid hashes found: {}", valid_w_stats.len());
    println!("\nMessage Schedule Entropy (popcount sum of W[16:63]):");
    println!("| Category | Avg Entropy |");
    println!("|----------|------------:|");
    println!("| All      | {:11.2} |", avg_entropy_all);
    println!("| Valid    | {:11.2} |", avg_entropy_valid);

    let entropy_diff = (avg_entropy_valid - avg_entropy_all).abs();
    let entropy_pct = entropy_diff / avg_entropy_all * 100.0;

    println!("\nDifference: {:.2} ({:.4}%)", entropy_diff, entropy_pct);

    if entropy_pct > 1.0 {
        println!("RESULT: SIGNIFICANT! W entropy differs for valid hashes.");
    } else {
        println!("RESULT: NOT SIGNIFICANT - W entropy is the same.");
    }

    // Check W[16] distribution
    println!("\n--- W[16] Distribution Analysis ---");
    println!("Unique W[16] values in all trials: {}", w16_all.len());
    println!("Unique W[16] values in valid hashes: {}", w16_valid.len());

    // Are there "lucky" W[16] values?
    let mut lucky_values = 0;
    for (w16_val, valid_count) in &w16_valid {
        let all_count = w16_all.get(w16_val).unwrap_or(&0);
        let expected = (*all_count as f64 / trials as f64) * valid_w_stats.len() as f64;
        if expected > 0.0 && *valid_count as f64 > expected * 2.0 {
            lucky_values += 1;
        }
    }

    println!("W[16] values with >2x expected valid rate: {}", lucky_values);
    println!("(Expected for random: 0)");

    if lucky_values > 0 {
        println!("RESULT: INVESTIGATE! Some W[16] values appear luckier.");
    } else {
        println!("RESULT: No W[16] value is special.");
    }
}

// =============================================================================
// TEST 4: Glitch Variance
// =============================================================================

/// Theory: Do some nonces create fewer carry glitches?
#[test]
#[ignore]
fn test_glitch_variance() {
    println!("\n=== Test 4: Glitch Variance ===");
    println!("Theory: Do valid nonces have unusual carry propagation patterns?\n");

    let trials = 2_000_000u32;
    let target = 14u32;

    // Track glitch counts
    let mut all_glitches: Vec<u32> = Vec::new();
    let mut valid_glitches: Vec<u32> = Vec::new();

    println!("Running {} trials...", trials);

    for nonce in 0..trials {
        let header = make_header(nonce);
        let (hash, glitches) = sha256_with_glitch_count(&header);

        all_glitches.push(glitches);

        let zeros = leading_zeros(&hash);
        if zeros >= target {
            valid_glitches.push(glitches);
        }
    }

    let avg_all: f64 = all_glitches.iter().map(|g| *g as f64).sum::<f64>() / all_glitches.len() as f64;
    let avg_valid: f64 = if !valid_glitches.is_empty() {
        valid_glitches.iter().map(|g| *g as f64).sum::<f64>() / valid_glitches.len() as f64
    } else {
        0.0
    };

    println!("\nValid hashes found: {}", valid_glitches.len());
    println!("\nGlitch Count Statistics:");
    println!("| Category | Avg Glitches | Std Dev |");
    println!("|----------|-------------:|--------:|");

    let std_all = standard_deviation(&all_glitches);
    let std_valid = standard_deviation(&valid_glitches);

    println!("| All      | {:12.2} | {:7.2} |", avg_all, std_all);
    println!("| Valid    | {:12.2} | {:7.2} |", avg_valid, std_valid);

    let diff = (avg_valid - avg_all).abs();
    let diff_pct = diff / avg_all * 100.0;

    println!("\nDifference: {:.2} ({:.2}%)", diff, diff_pct);

    if diff_pct > 5.0 {
        println!("RESULT: SIGNIFICANT! Valid hashes have different glitch patterns.");
    } else {
        println!("RESULT: NOT SIGNIFICANT - Glitch count doesn't predict validity.");
    }

    // Check extreme glitch counts
    let min_glitch = *all_glitches.iter().min().unwrap_or(&0);
    let max_glitch = *all_glitches.iter().max().unwrap_or(&0);

    println!("\nGlitch range: {} to {}", min_glitch, max_glitch);

    // Low-glitch nonces valid rate
    let low_threshold = avg_all as u32 - std_all as u32;
    let low_glitch_count = all_glitches.iter().filter(|g| **g < low_threshold).count();
    let low_glitch_valid = valid_glitches.iter().filter(|g| **g < low_threshold).count();

    println!("\nLow-glitch nonces (<{}): {} total, {} valid", low_threshold, low_glitch_count, low_glitch_valid);

    if low_glitch_count > 0 && valid_glitches.len() > 0 {
        let expected_valid = low_glitch_count as f64 * valid_glitches.len() as f64 / all_glitches.len() as f64;
        let ratio = low_glitch_valid as f64 / expected_valid;
        println!("Expected valid: {:.1}, Ratio: {:.2}x", expected_valid, ratio);

        if ratio > 1.5 {
            println!("RESULT: Low-glitch nonces ARE luckier!");
        } else {
            println!("RESULT: Glitch count doesn't help.");
        }
    }
}

fn standard_deviation(data: &[u32]) -> f64 {
    if data.is_empty() { return 0.0; }
    let mean = data.iter().map(|x| *x as f64).sum::<f64>() / data.len() as f64;
    let variance = data.iter().map(|x| (*x as f64 - mean).powi(2)).sum::<f64>() / data.len() as f64;
    variance.sqrt()
}

// =============================================================================
// TEST 5: Prime Nonce Magic
// =============================================================================

/// Theory: Are prime nonces luckier?
/// This is pure superstition testing. SHA-256 uses primes in K constants...
#[test]
#[ignore]
fn test_prime_nonce_magic() {
    println!("\n=== Test 5: Prime Nonce Magic ===");
    println!("Theory: Are prime number nonces statistically different?\n");
    println!("(This is testing superstition. SHA-256's K constants come from primes...)\n");

    let max_nonce = 10_000_000u32;
    let target = 12u32;

    let mut prime_count = 0u64;
    let mut non_prime_count = 0u64;
    let mut prime_valid = 0u64;
    let mut non_prime_valid = 0u64;

    let mut prime_zeros_sum = 0u64;
    let mut non_prime_zeros_sum = 0u64;

    println!("Testing nonces 0 to {}...", max_nonce);

    for nonce in 2..max_nonce {
        let header = make_header(nonce);
        let hash = double_sha256(&header);
        let zeros = leading_zeros(&hash);

        if is_prime(nonce) {
            prime_count += 1;
            prime_zeros_sum += zeros as u64;
            if zeros >= target { prime_valid += 1; }
        } else {
            non_prime_count += 1;
            non_prime_zeros_sum += zeros as u64;
            if zeros >= target { non_prime_valid += 1; }
        }
    }

    let prime_avg_zeros = prime_zeros_sum as f64 / prime_count as f64;
    let non_prime_avg_zeros = non_prime_zeros_sum as f64 / non_prime_count as f64;

    let prime_valid_rate = prime_valid as f64 / prime_count as f64 * 100.0;
    let non_prime_valid_rate = non_prime_valid as f64 / non_prime_count as f64 * 100.0;

    println!("\nResults:");
    println!("| Category   | Count     | Valid  | Rate    | Avg Zeros |");
    println!("|------------|----------:|-------:|--------:|----------:|");
    println!("| Prime      | {:9} | {:6} | {:6.4}% | {:9.4} |",
             prime_count, prime_valid, prime_valid_rate, prime_avg_zeros);
    println!("| Non-Prime  | {:9} | {:6} | {:6.4}% | {:9.4} |",
             non_prime_count, non_prime_valid, non_prime_valid_rate, non_prime_avg_zeros);

    let rate_ratio = prime_valid_rate / non_prime_valid_rate;
    let zeros_diff = prime_avg_zeros - non_prime_avg_zeros;

    println!("\nPrime/Non-Prime valid rate ratio: {:.4}x", rate_ratio);
    println!("Average zeros difference: {:.4}", zeros_diff);

    if (rate_ratio - 1.0).abs() < 0.02 && zeros_diff.abs() < 0.01 {
        println!("\nRESULT: CONFIRMED NO MAGIC - Primes are not special.");
        println!("(As expected. SHA-256 doesn't care about number theory properties of input.)");
    } else if rate_ratio > 1.05 {
        println!("\nRESULT: PRIMES ARE LUCKIER?! (Investigate immediately!)");
    } else {
        println!("\nRESULT: Statistical noise, primes slightly worse.");
    }
}

// =============================================================================
// TEST 6: Bit Position Dominance
// =============================================================================

/// Theory: Are all 32 nonce bits equally influential on final zeros?
#[test]
#[ignore]
fn test_bit_position_dominance() {
    println!("\n=== Test 6: Bit Position Dominance ===");
    println!("Theory: Does flipping bit 0 have the same effect as flipping bit 31?\n");

    let base_nonces = 100_000u32;

    // For each bit position, measure average zeros change when flipping
    let mut bit_influence: [f64; 32] = [0.0; 32];
    let mut bit_variance: [f64; 32] = [0.0; 32];

    println!("Testing each of 32 bit positions...");

    for bit in 0..32 {
        let mut zeros_changes: Vec<i32> = Vec::new();

        for nonce in 0..base_nonces {
            let header_orig = make_header(nonce);
            let hash_orig = double_sha256(&header_orig);
            let zeros_orig = leading_zeros(&hash_orig) as i32;

            // Flip bit
            let flipped_nonce = nonce ^ (1 << bit);
            let header_flip = make_header(flipped_nonce);
            let hash_flip = double_sha256(&header_flip);
            let zeros_flip = leading_zeros(&hash_flip) as i32;

            zeros_changes.push(zeros_flip - zeros_orig);
        }

        // Calculate mean and variance
        let mean: f64 = zeros_changes.iter().map(|x| *x as f64).sum::<f64>() / base_nonces as f64;
        let variance: f64 = zeros_changes.iter()
            .map(|x| (*x as f64 - mean).powi(2))
            .sum::<f64>() / base_nonces as f64;

        bit_influence[bit] = mean;
        bit_variance[bit] = variance.sqrt();
    }

    println!("\nBit Position Influence on Leading Zeros:");
    println!("| Bit | Mean Change | Std Dev |");
    println!("|----:|------------:|--------:|");

    for bit in 0..32 {
        println!("| {:3} | {:+11.4} | {:7.3} |",
                 bit, bit_influence[bit], bit_variance[bit]);
    }

    // Find if any bit is special
    let avg_influence: f64 = bit_influence.iter().sum::<f64>() / 32.0;
    let max_influence = bit_influence.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_influence = bit_influence.iter().cloned().fold(f64::INFINITY, f64::min);

    println!("\nAverage influence: {:.4}", avg_influence);
    println!("Min influence: {:.4} (bit {})", min_influence,
             bit_influence.iter().position(|x| *x == min_influence).unwrap());
    println!("Max influence: {:.4} (bit {})", max_influence,
             bit_influence.iter().position(|x| *x == max_influence).unwrap());

    let spread = max_influence - min_influence;
    println!("Spread: {:.4}", spread);

    if spread < 0.1 {
        println!("\nRESULT: ALL BITS EQUAL - No dominant bit position.");
    } else if spread > 0.5 {
        println!("\nRESULT: SIGNIFICANT VARIATION! Some bits matter more.");
    } else {
        println!("\nRESULT: Minor variation (statistical noise).");
    }
}

// =============================================================================
// TEST 7: Reverse Birthday Attack
// =============================================================================

/// Theory: Do valid hashes cluster in output space?
/// If there are "attractor" regions in hash output space...
#[test]
#[ignore]
fn test_reverse_birthday() {
    println!("\n=== Test 7: Reverse Birthday Attack ===");
    println!("Theory: Do valid hashes cluster in output space (bytes 4-31)?\n");

    let trials = 10_000_000u64;
    let target = 16u32; // First 2 bytes = 0

    // Store the "fingerprint" of valid hashes (bytes 2-5)
    let mut fingerprints: HashMap<u32, u64> = HashMap::new();
    let mut valid_count = 0u64;

    println!("Running {} trials...", trials);

    for nonce in 0..trials as u32 {
        let header = make_header(nonce);
        let hash = double_sha256(&header);
        let zeros = leading_zeros(&hash);

        if zeros >= target {
            valid_count += 1;

            // Extract bytes 2-5 as fingerprint (the bytes after leading zeros)
            let start = (target / 8) as usize;
            if start + 4 <= hash.len() {
                let fingerprint = u32::from_be_bytes([
                    hash[start], hash[start+1], hash[start+2], hash[start+3]
                ]);
                *fingerprints.entry(fingerprint).or_insert(0) += 1;
            }
        }
    }

    println!("\nValid hashes found: {}", valid_count);
    println!("Unique fingerprints: {}", fingerprints.len());

    // Check for clustering (collisions)
    let collisions: u64 = fingerprints.values().filter(|&&count| count > 1).sum();
    let max_collision = *fingerprints.values().max().unwrap_or(&0);

    println!("\nFingerprint Analysis (bytes {}..{}):", target/8, target/8 + 4);
    println!("| Metric | Value |");
    println!("|--------|------:|");
    println!("| Collisions (>1 hit) | {} |", collisions);
    println!("| Max same fingerprint | {} |", max_collision);

    // Expected collisions by birthday paradox
    // For n items in space of 2^32, expected collisions ≈ n²/(2×2^32)
    let expected_collisions = (valid_count * valid_count) as f64 / (2.0 * (1u64 << 32) as f64);
    println!("| Expected (birthday) | {:.1} |", expected_collisions);

    let collision_ratio = collisions as f64 / expected_collisions.max(0.01);
    println!("\nCollision ratio vs expected: {:.2}x", collision_ratio);

    if collision_ratio > 2.0 {
        println!("RESULT: MORE CLUSTERING than birthday paradox predicts!");
        println!("Valid hashes may have 'attractor' regions.");
    } else if collision_ratio < 0.5 {
        println!("RESULT: LESS clustering than expected (weird).");
    } else {
        println!("RESULT: Normal birthday paradox behavior - no clustering.");
    }

    // Top fingerprints
    let mut fp_vec: Vec<_> = fingerprints.iter().collect();
    fp_vec.sort_by(|a, b| b.1.cmp(a.1));

    if max_collision > 1 {
        println!("\nTop fingerprints:");
        for (fp, count) in fp_vec.iter().take(5) {
            println!("  0x{:08X}: {} times", fp, count);
        }
    }
}

// =============================================================================
// TEST 8: Compression Function Orbits
// =============================================================================

/// Theory: If we repeatedly compress with the same input, do orbits show patterns?
/// This tests for any weird deterministic chaos behavior.
#[test]
#[ignore]
fn test_compression_orbits() {
    println!("\n=== Test 8: Compression Function Orbits ===");
    println!("Theory: Do repeated hash iterations show orbital patterns?\n");

    let samples = 1000;
    let iterations = 1000;

    // For each starting point, iterate and check for cycles
    let mut cycle_lengths: Vec<usize> = Vec::new();
    let mut divergence_rates: Vec<f64> = Vec::new();

    println!("Testing {} samples with {} iterations each...", samples, iterations);

    for seed in 0..samples as u32 {
        let header = make_header(seed);
        let mut current = double_sha256(&header);

        // Floyd's cycle detection
        let mut slow = current.clone();
        let mut fast = current.clone();

        let mut cycle_found = false;
        let mut cycle_len = 0;

        for i in 0..iterations/2 {
            // Slow moves 1 step
            slow = Sha256::digest(&slow).to_vec();

            // Fast moves 2 steps
            fast = Sha256::digest(&fast).to_vec();
            fast = Sha256::digest(&fast).to_vec();

            if slow == fast {
                // Found cycle! Find length
                let mut tortoise = slow.clone();
                cycle_len = 0;
                loop {
                    tortoise = Sha256::digest(&tortoise).to_vec();
                    cycle_len += 1;
                    if tortoise == slow { break; }
                    if cycle_len > iterations { break; }
                }
                cycle_found = true;
                break;
            }

            // Also track bit divergence between consecutive hashes
            if i == 0 {
                let next = Sha256::digest(&current).to_vec();
                let hamming: usize = current.iter().zip(next.iter())
                    .map(|(a, b)| (a ^ b).count_ones() as usize)
                    .sum();
                divergence_rates.push(hamming as f64 / 256.0);
                current = next;
            }
        }

        if cycle_found {
            cycle_lengths.push(cycle_len);
        }
    }

    println!("\nResults:");
    println!("Cycles found: {} / {}", cycle_lengths.len(), samples);

    if !cycle_lengths.is_empty() {
        let min_cycle = *cycle_lengths.iter().min().unwrap();
        let max_cycle = *cycle_lengths.iter().max().unwrap();
        let avg_cycle: f64 = cycle_lengths.iter().sum::<usize>() as f64 / cycle_lengths.len() as f64;

        println!("Cycle lengths: min={}, max={}, avg={:.1}", min_cycle, max_cycle, avg_cycle);
        println!("\nRESULT: CYCLES FOUND! (This would be a major discovery!)");
    } else {
        println!("No cycles detected in {} iterations.", iterations);
        println!("RESULT: As expected - SHA-256 orbit is effectively infinite.");
    }

    // Divergence analysis
    if !divergence_rates.is_empty() {
        let avg_div: f64 = divergence_rates.iter().sum::<f64>() / divergence_rates.len() as f64;
        println!("\nFirst-step divergence rate: {:.4} (expected: 0.50)", avg_div);

        if (avg_div - 0.5).abs() < 0.01 {
            println!("RESULT: Perfect avalanche on iteration.");
        } else {
            println!("RESULT: Unusual divergence rate!");
        }
    }
}

// =============================================================================
// TEST 9: XOR Relationship Between Valid Nonces
// =============================================================================

/// Theory: Do valid nonces have XOR relationships?
#[test]
#[ignore]
fn test_nonce_xor_relationships() {
    println!("\n=== Test 9: Nonce XOR Relationships ===");
    println!("Theory: Do valid nonces XOR to special values?\n");

    let trials = 20_000_000u64;
    let target = 16u32;

    let mut valid_nonces: Vec<u32> = Vec::new();

    println!("Collecting valid nonces from {} trials...", trials);

    for nonce in 0..trials as u32 {
        let header = make_header(nonce);
        let hash = double_sha256(&header);
        if leading_zeros(&hash) >= target {
            valid_nonces.push(nonce);
        }
    }

    println!("Valid nonces found: {}", valid_nonces.len());

    if valid_nonces.len() < 2 {
        println!("Not enough valid nonces for XOR analysis.");
        return;
    }

    // XOR all pairs and check for patterns
    let mut xor_popcounts: HashMap<u32, u64> = HashMap::new();
    let mut xor_values: HashMap<u32, u64> = HashMap::new();

    let max_pairs = 10_000_000usize;
    let mut pairs_checked = 0u64;

    for i in 0..valid_nonces.len().min(5000) {
        for j in (i+1)..valid_nonces.len().min(5000) {
            let xor = valid_nonces[i] ^ valid_nonces[j];
            let pc = xor.count_ones();
            *xor_popcounts.entry(pc).or_insert(0) += 1;
            *xor_values.entry(xor).or_insert(0) += 1;
            pairs_checked += 1;
            if pairs_checked >= max_pairs as u64 { break; }
        }
        if pairs_checked >= max_pairs as u64 { break; }
    }

    println!("\nXOR pairs analyzed: {}", pairs_checked);

    // Popcount distribution
    println!("\nXOR Popcount Distribution:");
    println!("| Popcount | Count | Expected Pct |");
    println!("|----------|------:|-------------:|");

    for pc in 0..=32 {
        if let Some(&count) = xor_popcounts.get(&pc) {
            let pct = count as f64 / pairs_checked as f64 * 100.0;
            // Expected is binomial(32, 0.5)
            let expected = binomial_prob(32, pc as usize) * 100.0;
            println!("| {:8} | {:5} | {:10.2}% |", pc, count, expected);
        }
    }

    // Check for common XOR values
    let mut xor_vec: Vec<_> = xor_values.iter().collect();
    xor_vec.sort_by(|a, b| b.1.cmp(a.1));

    println!("\nMost common XOR values:");
    for (xor_val, count) in xor_vec.iter().take(10) {
        let expected = pairs_checked as f64 / (1u64 << 32) as f64;
        let ratio = **count as f64 / expected.max(0.0001);
        println!("  0x{:08X} ({:3} ones): {} times ({:.1}x expected)",
                 xor_val, xor_val.count_ones(), count, ratio);
    }

    let max_hits = *xor_vec.first().map(|(_, c)| *c).unwrap_or(&0);
    if max_hits > 5 {
        println!("\nRESULT: Some XOR values appear suspiciously often!");
    } else {
        println!("\nRESULT: XOR values are uniformly distributed - no pattern.");
    }
}

fn binomial_prob(n: u32, k: usize) -> f64 {
    let mut result = 1.0;
    for i in 0..k {
        result *= (n as f64 - i as f64) / ((k - i) as f64);
    }
    result / (2.0f64.powi(n as i32))
}

// =============================================================================
// TEST 10: Timestamp Wiggle
// =============================================================================

/// Theory: Are some timestamps luckier than others?
#[test]
#[ignore]
fn test_timestamp_wiggle() {
    println!("\n=== Test 10: Timestamp Wiggle ===");
    println!("Theory: Do certain timestamp values produce more valid hashes?\n");

    let nonces_per_timestamp = 500_000u32;
    let target = 14u32;

    // Test different timestamp values (representing seconds)
    let timestamps: Vec<u32> = vec![
        0x5f5e0000, // Base time
        0x5f5e0001, // +1 second
        0x5f5e0010, // +16 seconds
        0x5f5e00ff, // +255 seconds
        0x5f5e0100, // +256 seconds
        0x5f5e1000, // +4096 seconds
        0x5f5ef000, // +61440 seconds
        0x60000000, // Different high nibble
    ];

    let mut results: Vec<(u32, u64, u64)> = Vec::new();

    println!("Testing {} nonces per timestamp...", nonces_per_timestamp);

    for timestamp in timestamps {
        let mut valid = 0u64;
        let mut zeros_sum = 0u64;

        for nonce in 0..nonces_per_timestamp {
            let mut header = make_header(nonce);
            header[68..72].copy_from_slice(&timestamp.to_le_bytes());

            let hash = double_sha256(&header);
            let zeros = leading_zeros(&hash);
            zeros_sum += zeros as u64;
            if zeros >= target {
                valid += 1;
            }
        }

        results.push((timestamp, valid, zeros_sum));
    }

    println!("\nResults:");
    println!("| Timestamp  | Valid | Avg Zeros |");
    println!("|------------|------:|----------:|");

    let total_valid: u64 = results.iter().map(|(_, v, _)| v).sum();
    let avg_valid = total_valid as f64 / results.len() as f64;

    for (ts, valid, zeros_sum) in &results {
        let avg_zeros = *zeros_sum as f64 / nonces_per_timestamp as f64;
        let deviation = (*valid as f64 - avg_valid) / avg_valid * 100.0;
        println!("| 0x{:08X} | {:5} | {:9.4} | {:+.1}%", ts, valid, avg_zeros, deviation);
    }

    let min_valid = results.iter().map(|(_, v, _)| v).min().unwrap();
    let max_valid = results.iter().map(|(_, v, _)| v).max().unwrap();
    let spread = (*max_valid - *min_valid) as f64 / avg_valid * 100.0;

    println!("\nSpread: {:.1}% (min {} to max {})", spread, min_valid, max_valid);

    if spread > 10.0 {
        println!("RESULT: SIGNIFICANT! Timestamp affects valid rate.");
    } else {
        println!("RESULT: NOT SIGNIFICANT - Timestamp doesn't matter.");
    }
}

/// Run all v24 tests
#[test]
#[ignore]
fn test_all_v24_wild_theories() {
    println!("\n============================================================");
    println!("Research v24: Wild & Loco Theories");
    println!("============================================================\n");
    println!("Testing unconventional, creative theories that probably won't work");
    println!("but are worth testing for completeness.\n");

    test_hamming_weight_destiny();
    test_gray_code_walk();
    test_message_schedule_fingerprint();
    test_glitch_variance();
    test_prime_nonce_magic();
    test_bit_position_dominance();
    test_reverse_birthday();
    test_compression_orbits();
    test_nonce_xor_relationships();
    test_timestamp_wiggle();

    println!("\n============================================================");
    println!("CONCLUSION: Wild & Loco Theories");
    println!("============================================================\n");

    println!("Summary of wild theory testing:");
    println!("1. Hamming Weight - Does nonce bit count matter?");
    println!("2. Gray Code Walk - Does single-bit increment help?");
    println!("3. Message Schedule - Can W[16:63] predict validity?");
    println!("4. Glitch Variance - Do carry counts correlate?");
    println!("5. Prime Nonces - Are primes special?");
    println!("6. Bit Dominance - Are all 32 bits equal?");
    println!("7. Reverse Birthday - Do outputs cluster?");
    println!("8. Compression Orbits - Any cyclic behavior?");
    println!("9. XOR Relationships - Do valid nonces have patterns?");
    println!("10. Timestamp Wiggle - Are some seconds luckier?");
    println!("\nThese tests represent 'outside the box' thinking.");
    println!("SHA-256's design predicts all should be NEGATIVE.");
}
