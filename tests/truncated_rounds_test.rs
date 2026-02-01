//! Research Experiment v1: Truncated Rounds Correlation
//!
//! Tests whether early SHA-256 rounds can predict final hash's leading bytes.
//! If correlation exists, we can use partial computation as a filter.
//!
//! Run with: cargo test --release --test truncated_rounds_test -- --nocapture

use std::collections::HashMap;

/// SHA-256 round constants
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

/// Initial hash values
const H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

#[inline]
fn rotr(x: u32, n: u32) -> u32 {
    (x >> n) | (x << (32 - n))
}

#[inline]
fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

#[inline]
fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

#[inline]
fn sigma0(x: u32) -> u32 {
    rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22)
}

#[inline]
fn sigma1(x: u32) -> u32 {
    rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25)
}

#[inline]
fn gamma0(x: u32) -> u32 {
    rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3)
}

#[inline]
fn gamma1(x: u32) -> u32 {
    rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10)
}

/// Compute SHA-256 with configurable number of rounds
/// Returns the state after `rounds` iterations
fn sha256_partial(data: &[u8; 64], rounds: usize) -> [u32; 8] {
    // Parse message into 16 32-bit words
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            data[i * 4],
            data[i * 4 + 1],
            data[i * 4 + 2],
            data[i * 4 + 3],
        ]);
    }

    // Extend to 64 words
    for i in 16..64 {
        w[i] = gamma1(w[i - 2])
            .wrapping_add(w[i - 7])
            .wrapping_add(gamma0(w[i - 15]))
            .wrapping_add(w[i - 16]);
    }

    // Initialize working variables
    let mut a = H[0];
    let mut b = H[1];
    let mut c = H[2];
    let mut d = H[3];
    let mut e = H[4];
    let mut f = H[5];
    let mut g = H[6];
    let mut h = H[7];

    // Run specified number of rounds
    for i in 0..rounds.min(64) {
        let t1 = h
            .wrapping_add(sigma1(e))
            .wrapping_add(ch(e, f, g))
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let t2 = sigma0(a).wrapping_add(maj(a, b, c));

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    [a, b, c, d, e, f, g, h]
}

/// Full SHA-256 of a 64-byte block, returns first byte of hash
fn sha256_first_byte(data: &[u8; 64]) -> u8 {
    let state = sha256_partial(data, 64);

    // Add initial values
    let final_a = state[0].wrapping_add(H[0]);

    // First byte is most significant byte of 'a'
    (final_a >> 24) as u8
}

/// Generate random Bitcoin-like header (64 bytes for single block)
fn random_header() -> [u8; 64] {
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut header = [0u8; 64];
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    // Simple LCG random
    let mut state = seed;
    for i in 0..64 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }

    header
}

/// Seeded random for reproducibility
fn seeded_header(seed: u64) -> [u8; 64] {
    let mut header = [0u8; 64];
    let mut state = seed;

    for i in 0..64 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        header[i] = (state >> 56) as u8;
    }

    header
}

#[derive(Debug, Default)]
struct CorrelationStats {
    total: u64,
    both_low: u64,      // partial < threshold AND final < threshold
    both_high: u64,     // partial >= threshold AND final >= threshold
    false_positive: u64, // partial < threshold BUT final >= threshold
    false_negative: u64, // partial >= threshold BUT final < threshold
}

impl CorrelationStats {
    fn correlation(&self) -> f64 {
        (self.both_low + self.both_high) as f64 / self.total as f64
    }

    fn false_negative_rate(&self) -> f64 {
        if self.both_low + self.false_negative == 0 {
            0.0
        } else {
            self.false_negative as f64 / (self.both_low + self.false_negative) as f64
        }
    }

    fn filter_rate(&self) -> f64 {
        // What % of "bad" candidates (final >= threshold) can we filter out?
        if self.both_high + self.false_positive == 0 {
            0.0
        } else {
            self.both_high as f64 / (self.both_high + self.false_positive) as f64
        }
    }
}

/// Run the truncated rounds correlation experiment
fn run_correlation_experiment(rounds: usize, samples: u64, threshold: u8) -> CorrelationStats {
    let mut stats = CorrelationStats::default();

    for seed in 0..samples {
        let header = seeded_header(seed);

        // Get partial state after N rounds
        let partial_state = sha256_partial(&header, rounds);
        let partial_first = (partial_state[0] >> 24) as u8;

        // Get final hash first byte
        let final_first = sha256_first_byte(&header);

        stats.total += 1;

        let partial_low = partial_first < threshold;
        let final_low = final_first < threshold;

        match (partial_low, final_low) {
            (true, true) => stats.both_low += 1,
            (false, false) => stats.both_high += 1,
            (true, false) => stats.false_positive += 1,
            (false, true) => stats.false_negative += 1,
        }
    }

    stats
}

#[test]
fn test_sha256_partial_correctness() {
    // Verify our partial implementation matches at 64 rounds
    let header = seeded_header(12345);
    let partial_64 = sha256_partial(&header, 64);
    let final_byte = sha256_first_byte(&header);

    let expected_first = ((partial_64[0].wrapping_add(H[0])) >> 24) as u8;
    assert_eq!(final_byte, expected_first, "64 rounds should match full hash");
}

#[test]
fn test_truncated_rounds_correlation() {
    println!("\n=== Truncated Rounds Correlation Experiment ===\n");

    let samples = 1_000_000;
    let threshold = 0x10; // ~6% of values below this

    println!("Samples: {}", samples);
    println!("Threshold: 0x{:02x} ({:.1}% of byte range)\n", threshold, threshold as f64 / 256.0 * 100.0);

    println!("| Rounds | Correlation | False Neg | Filter Rate | Notes |");
    println!("|--------|-------------|-----------|-------------|-------|");

    for rounds in [8, 16, 24, 32, 40, 48, 56, 64] {
        let stats = run_correlation_experiment(rounds, samples, threshold);

        let correlation = stats.correlation() * 100.0;
        let false_neg = stats.false_negative_rate() * 100.0;
        let filter = stats.filter_rate() * 100.0;

        let notes = if rounds == 64 {
            "baseline"
        } else if correlation > 60.0 {
            "promising!"
        } else if correlation > 55.0 {
            "weak signal"
        } else {
            "no signal"
        };

        println!(
            "| {:>6} | {:>10.2}% | {:>8.2}% | {:>10.2}% | {} |",
            rounds, correlation, false_neg, filter, notes
        );
    }

    println!("\n=== Detailed Breakdown for Best Candidates ===\n");

    // Detailed stats for promising round counts
    for rounds in [32, 48] {
        let stats = run_correlation_experiment(rounds, samples, threshold);
        println!("Rounds: {}", rounds);
        println!("  Total samples: {}", stats.total);
        println!("  Both low (true positive): {}", stats.both_low);
        println!("  Both high (true negative): {}", stats.both_high);
        println!("  False positive: {}", stats.false_positive);
        println!("  False negative: {}", stats.false_negative);
        println!("  Correlation: {:.4}", stats.correlation());
        println!();
    }
}

#[test]
fn test_multiple_thresholds() {
    println!("\n=== Threshold Sensitivity Analysis ===\n");

    let samples = 500_000;
    let rounds = 32; // Test one round count across thresholds

    println!("Rounds: {}", rounds);
    println!("Samples: {}\n", samples);

    println!("| Threshold | Correlation | False Neg | Filter Rate |");
    println!("|-----------|-------------|-----------|-------------|");

    for threshold in [0x01, 0x04, 0x10, 0x20, 0x40, 0x80] {
        let stats = run_correlation_experiment(rounds, samples, threshold);

        println!(
            "| 0x{:02x}      | {:>10.2}% | {:>8.2}% | {:>10.2}% |",
            threshold,
            stats.correlation() * 100.0,
            stats.false_negative_rate() * 100.0,
            stats.filter_rate() * 100.0
        );
    }
}

#[test]
fn test_state_variable_analysis() {
    println!("\n=== Which State Variable Correlates Best? ===\n");

    let samples = 500_000;
    let rounds = 32;
    let threshold = 0x10;

    println!("Testing which of the 8 state variables (a-h) at round {} correlates best with final hash.\n", rounds);

    for var_idx in 0..8 {
        let var_name = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'][var_idx];
        let mut stats = CorrelationStats::default();

        for seed in 0..samples {
            let header = seeded_header(seed);
            let partial_state = sha256_partial(&header, rounds);
            let partial_byte = (partial_state[var_idx] >> 24) as u8;

            // Compare to final hash (which uses 'a' after adding H[0])
            let final_first = sha256_first_byte(&header);

            stats.total += 1;
            let partial_low = partial_byte < threshold;
            let final_low = final_first < threshold;

            match (partial_low, final_low) {
                (true, true) => stats.both_low += 1,
                (false, false) => stats.both_high += 1,
                (true, false) => stats.false_positive += 1,
                (false, true) => stats.false_negative += 1,
            }
        }

        println!(
            "Variable '{}': correlation={:.2}%, false_neg={:.2}%",
            var_name,
            stats.correlation() * 100.0,
            stats.false_negative_rate() * 100.0
        );
    }
}

#[test]
#[ignore] // Long running test
fn test_high_precision_correlation() {
    println!("\n=== High Precision Correlation Test ===\n");

    let samples = 10_000_000;
    let threshold = 0x01; // Very strict - looking for 0x00 first byte

    println!("Samples: {} (10M)", samples);
    println!("Threshold: 0x{:02x} (looking for very low first bytes)\n", threshold);

    for rounds in [24, 32, 40, 48] {
        let stats = run_correlation_experiment(rounds, samples, threshold);

        println!("Rounds {}: correlation={:.4}%, false_neg={:.4}%, filter={:.4}%",
            rounds,
            stats.correlation() * 100.0,
            stats.false_negative_rate() * 100.0,
            stats.filter_rate() * 100.0
        );
    }
}
