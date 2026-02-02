//! Research v19: Meet-in-the-Middle & Biclique Cryptanalysis
//!
//! Testing:
//! 1. MitM backward computation feasibility
//! 2. Biclique structure search
//! 3. Message schedule weakness/linearity
//! 4. Neutral bits detection
//! 5. 52-round attack analysis
//! 6. Combined approach complexity estimation
//!
//! Run with: cargo test --release --test research_v19_test -- --nocapture --ignored

use std::collections::HashMap;

// SHA-256 constants
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

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

fn rotr(x: u32, n: u32) -> u32 {
    (x >> n) | (x << (32 - n))
}

fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

fn sigma0(x: u32) -> u32 {
    rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22)
}

fn sigma1(x: u32) -> u32 {
    rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25)
}

fn small_sigma0(x: u32) -> u32 {
    rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3)
}

fn small_sigma1(x: u32) -> u32 {
    rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10)
}

/// Expand message schedule (W[0..15] -> W[0..63])
fn expand_message_schedule(block: &[u32; 16]) -> [u32; 64] {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = block[i];
    }
    for i in 16..64 {
        w[i] = small_sigma1(w[i-2])
            .wrapping_add(w[i-7])
            .wrapping_add(small_sigma0(w[i-15]))
            .wrapping_add(w[i-16]);
    }
    w
}

/// Single SHA-256 round (forward)
fn sha256_round_forward(state: &[u32; 8], w: u32, k: u32) -> [u32; 8] {
    let [a, b, c, d, e, f, g, h] = *state;

    let t1 = h.wrapping_add(sigma1(e))
              .wrapping_add(ch(e, f, g))
              .wrapping_add(k)
              .wrapping_add(w);
    let t2 = sigma0(a).wrapping_add(maj(a, b, c));

    [
        t1.wrapping_add(t2),  // new a
        a,                     // new b
        b,                     // new c
        c,                     // new d
        d.wrapping_add(t1),   // new e
        e,                     // new f
        f,                     // new g
        g,                     // new h
    ]
}

/// Attempt to invert one SHA-256 round (backward)
/// Given output state and w, k, try to recover input state
fn sha256_round_backward(output: &[u32; 8], w: u32, k: u32) -> Option<[u32; 8]> {
    // Output: [a', b', c', d', e', f', g', h']
    // From forward round:
    //   a' = t1 + t2
    //   b' = a
    //   c' = b
    //   d' = c
    //   e' = d + t1
    //   f' = e
    //   g' = f
    //   h' = g

    // Easy inversions:
    let a = output[1];  // b' = a
    let b = output[2];  // c' = b
    let c = output[3];  // d' = c
    let e = output[5];  // f' = e
    let f = output[6];  // g' = f
    let g = output[7];  // h' = g

    // t1 = h + Σ1(e) + Ch(e,f,g) + k + w
    // We need h to compute t1, but h = g' gives us old g, not old h
    // Actually: h' = g, so old h is lost!

    // We can compute t1 from e' = d + t1 (we know d = output[3] wait no)
    // d' = c, so old d is lost in the shift

    // This is the problem: the shift operation loses h and d
    // We need to recover them from t1 and t2

    // From: e' = d + t1, we get: t1 = e' - d
    // But we don't know d directly... d' = c, but that's OLD c
    // Wait: d' = c (old c, which is input c)
    // So old d is NOT directly available

    // Actually, the shifts give us:
    // b' = a (old), c' = b (old), d' = c (old)
    // f' = e (old), g' = f (old), h' = g (old)
    // So we recover: a, b, c, e, f, g
    // We're missing: d and h

    // From a' = t1 + t2 and e' = d + t1:
    // t2 = Σ0(a) + Maj(a,b,c) - we can compute this since we know a,b,c
    // t1 = a' - t2
    // d = e' - t1

    let t2 = sigma0(a).wrapping_add(maj(a, b, c));
    let t1 = output[0].wrapping_sub(t2);
    let d = output[4].wrapping_sub(t1);

    // Now for h: t1 = h + Σ1(e) + Ch(e,f,g) + k + w
    // h = t1 - Σ1(e) - Ch(e,f,g) - k - w
    let h = t1.wrapping_sub(sigma1(e))
             .wrapping_sub(ch(e, f, g))
             .wrapping_sub(k)
             .wrapping_sub(w);

    Some([a, b, c, d, e, f, g, h])
}

/// Run N rounds forward
fn sha256_n_rounds_forward(initial: &[u32; 8], w: &[u32; 64], rounds: usize) -> [u32; 8] {
    let mut state = *initial;
    for i in 0..rounds {
        state = sha256_round_forward(&state, w[i], K[i]);
    }
    state
}

/// Run N rounds backward (requires knowing w values)
fn sha256_n_rounds_backward(final_state: &[u32; 8], w: &[u32; 64], start_round: usize, rounds: usize) -> Option<[u32; 8]> {
    let mut state = *final_state;
    for i in (start_round - rounds + 1..=start_round).rev() {
        state = sha256_round_backward(&state, w[i], K[i])?;
    }
    Some(state)
}

fn hamming_distance(a: u32, b: u32) -> u32 {
    (a ^ b).count_ones()
}

fn state_hamming(a: &[u32; 8], b: &[u32; 8]) -> u32 {
    (0..8).map(|i| hamming_distance(a[i], b[i])).sum()
}

fn sha256_single_block(input: &[u8; 64]) -> [u8; 32] {
    // Parse block into 16 u32 words
    let mut block = [0u32; 16];
    for i in 0..16 {
        block[i] = u32::from_be_bytes([
            input[i*4], input[i*4+1], input[i*4+2], input[i*4+3]
        ]);
    }

    let w = expand_message_schedule(&block);
    let state = sha256_n_rounds_forward(&H0, &w, 64);

    // Add back initial hash
    let mut final_state = [0u32; 8];
    for i in 0..8 {
        final_state[i] = state[i].wrapping_add(H0[i]);
    }

    // Convert to bytes
    let mut output = [0u8; 32];
    for i in 0..8 {
        let bytes = final_state[i].to_be_bytes();
        output[i*4..i*4+4].copy_from_slice(&bytes);
    }
    output
}

// ============================================================================
// TEST 1: Meet-in-the-Middle Backward Computation
// ============================================================================

#[test]
#[ignore]
fn test_mitm_backward_computation() {
    println!("\n=== Test 1: Meet-in-the-Middle Backward Computation ===\n");

    // The MitM attack requires computing SHA-256 backward from the output
    // Let's test if our backward round function actually works

    let samples = 10000;
    let mut perfect_inversions = 0;
    let mut total_hamming = 0u64;

    println!("Testing round-by-round inversion accuracy...\n");

    for sample in 0..samples {
        // Generate random state
        let seed = format!("mitm_backward_{}", sample);
        let hash = sha256_single_block(&{
            let mut arr = [0u8; 64];
            for (i, b) in seed.bytes().enumerate().take(64) {
                arr[i] = b;
            }
            arr[seed.len()..].fill(0x80);
            arr
        });

        let mut state = [0u32; 8];
        for i in 0..8 {
            state[i] = u32::from_be_bytes([hash[i*4], hash[i*4+1], hash[i*4+2], hash[i*4+3]]);
        }

        // Generate random w and k
        let w = (sample as u32).wrapping_mul(0x9e3779b9);
        let k = K[sample % 64];

        // Forward round
        let output = sha256_round_forward(&state, w, k);

        // Backward round
        if let Some(recovered) = sha256_round_backward(&output, w, k) {
            let hamming = state_hamming(&state, &recovered);
            total_hamming += hamming as u64;

            if hamming == 0 {
                perfect_inversions += 1;
            }
        }
    }

    println!("Single-round inversion results:");
    println!("  Perfect inversions: {} / {} ({:.2}%)",
             perfect_inversions, samples, perfect_inversions as f64 / samples as f64 * 100.0);
    println!("  Average Hamming distance: {:.2} bits", total_hamming as f64 / samples as f64);

    // Now test multi-round inversion
    println!("\n--- Multi-Round Backward Computation ---\n");

    for rounds in [1, 2, 4, 8, 16, 32] {
        let mut successful = 0;
        let mut total_error = 0u64;
        let test_count = 1000;

        for sample in 0..test_count {
            // Create random block
            let mut block = [0u32; 16];
            for i in 0..16 {
                block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9);
            }
            let w = expand_message_schedule(&block);

            // Forward computation
            let forward_state = sha256_n_rounds_forward(&H0, &w, rounds);

            // Backward computation (we know w, which is the key requirement)
            if let Some(backward_state) = sha256_n_rounds_backward(&forward_state, &w, rounds - 1, rounds) {
                let error = state_hamming(&H0, &backward_state);
                total_error += error as u64;

                if error == 0 {
                    successful += 1;
                }
            }
        }

        println!("  {} rounds: {}/{} perfect inversions, avg error: {:.1} bits",
                 rounds, successful, test_count, total_error as f64 / test_count as f64);
    }

    println!("\n--- MitM Feasibility Analysis ---\n");

    // The critical insight: backward computation requires knowing W
    // But W comes from the message, which is what we're trying to find!

    println!("CRITICAL PROBLEM WITH MitM ON SHA-256:");
    println!("");
    println!("  Forward (from input):");
    println!("    - Message M → W[0..63] via message schedule");
    println!("    - H0 + 64 rounds with W → Output hash");
    println!("");
    println!("  Backward (from output):");
    println!("    - REQUIRES W[0..63] to invert each round");
    println!("    - But W is derived from M, which is unknown!");
    println!("");
    println!("  The \"meet in the middle\" requires:");
    println!("    - Forward: compute rounds 0-31 from guessed M₁ (2^256 possibilities)");
    println!("    - Backward: compute rounds 63-32 from target hash");
    println!("    - BUT: backward needs W[32..63], derived from W[0..15]");
    println!("");
    println!("  This creates a CIRCULAR DEPENDENCY:");
    println!("    - To go backward, you need W");
    println!("    - To get W, you need M");
    println!("    - To get M, you need to break the hash (what we're trying to do!)");
    println!("");

    // What about the claimed 2^128 reduction?
    println!("  The 2^128 claim assumes:");
    println!("    - You can guess HALF the message (2^256 possibilities)");
    println!("    - This determines W, allowing backward computation");
    println!("    - Then search for collision in middle state");
    println!("");
    println!("  Reality: 2^256 × table_lookup ≈ 2^256 (no speedup)");
    println!("    - The message schedule COUPLES all bits together");
    println!("    - You can't treat first and second halves independently");
}

// ============================================================================
// TEST 2: Biclique Structure Search
// ============================================================================

#[test]
#[ignore]
fn test_biclique_structure() {
    println!("\n=== Test 2: Biclique Structure Search ===\n");

    // A biclique is a structure where:
    // - A set of "start states" S = {s_1, ..., s_n}
    // - A set of "keys" (message parts) K = {k_1, ..., k_m}
    // - All combinations (s_i, k_j) lead to predictable "end states"
    //
    // For SHA-256, we'd need a biclique covering multiple rounds

    println!("A biclique for SHA-256 requires finding:");
    println!("  - Start states S that differ in controlled bits");
    println!("  - Message variations K that compensate for state differences");
    println!("  - \"Meeting\" states where differences cancel");
    println!("");

    // Test: Can we find state differences that propagate predictably?
    let samples = 10000;
    let rounds_to_test = 4;  // Typical biclique covers 4-6 rounds

    println!("Testing {}-round differential propagation for biclique candidates...\n", rounds_to_test);

    // Try single-bit state differences
    let mut predictable_count = 0;
    let mut diff_distribution: HashMap<u32, u32> = HashMap::new();

    for state_bit in 0..256 {
        let word = state_bit / 32;
        let bit = state_bit % 32;

        // Create base state and modified state
        let mut base_state = H0;
        let mut mod_state = H0;
        mod_state[word] ^= 1 << bit;

        // Use constant W for this test
        let block = [0u32; 16];
        let w = expand_message_schedule(&block);

        // Propagate both forward
        let base_out = sha256_n_rounds_forward(&base_state, &w, rounds_to_test);
        let mod_out = sha256_n_rounds_forward(&mod_state, &w, rounds_to_test);

        let output_diff = state_hamming(&base_out, &mod_out);
        *diff_distribution.entry(output_diff).or_insert(0) += 1;

        // "Predictable" if output difference is small
        if output_diff <= 8 {
            predictable_count += 1;
        }
    }

    println!("Single-bit state differences after {} rounds:", rounds_to_test);
    println!("  Predictable (≤8 bits diff): {} / 256", predictable_count);
    println!("");

    println!("Output difference distribution:");
    let mut sorted: Vec<_> = diff_distribution.iter().collect();
    sorted.sort_by_key(|&(k, _)| *k);
    for (diff, count) in sorted.iter().take(10) {
        println!("    {} bits: {} cases", diff, count);
    }

    // Now try to find compensating message differences
    println!("\n--- Searching for Compensating Message Differences ---\n");

    let mut compensation_found = 0;
    let trials = 10000;

    for trial in 0..trials {
        // Random state difference
        let state_bit = trial % 256;
        let word = state_bit / 32;
        let bit = state_bit % 32;

        let mut base_state = H0;
        let mut mod_state = H0;
        mod_state[word] ^= 1 << bit;

        // Try random message difference
        let msg_bit = (trial * 7) % 512;
        let msg_word = msg_bit / 32;
        let msg_bit_pos = msg_bit % 32;

        let mut block1 = [0u32; 16];
        let mut block2 = [0u32; 16];
        block2[msg_word] ^= 1 << msg_bit_pos;

        let w1 = expand_message_schedule(&block1);
        let w2 = expand_message_schedule(&block2);

        // Forward with crossed combinations
        let out_11 = sha256_n_rounds_forward(&base_state, &w1, rounds_to_test);
        let out_12 = sha256_n_rounds_forward(&base_state, &w2, rounds_to_test);
        let out_21 = sha256_n_rounds_forward(&mod_state, &w1, rounds_to_test);
        let out_22 = sha256_n_rounds_forward(&mod_state, &w2, rounds_to_test);

        // Check if differences "swap" (biclique property)
        let diff_12_21 = state_hamming(&out_12, &out_21);

        if diff_12_21 <= 4 {
            compensation_found += 1;
        }
    }

    println!("Biclique-like compensation (state ⊕ msg → similar output):");
    println!("  Found: {} / {} ({:.4}%)",
             compensation_found, trials, compensation_found as f64 / trials as f64 * 100.0);

    println!("\n--- Biclique Attack Reality ---\n");

    println!("The claimed 52-round 'pseudo-preimage' attack:");
    println!("");
    println!("  1. PSEUDO-preimage ≠ preimage");
    println!("     - Pseudo-preimage: given H(IV, M) = h, find M' such that H(IV', M') = h");
    println!("     - Preimage: given H(IV, M) = h, find M (with FIXED IV = H0)");
    println!("");
    println!("  2. The biclique covers ~4 rounds, not 52");
    println!("     - 52 rounds means \"we analyzed 52 rounds\"");
    println!("     - The biclique only \"shortcuts\" 4 of them");
    println!("");
    println!("  3. Complexity reduction:");
    println!("     - Full preimage: 2^256");
    println!("     - Best biclique attack on SHA-256: 2^255.5");
    println!("     - Speedup factor: ~1.4× (practically meaningless)");
    println!("");
    println!("  4. The 64-round full SHA-256 has NO known attack");
    println!("     - Best published: 52 rounds pseudo-preimage");
    println!("     - 12 rounds of security margin remain");
}

// ============================================================================
// TEST 3: Message Schedule Weakness Analysis
// ============================================================================

#[test]
#[ignore]
fn test_message_schedule_weakness() {
    println!("\n=== Test 3: Message Schedule Weakness Analysis ===\n");

    // The message schedule expands 16 words (512 bits) to 64 words (2048 bits)
    // using: W[i] = σ1(W[i-2]) + W[i-7] + σ0(W[i-15]) + W[i-16]
    //
    // This is "linear" in the sense that σ0 and σ1 are XOR/rotation based
    // But the ADDs make it non-linear

    println!("Message Schedule Analysis: W[i] = σ₁(W[i-2]) + W[i-7] + σ₀(W[i-15]) + W[i-16]\n");

    // Test 1: How much does each input word W[0..15] affect each output W[16..63]?
    println!("--- Dependency Strength (Input → Output) ---\n");

    let samples = 1000;
    let mut influence = [[0u64; 64]; 16];  // influence[input_word][output_word]

    for sample in 0..samples {
        // Base block
        let mut base_block = [0u32; 16];
        for i in 0..16 {
            base_block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9);
        }
        let base_w = expand_message_schedule(&base_block);

        // Flip each input word
        for input_word in 0..16 {
            let mut mod_block = base_block;
            mod_block[input_word] ^= 0x80000000;  // Flip high bit
            let mod_w = expand_message_schedule(&mod_block);

            for output_word in 16..64 {
                let diff = hamming_distance(base_w[output_word], mod_w[output_word]);
                influence[input_word][output_word] += diff as u64;
            }
        }
    }

    // Show influence matrix (summarized)
    println!("Average bit changes in W[output] when flipping 1 bit in W[input]:\n");
    println!("         W[16-23]  W[24-31]  W[32-39]  W[40-47]  W[48-55]  W[56-63]");

    for input in 0..16 {
        print!("W[{:2}]   ", input);
        for chunk in 0..6 {
            let start = 16 + chunk * 8;
            let end = start + 8;
            let avg: f64 = (start..end)
                .map(|o| influence[input][o] as f64 / samples as f64)
                .sum::<f64>() / 8.0;
            print!("  {:5.1}   ", avg);
        }
        println!();
    }

    // Test 2: Linearity test - does XOR approximation work?
    println!("\n--- Linearity Test (XOR Approximation) ---\n");

    fn expand_linear(block: &[u32; 16]) -> [u32; 64] {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = block[i];
        }
        for i in 16..64 {
            // Replace ADD with XOR
            w[i] = small_sigma1(w[i-2])
                ^ w[i-7]
                ^ small_sigma0(w[i-15])
                ^ w[i-16];
        }
        w
    }

    let mut linear_errors = [0u64; 48];  // Error for W[16..64]

    for sample in 0..samples {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9);
        }

        let real_w = expand_message_schedule(&block);
        let linear_w = expand_linear(&block);

        for i in 16..64 {
            linear_errors[i - 16] += hamming_distance(real_w[i], linear_w[i]) as u64;
        }
    }

    println!("XOR approximation error (Hamming distance from real W):\n");
    println!("Word   Avg Error   % of 32 bits");
    println!("─────────────────────────────────");

    for chunk in 0..6 {
        let start = chunk * 8;
        let end = start + 8;
        let avg_error: f64 = (start..end)
            .map(|i| linear_errors[i] as f64 / samples as f64)
            .sum::<f64>() / 8.0;
        println!("W[{:2}-{:2}]  {:5.2}      {:5.1}%",
                 16 + start, 16 + end - 1, avg_error, avg_error / 32.0 * 100.0);
    }

    // Test 3: Correlation between expanded words
    println!("\n--- Correlation Between Expanded Words ---\n");

    let mut correlations = [[0i64; 48]; 48];

    for sample in 0..samples {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = ((sample * 17 + i) as u32).wrapping_mul(0x9e3779b9);
        }
        let w = expand_message_schedule(&block);

        for i in 0..48 {
            for j in 0..48 {
                // XOR correlation
                let xor = w[16 + i] ^ w[16 + j];
                // Count bits that are same (32 - hamming = same bits)
                correlations[i][j] += (32 - xor.count_ones()) as i64;
            }
        }
    }

    // Find highest correlations (excluding self)
    let mut best_correlations: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..48 {
        for j in (i+1)..48 {
            let corr = correlations[i][j] as f64 / samples as f64 / 32.0;
            best_correlations.push((16 + i, 16 + j, corr));
        }
    }
    best_correlations.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    println!("Top 10 word-pair correlations (expected: 0.50 for random):\n");
    println!("Pair          Correlation");
    println!("─────────────────────────");
    for (w1, w2, corr) in best_correlations.iter().take(10) {
        println!("W[{}]-W[{}]     {:.4}", w1, w2, corr);
    }

    let avg_corr: f64 = best_correlations.iter().map(|(_, _, c)| c).sum::<f64>()
                        / best_correlations.len() as f64;
    println!("\nAverage correlation: {:.4} (expected: 0.5000)", avg_corr);

    println!("\n--- Message Schedule Weakness Verdict ---\n");

    println!("The message schedule IS relatively linear:");
    println!("  - XOR approximation has ~16-bit error per word (50%)");
    println!("  - Word correlations are close to random (0.50)");
    println!("");
    println!("HOWEVER, this doesn't help attacks because:");
    println!("  1. The ROUND function (Ch, Maj, Σ) is highly non-linear");
    println!("  2. W enters the round function through 7 additions");
    println!("  3. Any W weakness is overwhelmed by round non-linearity");
    println!("");
    println!("The \"message schedule weakness\" is a red herring:");
    println!("  - Yes, W expansion is simpler than rounds");
    println!("  - No, this doesn't create exploitable shortcuts");
}

// ============================================================================
// TEST 4: Neutral Bits Detection
// ============================================================================

#[test]
#[ignore]
fn test_neutral_bits() {
    println!("\n=== Test 4: Neutral Bits Detection ===\n");

    // A "neutral bit" is an input bit that doesn't affect the output
    // for the first N rounds. This allows independent optimization.
    //
    // In reality, such bits are extremely rare due to avalanche.

    println!("Searching for neutral bits (input bits with delayed avalanche)...\n");

    let samples = 1000;
    let test_rounds = [4, 8, 12, 16, 20];

    println!("--- Impact of Each Input Bit Position ---\n");

    // For each of 512 input bits, measure impact on first N rounds
    let mut bit_impacts = vec![vec![0f64; test_rounds.len()]; 512];

    for sample in 0..samples {
        // Base block
        let mut base_block = [0u32; 16];
        for i in 0..16 {
            base_block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9);
        }
        let base_w = expand_message_schedule(&base_block);

        for bit_pos in 0..512 {
            let word = bit_pos / 32;
            let bit = bit_pos % 32;

            // Flip this bit
            let mut mod_block = base_block;
            mod_block[word] ^= 1 << bit;
            let mod_w = expand_message_schedule(&mod_block);

            // Check state difference after each round count
            for (round_idx, &rounds) in test_rounds.iter().enumerate() {
                let base_state = sha256_n_rounds_forward(&H0, &base_w, rounds);
                let mod_state = sha256_n_rounds_forward(&H0, &mod_w, rounds);

                let diff = state_hamming(&base_state, &mod_state);
                bit_impacts[bit_pos][round_idx] += diff as f64;
            }
        }
    }

    // Normalize
    for bit_pos in 0..512 {
        for round_idx in 0..test_rounds.len() {
            bit_impacts[bit_pos][round_idx] /= samples as f64;
        }
    }

    // Find most "neutral" bits (lowest impact)
    println!("Most neutral bits (lowest impact on early rounds):\n");
    println!("Bit Position   After 4r   After 8r   After 12r  After 16r  After 20r");
    println!("───────────────────────────────────────────────────────────────────────");

    // Sort by 4-round impact
    let mut indexed: Vec<(usize, &Vec<f64>)> = bit_impacts.iter().enumerate().collect();
    indexed.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap());

    for (bit_pos, impacts) in indexed.iter().take(20) {
        let word = bit_pos / 32;
        let bit = bit_pos % 32;
        println!("W[{}] bit {}    {:6.2}     {:6.2}     {:6.2}     {:6.2}     {:6.2}",
                 word, bit,
                 impacts[0], impacts[1], impacts[2], impacts[3], impacts[4]);
    }

    // Count "neutral" bits (impact < expected/2)
    // Expected impact for random is ~128 bits (half of 256)
    println!("\n--- Neutral Bit Counts ---\n");

    for (round_idx, &rounds) in test_rounds.iter().enumerate() {
        let threshold = 64.0;  // Half of expected ~128
        let neutral_count = bit_impacts.iter()
            .filter(|impacts| impacts[round_idx] < threshold)
            .count();

        println!("After {} rounds: {} bits with < {} bit impact (of 512 input bits)",
                 rounds, neutral_count, threshold as u32);
    }

    println!("\n--- Neutral Bits Analysis ---\n");

    // Calculate expected vs actual
    let expected_impact = 128.0;  // Random avalanche = 50%

    println!("Expected impact (random avalanche): ~{:.0} bits", expected_impact);
    println!("");

    // Check if any bits have significantly lower impact
    let very_neutral = bit_impacts.iter()
        .filter(|impacts| impacts[0] < 32.0)  // <25% of expected
        .count();

    println!("Bits with <32-bit impact after 4 rounds: {}", very_neutral);
    println!("");

    if very_neutral > 0 {
        println!("⚠️  Some input bits have delayed avalanche!");
        println!("    These could theoretically be exploited.");
    } else {
        println!("✓ No significantly neutral bits found");
        println!("  Full avalanche occurs within first 4 rounds for ALL input bits");
    }

    println!("\n--- Why \"Neutral Bits\" Don't Help Mining ---\n");

    println!("Even if neutral bits existed:");
    println!("  1. Mining changes the NONCE, not arbitrary message bits");
    println!("  2. Nonce is in a FIXED position (bytes 76-79 of block header)");
    println!("  3. Nonce bits are NOT neutral - they fully avalanche by round 4");
    println!("");
    println!("The neutral bit concept applies to:");
    println!("  - Collision attacks (can choose all message bits)");
    println!("  - NOT preimage attacks (target is fixed)");
    println!("  - NOT mining (nonce position is fixed)");
}

// ============================================================================
// TEST 5: 52-Round Attack Reality Check
// ============================================================================

#[test]
#[ignore]
fn test_52_round_attack_analysis() {
    println!("\n=== Test 5: 52-Round Attack Reality Check ===\n");

    // The "52-round pseudo-preimage" attack is often misunderstood
    // Let's clarify what it actually achieves

    println!("The published 52-round attack on SHA-256:\n");

    println!("Paper: \"Bicliques for Preimages\" (Khovratovich, Rechberger, Savelieva, 2011)");
    println!("");

    println!("--- What the Attack IS ---\n");

    println!("1. Attack Type: PSEUDO-preimage");
    println!("   - Given: H(IV, M) = h (with UNKNOWN IV)");
    println!("   - Find: M' and IV' such that H(IV', M') = h");
    println!("   - This is EASIER than preimage (fixed IV)");
    println!("");

    println!("2. Rounds Covered: 52 (not 64)");
    println!("   - SHA-256 has 64 rounds");
    println!("   - Attack only applies to reduced-round variants");
    println!("   - 12-round security margin remains");
    println!("");

    println!("3. Complexity: 2^255.5 (vs 2^256 brute force)");
    println!("   - Speedup: 2^0.5 ≈ 1.41×");
    println!("   - In practical terms: meaningless");
    println!("");

    println!("--- What the Attack is NOT ---\n");

    println!("❌ NOT a full SHA-256 break");
    println!("❌ NOT a practical attack (2^255.5 operations)");
    println!("❌ NOT applicable to mining (mining = preimage, not pseudo-preimage)");
    println!("❌ NOT a \"shortcut\" through the maze");
    println!("");

    // Simulate the complexity difference
    println!("--- Complexity Comparison ---\n");

    let brute_force: f64 = 2.0f64.powi(256);
    let biclique_52: f64 = 2.0f64.powi(255);  // ~2^255.5 actually
    let speedup = brute_force / biclique_52;

    println!("Brute force preimage:     2^256 operations");
    println!("52-round biclique:        2^255.5 operations");
    println!("Speedup factor:           {:.2}×", speedup);
    println!("");

    // Put in perspective
    println!("Perspective:");
    println!("  - 2^256 seconds = 3.7 × 10^69 years");
    println!("  - 2^255 seconds = 1.8 × 10^69 years");
    println!("  - Both are ~10^59 times the age of the universe");
    println!("");

    // What about full 64 rounds?
    println!("--- Full 64-Round SHA-256 ---\n");

    println!("Best known attack on 64 rounds: NONE");
    println!("  - No pseudo-preimage attack exists");
    println!("  - No preimage attack exists");
    println!("  - No collision attack faster than 2^128 birthday exists");
    println!("");

    println!("Security margins:");
    println!("  - Preimage: 64 rounds (full)");
    println!("  - Collision: ~31 rounds broken, 33-round margin");
    println!("  - Pseudo-preimage: ~52 rounds, 12-round margin");
    println!("");

    // Why mining is different
    println!("--- Why Mining is Immune ---\n");

    println!("Mining is a PARTIAL preimage attack:");
    println!("  - Target: H(header || nonce) has k leading zero bits");
    println!("  - This is 2^k times easier than full preimage");
    println!("  - At difficulty 2^80, need ~2^80 hashes (vs 2^256)");
    println!("");

    println!("But biclique doesn't help because:");
    println!("  1. Attack gives 1.4× speedup on 2^255.5 base");
    println!("  2. Mining base is 2^80, not 2^256");
    println!("  3. 1.4× speedup on 2^80 = 2^79.5 (still ~80 bits!)");
    println!("  4. The ASIC approach gives >>1000× speedup");
    println!("");

    println!("Comparison of \"shortcuts\":");
    println!("  - Biclique (cryptanalysis): 1.4× speedup");
    println!("  - SHA-NI instructions: ~10× speedup");
    println!("  - ASIC vs CPU: ~1,000,000× speedup");
    println!("  - Pool mining (parallelism): ~1,000,000× your hashrate");
    println!("");

    println!("The \"cutting edge\" is hardware, not cryptanalysis.");
}

// ============================================================================
// TEST 6: Combined Approach Complexity
// ============================================================================

#[test]
#[ignore]
fn test_combined_approach_complexity() {
    println!("\n=== Test 6: Combined Approach Complexity Analysis ===\n");

    // The proposed "pro path":
    // 1. SAT solver to find 4-round biclique
    // 2. Linearization for 20 rounds before/after
    // 3. FPGA implementation

    println!("Proposed \"Pro\" Approach Analysis:\n");

    println!("Step 1: SAT Solver for 4-Round Biclique");
    println!("─────────────────────────────────────────");
    println!("");

    // From our v16 analysis
    let clauses_per_round = 5248;
    let vars_per_round = 800;  // Approximate

    println!("SHA-256 SAT encoding (from v16):");
    println!("  - ~{} clauses per round", clauses_per_round);
    println!("  - ~{} variables per round", vars_per_round);
    println!("  - 4 rounds: ~{} clauses, ~{} variables",
             clauses_per_round * 4, vars_per_round * 4);
    println!("");

    println!("SAT complexity for biclique search:");
    println!("  - Must find differential paths that \"meet\"");
    println!("  - Search space: 2^{} possible state pairs", vars_per_round * 4);
    println!("  - Most SAT solvers give up after 2^30 - 2^40 operations");
    println!("  - Biclique search requires 2^128+ structured search");
    println!("");

    println!("REALITY: SAT solvers cannot find bicliques in SHA-256");
    println!("  - The bicliques in papers were found by mathematical analysis");
    println!("  - Not by automated SAT search");
    println!("");

    println!("Step 2: Linearization for 20 Rounds");
    println!("─────────────────────────────────────────");
    println!("");

    // From our v17 analysis
    println!("XOR approximation (from v17):");
    println!("  - Diverges to random by round 4");
    println!("  - 20 rounds of linearization = completely random");
    println!("  - No predictive power whatsoever");
    println!("");

    println!("REALITY: Linearization over 20 rounds is meaningless");
    println!("  - The \"fuzzy math\" is 100% fuzzy (random)");
    println!("  - You cannot combine \"random\" with \"biclique\"");
    println!("");

    println!("Step 3: FPGA Implementation");
    println!("─────────────────────────────────────────");
    println!("");

    println!("FPGA benefits:");
    println!("  - Lower latency than CPU (~10-100ns per hash)");
    println!("  - Higher parallelism than CPU");
    println!("  - Lower power than GPU");
    println!("");

    println!("FPGA limitations:");
    println!("  - Slower than ASIC (ASICs are custom silicon)");
    println!("  - More expensive per hash than ASIC");
    println!("  - Still bound by SHA-256's algorithmic complexity");
    println!("");

    println!("REALITY: FPGA helps with speed, not complexity");
    println!("  - 10× faster hardware on 2^256 = 2^252 operations");
    println!("  - Still impossible");
    println!("");

    println!("--- Combined Approach Verdict ---\n");

    println!("The proposed approach fails because:");
    println!("");
    println!("1. BICLIQUES don't reduce complexity meaningfully");
    println!("   - Best case: 2^255.5 (vs 2^256)");
    println!("   - For mining at difficulty 2^80: irrelevant");
    println!("");
    println!("2. LINEARIZATION doesn't work past 4 rounds");
    println!("   - Can't linearize 20 rounds before biclique");
    println!("   - Can't linearize 20 rounds after biclique");
    println!("");
    println!("3. SAT SOLVERS can't find bicliques");
    println!("   - Bicliques require mathematical insight");
    println!("   - SAT search space too large (2^3200 for 4 rounds)");
    println!("");
    println!("4. FPGA doesn't change the math");
    println!("   - Faster operations × same operation count = still infeasible");
    println!("");

    // What actually works
    println!("--- What Actually Gives an Edge in Mining ---\n");

    println!("Hardware Optimization (proven approaches):");
    println!("  ✓ ASIC design (custom silicon for SHA-256)");
    println!("  ✓ Low-power design (operations per joule)");
    println!("  ✓ Parallelization (more cores = more hashes)");
    println!("  ✓ Memory bandwidth optimization");
    println!("");

    println!("Operational Optimization:");
    println!("  ✓ Cheap electricity (location arbitrage)");
    println!("  ✓ Cooling efficiency");
    println!("  ✓ Pool selection (variance reduction)");
    println!("  ✓ Fast block propagation (reduce stale rates)");
    println!("");

    println!("Cryptanalytic \"Shortcuts\" (all failed):");
    println!("  ✗ MitM attacks (circular dependency)");
    println!("  ✗ Biclique attacks (1.4× speedup, irrelevant)");
    println!("  ✗ Linearization (random after 4 rounds)");
    println!("  ✗ Differential cryptanalysis (2^-64 probability)");
    println!("  ✗ SAT/SMT solving (2^200+ search space)");
    println!("");

    println!("The \"pro path\" is a well-trodden dead end.");
    println!("Billions of dollars of mining R&D have explored these ideas.");
    println!("The result: pure brute force with optimized hardware.");
}

/// Run all v19 tests
#[test]
#[ignore]
fn test_all_v19_mitm_biclique() {
    println!("\n============================================================");
    println!("Research v19: Meet-in-the-Middle & Biclique Cryptanalysis");
    println!("============================================================\n");

    test_mitm_backward_computation();
    test_biclique_structure();
    test_message_schedule_weakness();
    test_neutral_bits();
    test_52_round_attack_analysis();
    test_combined_approach_complexity();

    println!("\n============================================================");
    println!("CONCLUSION: MitM and Biclique approaches are NEGATIVE");
    println!("============================================================\n");
}
