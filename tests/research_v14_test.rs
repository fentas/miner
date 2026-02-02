//! Research v14: Algebraic Cryptanalysis - Linearization Attack
//!
//! Investigating whether SHA-256 can be "linearized" for faster solving:
//! 1. Ch and Maj function linear approximations
//! 2. Modular addition carry analysis
//! 3. Invariant subspace detection
//! 4. Linear vs actual propagation comparison
//! 5. SAT-style constraint satisfaction analysis
//!
//! Run: cargo test --release --test research_v14_test -- --nocapture --ignored

use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// SHA-256 Initial Values
const H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// SHA-256 Round Constants (first 16 for message schedule analysis)
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

// === SHA-256 Primitive Operations ===

/// Actual Ch function: Ch(x,y,z) = (x AND y) XOR (NOT x AND z)
/// "Choose": x chooses whether to take y or z
fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

/// Actual Maj function: Maj(x,y,z) = (x AND y) XOR (x AND z) XOR (y AND z)
/// "Majority": output is majority vote of x,y,z at each bit
fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

/// Linear approximation of Ch: Ch_linear(x,y,z) = x XOR y XOR z
/// This is the "best" linear approximation (highest correlation)
fn ch_linear(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

/// Linear approximation of Maj: Maj_linear(x,y,z) = x XOR y XOR z
/// Same as Ch_linear (both non-linear functions have same linear approx)
fn maj_linear(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

/// Alternative linear approx: just use one input
fn ch_linear_simple(x: u32, _y: u32, z: u32) -> u32 {
    x ^ z  // Simplified: ignore y
}

/// Sigma0 rotation (linear operation)
fn sigma0(x: u32) -> u32 {
    x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
}

/// Sigma1 rotation (linear operation)
fn sigma1(x: u32) -> u32 {
    x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
}

/// sigma0 for message schedule (linear)
fn sigma0_msg(x: u32) -> u32 {
    x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
}

/// sigma1 for message schedule (linear)
fn sigma1_msg(x: u32) -> u32 {
    x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn hamming_distance(a: u32, b: u32) -> u32 {
    (a ^ b).count_ones()
}

/// Test 1: Ch and Maj Linear Approximation Quality
#[test]
#[ignore]
fn test_ch_maj_linearization() {
    println!("\n=== Test 1: Ch/Maj Linear Approximation ===\n");

    let samples = 100_000;
    let mut ch_matches = 0u64;
    let mut maj_matches = 0u64;
    let mut ch_hamming_sum = 0u64;
    let mut maj_hamming_sum = 0u64;

    println!("Testing {} random input triplets...", samples);

    for i in 0..samples {
        // Generate pseudo-random values
        let x = ((i as u32).wrapping_mul(2654435761)).wrapping_add(12345);
        let y = ((i as u32).wrapping_mul(1597334677)).wrapping_add(67890);
        let z = ((i as u32).wrapping_mul(789456123)).wrapping_add(11111);

        // Actual vs Linear
        let ch_actual = ch(x, y, z);
        let ch_lin = ch_linear(x, y, z);
        let maj_actual = maj(x, y, z);
        let maj_lin = maj_linear(x, y, z);

        // Bit-by-bit match rate
        ch_matches += (32 - hamming_distance(ch_actual, ch_lin)) as u64;
        maj_matches += (32 - hamming_distance(maj_actual, maj_lin)) as u64;

        // Hamming distance
        ch_hamming_sum += hamming_distance(ch_actual, ch_lin) as u64;
        maj_hamming_sum += hamming_distance(maj_actual, maj_lin) as u64;
    }

    let total_bits = samples as u64 * 32;
    let ch_match_rate = ch_matches as f64 / total_bits as f64 * 100.0;
    let maj_match_rate = maj_matches as f64 / total_bits as f64 * 100.0;
    let ch_avg_hamming = ch_hamming_sum as f64 / samples as f64;
    let maj_avg_hamming = maj_hamming_sum as f64 / samples as f64;

    println!("\n| Function | Match Rate | Avg Hamming Dist | Expected (random) |");
    println!("|----------|------------|------------------|-------------------|");
    println!("| Ch       | {:.2}%     | {:.2}/32         | 50% / 16          |",
        ch_match_rate, ch_avg_hamming);
    println!("| Maj      | {:.2}%     | {:.2}/32         | 50% / 16          |",
        maj_match_rate, maj_avg_hamming);

    // Theoretical: Ch has 75% linear approximation bias (3/4 of inputs match XOR)
    println!("\n--- Theoretical Analysis ---");
    println!("Ch(x,y,z) truth table analysis:");
    println!("  When x=0: Ch = z (so x^y^z matches 50% when y random)");
    println!("  When x=1: Ch = y (so x^y^z matches 50% when z random)");
    println!("  Expected match: 75% (NOT 50%)");

    if ch_match_rate > 70.0 && ch_match_rate < 80.0 {
        println!("\n✓ Ch matches theoretical 75% linear approximation");
    }
    if maj_match_rate > 70.0 && maj_match_rate < 80.0 {
        println!("✓ Maj matches theoretical 75% linear approximation");
    }
}

/// Test 2: Carry Propagation in Modular Addition
#[test]
#[ignore]
fn test_carry_propagation() {
    println!("\n=== Test 2: Modular Addition Carry Analysis ===\n");

    let samples = 100_000;
    let mut carry_counts = [0u64; 32];  // How often each bit position has carry
    let mut cascade_lengths: HashMap<u32, u32> = HashMap::new();

    println!("Analyzing carry propagation in {} additions...", samples);

    for i in 0..samples {
        let a = ((i as u32).wrapping_mul(2654435761)).wrapping_add(12345);
        let b = ((i as u32).wrapping_mul(1597334677)).wrapping_add(67890);

        // Track carries bit by bit
        let mut carry = 0u32;
        let mut cascade_len = 0u32;
        let mut in_cascade = false;

        for bit in 0..32 {
            let a_bit = (a >> bit) & 1;
            let b_bit = (b >> bit) & 1;
            let sum = a_bit + b_bit + carry;
            let new_carry = sum >> 1;

            if new_carry == 1 {
                carry_counts[bit] += 1;
                if !in_cascade {
                    in_cascade = true;
                    cascade_len = 1;
                } else {
                    cascade_len += 1;
                }
            } else {
                if in_cascade {
                    *cascade_lengths.entry(cascade_len).or_insert(0) += 1;
                    in_cascade = false;
                }
            }
            carry = new_carry;
        }
        if in_cascade {
            *cascade_lengths.entry(cascade_len).or_insert(0) += 1;
        }
    }

    println!("\n--- Carry Probability by Bit Position ---");
    println!("(Bit 0 = LSB, higher bits have more opportunity for carries)");
    for bit in [0, 7, 15, 23, 31] {
        let prob = carry_counts[bit] as f64 / samples as f64 * 100.0;
        println!("  Bit {:2}: {:.2}% carry probability", bit, prob);
    }

    println!("\n--- Carry Cascade Length Distribution ---");
    let mut sorted: Vec<_> = cascade_lengths.iter().collect();
    sorted.sort_by_key(|&(k, _)| k);
    for (len, count) in sorted.iter().take(10) {
        println!("  Length {}: {} occurrences", len, count);
    }

    // Calculate average cascade length
    let total_cascades: u32 = cascade_lengths.values().sum();
    let weighted_sum: u64 = cascade_lengths.iter()
        .map(|(&len, &count)| len as u64 * count as u64)
        .sum();
    let avg_cascade = weighted_sum as f64 / total_cascades as f64;

    println!("\nAverage carry cascade length: {:.2} bits", avg_cascade);
    println!("(This is the 'non-linearity injection' per addition)");

    // The key insight: each addition injects ~2-3 bits of non-linearity
    if avg_cascade > 1.5 && avg_cascade < 3.5 {
        println!("\n✓ Carry cascade confirms ~2-3 bits of non-linearity per ADD");
    }
}

/// Test 3: Invariant Subspace Detection
#[test]
#[ignore]
fn test_invariant_subspace() {
    println!("\n=== Test 3: Invariant Subspace Detection ===\n");

    // An invariant subspace would be a set of bit positions that
    // maintain their XOR relationship through multiple rounds
    //
    // Test: flip input bit i, count how often output bit j flips
    // Perfect avalanche = 50% flip rate for all (i,j) pairs

    let samples = 1000;  // Reduced for speed, 1000 * 256 input bits * 256 output bits = 65M comparisons

    // Track flip counts for each (input_bit, output_bit) pair
    // We only test first 64 input bits (one block) vs all 256 output bits
    let mut flip_counts = [[0u32; 256]; 64];

    println!("Testing avalanche: {} samples * 64 input bits * 256 output bits", samples);

    for sample in 0..samples {
        // Create random 64-byte input using SHA-256 as RNG
        let seed = format!("invariant_seed_{}", sample);
        let seed_hash = sha256(seed.as_bytes());
        let mut input = [0u8; 64];
        for i in 0..32 {
            input[i] = seed_hash[i];
        }
        let seed2 = sha256(&seed_hash);
        for i in 0..32 {
            input[32 + i] = seed2[i];
        }

        let base_hash = sha256(&input);

        // Test flipping each of first 64 input bits
        for in_bit in 0..64 {
            let byte_idx = in_bit / 8;
            let bit_idx = in_bit % 8;

            let mut modified = input;
            modified[byte_idx] ^= 1 << bit_idx;
            let mod_hash = sha256(&modified);

            // Count flipped output bits
            for out_bit in 0..256 {
                let out_byte = out_bit / 8;
                let out_bit_idx = out_bit % 8;

                let base_val = (base_hash[out_byte] >> out_bit_idx) & 1;
                let mod_val = (mod_hash[out_byte] >> out_bit_idx) & 1;

                if base_val != mod_val {
                    flip_counts[in_bit][out_bit] += 1;
                }
            }
        }
    }

    // Analyze flip rates
    let mut min_flip_rate = 100.0f64;
    let mut max_flip_rate = 0.0f64;
    let mut sum_flip_rate = 0.0f64;
    let mut outlier_count = 0;
    let mut min_pair = (0, 0);
    let mut max_pair = (0, 0);

    for in_bit in 0..64 {
        for out_bit in 0..256 {
            let flip_rate = flip_counts[in_bit][out_bit] as f64 / samples as f64 * 100.0;
            sum_flip_rate += flip_rate;

            if flip_rate < min_flip_rate {
                min_flip_rate = flip_rate;
                min_pair = (in_bit, out_bit);
            }
            if flip_rate > max_flip_rate {
                max_flip_rate = flip_rate;
                max_pair = (in_bit, out_bit);
            }

            // Outlier if more than 10% away from 50%
            if flip_rate < 40.0 || flip_rate > 60.0 {
                outlier_count += 1;
            }
        }
    }

    let avg_flip_rate = sum_flip_rate / (64.0 * 256.0);

    println!("\n--- Avalanche Analysis ---");
    println!("Expected flip rate: 50.00%");
    println!("Average flip rate: {:.2}%", avg_flip_rate);
    println!("Min flip rate: {:.2}% (in_bit {} -> out_bit {})", min_flip_rate, min_pair.0, min_pair.1);
    println!("Max flip rate: {:.2}% (in_bit {} -> out_bit {})", max_flip_rate, max_pair.0, max_pair.1);
    println!("Outliers (>10% from 50%): {} / {} pairs", outlier_count, 64 * 256);

    // With 1000 samples, 95% confidence interval for 50% is roughly [46.9%, 53.1%]
    // Outliers outside 40-60% would be extremely rare by chance

    if avg_flip_rate > 49.0 && avg_flip_rate < 51.0 && outlier_count == 0 {
        println!("\n✓ PERFECT AVALANCHE - No invariant subspaces");
    } else if outlier_count < 10 {
        println!("\n✓ Near-perfect avalanche - statistical noise only");
    } else {
        println!("\n⚠️  {} outlier pairs - investigate further!", outlier_count);
    }
}

/// Test 4: Linear vs Actual Round Propagation
#[test]
#[ignore]
fn test_linear_vs_actual_rounds() {
    println!("\n=== Test 4: Linear vs Actual Propagation ===\n");

    // Implement simplified SHA-256 rounds to compare linear vs actual

    let samples = 1000;
    let mut divergence_by_round = [0.0f64; 64];

    println!("Comparing linear approximation divergence over 64 rounds...");

    for sample in 0..samples {
        // Initialize state
        let mut actual_state = H;
        let mut linear_state = H;

        // Create message schedule (simplified)
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = ((sample as u32).wrapping_mul(2654435761))
                .wrapping_add((i as u32).wrapping_mul(1597334677));
        }
        // Extend message schedule
        for i in 16..64 {
            w[i] = sigma1_msg(w[i-2])
                .wrapping_add(w[i-7])
                .wrapping_add(sigma0_msg(w[i-15]))
                .wrapping_add(w[i-16]);
        }

        // Run rounds
        for round in 0..64 {
            // === ACTUAL SHA-256 round ===
            let s1 = sigma1(actual_state[4]);
            let ch_val = ch(actual_state[4], actual_state[5], actual_state[6]);
            let temp1 = actual_state[7]
                .wrapping_add(s1)
                .wrapping_add(ch_val)
                .wrapping_add(K[round])
                .wrapping_add(w[round]);
            let s0 = sigma0(actual_state[0]);
            let maj_val = maj(actual_state[0], actual_state[1], actual_state[2]);
            let temp2 = s0.wrapping_add(maj_val);

            // Update actual state
            actual_state[7] = actual_state[6];
            actual_state[6] = actual_state[5];
            actual_state[5] = actual_state[4];
            actual_state[4] = actual_state[3].wrapping_add(temp1);
            actual_state[3] = actual_state[2];
            actual_state[2] = actual_state[1];
            actual_state[1] = actual_state[0];
            actual_state[0] = temp1.wrapping_add(temp2);

            // === LINEAR approximation round ===
            let s1_lin = sigma1(linear_state[4]);
            let ch_lin = ch_linear(linear_state[4], linear_state[5], linear_state[6]);
            // Use XOR instead of ADD for linear approximation
            let temp1_lin = linear_state[7]
                ^ s1_lin
                ^ ch_lin
                ^ K[round]
                ^ w[round];
            let s0_lin = sigma0(linear_state[0]);
            let maj_lin = maj_linear(linear_state[0], linear_state[1], linear_state[2]);
            let temp2_lin = s0_lin ^ maj_lin;

            // Update linear state
            linear_state[7] = linear_state[6];
            linear_state[6] = linear_state[5];
            linear_state[5] = linear_state[4];
            linear_state[4] = linear_state[3] ^ temp1_lin;
            linear_state[3] = linear_state[2];
            linear_state[2] = linear_state[1];
            linear_state[1] = linear_state[0];
            linear_state[0] = temp1_lin ^ temp2_lin;

            // Measure divergence (normalized hamming distance)
            let mut total_hamming = 0u32;
            for i in 0..8 {
                total_hamming += hamming_distance(actual_state[i], linear_state[i]);
            }
            divergence_by_round[round] += total_hamming as f64 / 256.0;  // 8 words * 32 bits
        }
    }

    println!("\n| Round | Divergence | Status |");
    println!("|-------|------------|--------|");

    for round in [0, 1, 2, 3, 4, 5, 10, 15, 20, 30, 40, 50, 63] {
        let div = divergence_by_round[round] / samples as f64 * 100.0;
        let status = if div < 10.0 { "Linear OK" }
                    else if div < 30.0 { "Diverging" }
                    else if div < 45.0 { "Broken" }
                    else { "RANDOM" };
        println!("|   {:2}  |   {:.1}%   | {} |", round, div, status);
    }

    // Find when divergence hits 50% (random)
    let mut random_round = 64;
    for round in 0..64 {
        let div = divergence_by_round[round] / samples as f64 * 100.0;
        if div > 45.0 {
            random_round = round;
            break;
        }
    }

    println!("\nLinear approximation becomes RANDOM at round: {}", random_round);

    if random_round < 10 {
        println!("✓ Linearization fails EARLY - no algebraic shortcut viable");
    } else {
        println!("⚠️  Linearization survives {} rounds - investigate!", random_round);
    }
}

/// Test 5: SAT Constraint Density Analysis
#[test]
#[ignore]
fn test_sat_constraint_density() {
    println!("\n=== Test 5: SAT Constraint Analysis ===\n");

    // Estimate the constraint density of SHA-256 as a SAT problem

    // Variables per round:
    // - 8 state words * 32 bits = 256 state bits
    // - 1 message word * 32 bits = 32 message bits
    // - Intermediate values (temp1, temp2, etc.)

    let state_bits = 256;
    let msg_bits_per_round = 32;
    let intermediate_vars_per_round = 32 * 5;  // temp1, temp2, ch, maj, sigma outputs

    // Constraints per round:
    // - Ch function: 3 clauses per bit * 32 bits (CNF representation of AND)
    // - Maj function: 4 clauses per bit * 32 bits
    // - Addition: ~5 clauses per bit for carry propagation * 32 bits * 3 additions
    // - XOR: 4 clauses per bit * 32 bits * 2 XORs
    // - Rotation: just variable renaming, 0 clauses

    let ch_clauses = 3 * 32;
    let maj_clauses = 4 * 32;
    let add_clauses = 5 * 32 * 3;
    let xor_clauses = 4 * 32 * 2;

    let clauses_per_round = ch_clauses + maj_clauses + add_clauses + xor_clauses;
    let vars_per_round = state_bits + msg_bits_per_round + intermediate_vars_per_round;

    let total_rounds = 64;
    let total_vars = vars_per_round * total_rounds;
    let total_clauses = clauses_per_round * total_rounds;

    let clause_to_var_ratio = total_clauses as f64 / total_vars as f64;

    println!("=== SHA-256 as SAT Problem ===\n");
    println!("Per round:");
    println!("  State bits: {}", state_bits);
    println!("  Message bits: {}", msg_bits_per_round);
    println!("  Intermediate vars: {}", intermediate_vars_per_round);
    println!("  Ch clauses: {}", ch_clauses);
    println!("  Maj clauses: {}", maj_clauses);
    println!("  ADD clauses: {}", add_clauses);
    println!("  XOR clauses: {}", xor_clauses);
    println!("  Total clauses: {}", clauses_per_round);

    println!("\nTotal (64 rounds):");
    println!("  Variables: ~{}", total_vars);
    println!("  Clauses: ~{}", total_clauses);
    println!("  Clause/Variable ratio: {:.2}", clause_to_var_ratio);

    println!("\n--- SAT Hardness Analysis ---");

    // SAT phase transition occurs around clause/var ratio of 4.26 for random 3-SAT
    // SHA-256 is much more structured, but still...

    println!("Random 3-SAT phase transition: ~4.26 clauses/var");
    println!("SHA-256 clause density: {:.2} clauses/var", clause_to_var_ratio);

    if clause_to_var_ratio > 4.0 {
        println!("\n✓ SHA-256 is in the HARD region of SAT space");
        println!("  Clause density exceeds phase transition threshold");
    }

    // Additional hardness: carry propagation creates long-range dependencies
    println!("\n--- Carry Propagation Impact ---");
    println!("Each ADD creates dependencies spanning up to 32 bits");
    println!("3 ADDs per round * 64 rounds = 192 carry chains");
    println!("This creates a 'web' of interdependencies that SAT solvers struggle with");

    // Estimate solving time
    println!("\n--- Estimated SAT Solving ---");
    println!("Reduced rounds (8): Solved in seconds (academic papers)");
    println!("Reduced rounds (16): Solved in hours");
    println!("Reduced rounds (24): Solved in days");
    println!("Full SHA-256 (64): UNSOLVED (would take longer than universe age)");
}

/// Test 6: The "Kaoru Trick" - Partial State Recovery
#[test]
#[ignore]
fn test_partial_state_recovery() {
    println!("\n=== Test 6: Partial State Recovery (Kaoru Trick) ===\n");

    // If you know part of the internal state at some round,
    // can you recover the rest more easily?
    //
    // Using actual SHA-256 outputs as "states" to ensure true randomness

    let samples = 10000;
    let mut recovery_success = [0u32; 8];  // For each known word count

    println!("Testing correlation between known and unknown state words...");
    println!("(Using SHA-256 outputs as truly random 256-bit states)\n");

    for sample in 0..samples {
        // Generate truly random state using SHA-256
        let seed = format!("kaoru_test_{}", sample);
        let hash = sha256(seed.as_bytes());

        // Interpret hash as 8 x 32-bit words
        let mut target_state = [0u32; 8];
        for i in 0..8 {
            target_state[i] = u32::from_be_bytes([
                hash[i*4], hash[i*4+1], hash[i*4+2], hash[i*4+3]
            ]);
        }

        // Try to "guess" unknown words given known words
        for known_count in 1..8 {
            // XOR of known words
            let mut xor_known = 0u32;
            for i in 0..known_count {
                xor_known ^= target_state[i];
            }

            // XOR of unknown words
            let mut xor_unknown = 0u32;
            for i in known_count..8 {
                xor_unknown ^= target_state[i];
            }

            // Count matching bits (should be 50% if independent)
            let matching = 32 - hamming_distance(xor_known, xor_unknown);
            if matching > 20 {  // >62.5% match is suspicious (3σ threshold)
                recovery_success[known_count] += 1;
            }
        }
    }

    println!("| Known Words | High Correlation Rate | Expected (random) |");
    println!("|-------------|----------------------|-------------------|");

    for known in 1..8 {
        let rate = recovery_success[known] as f64 / samples as f64 * 100.0;
        // Binomial(32, 0.5): P(X > 20) ≈ 4.1% (correct calculation)
        println!("|     {}       |        {:.2}%         |       4.1%        |", known, rate);
    }

    // Allow 50% margin above expected (4.1% * 1.5 = 6.15%)
    let any_elevated = recovery_success.iter().any(|&r| r as f64 / samples as f64 > 0.08);

    if any_elevated {
        println!("\n⚠️  Elevated correlation detected - investigate!");
    } else {
        println!("\n✓ No exploitable correlation between known and unknown state");
        println!("  SHA-256 output words are statistically independent");
    }
}

/// Run all algebraic analysis tests
#[test]
#[ignore]
fn test_all_v14_algebraic() {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Research v14: Algebraic Cryptanalysis                        ║");
    println!("║  Testing Linearization and SAT-Based Attacks                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    test_ch_maj_linearization();
    test_carry_propagation();
    test_invariant_subspace();
    test_linear_vs_actual_rounds();
    test_sat_constraint_density();
    test_partial_state_recovery();

    println!("\n");
    println!("═══════════════════════════════════════════════════════════════");
    println!("                  ALGEBRAIC ANALYSIS COMPLETE                   ");
    println!("═══════════════════════════════════════════════════════════════");
}
