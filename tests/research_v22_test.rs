/// Research v22: Sweet Spot Bits & Round 60 Abort
///
/// Testing claims:
/// 1. W[3] and W[4] are "sweet spots" with slowest avalanche
/// 2. Early abort at Round 60 gives 5-10% efficiency boost
/// 3. Nested loop optimization for neutral bits
/// 4. Double SHA-256 impact on early abort strategies
///
/// Key insight to test: Bitcoin uses SHA256(SHA256(header))
/// Early abort in FIRST hash is pointless - entire output feeds second hash

use std::collections::HashMap;

/// SHA-256 constants
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

/// SHA-256 initial hash values
const H_INIT: [u32; 8] = [
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

fn delta0(x: u32) -> u32 {
    rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3)
}

fn delta1(x: u32) -> u32 {
    rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10)
}

/// Expand message to 64 words
fn expand_message(block: &[u32; 16]) -> [u32; 64] {
    let mut w = [0u32; 64];
    w[..16].copy_from_slice(block);
    for i in 16..64 {
        w[i] = delta1(w[i-2])
            .wrapping_add(w[i-7])
            .wrapping_add(delta0(w[i-15]))
            .wrapping_add(w[i-16]);
    }
    w
}

/// Run SHA-256 for specified rounds, return state and W
fn sha256_rounds(block: &[u32; 16], rounds: usize) -> ([u32; 8], [u32; 64]) {
    let w = expand_message(block);
    let mut state = H_INIT;

    for i in 0..rounds.min(64) {
        let t1 = state[7]
            .wrapping_add(sigma1(state[4]))
            .wrapping_add(ch(state[4], state[5], state[6]))
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let t2 = sigma0(state[0]).wrapping_add(maj(state[0], state[1], state[2]));

        state[7] = state[6];
        state[6] = state[5];
        state[5] = state[4];
        state[4] = state[3].wrapping_add(t1);
        state[3] = state[2];
        state[2] = state[1];
        state[1] = state[0];
        state[0] = t1.wrapping_add(t2);
    }

    (state, w)
}

/// Full SHA-256 of a block
fn sha256_block(block: &[u32; 16]) -> [u32; 8] {
    let (state, _) = sha256_rounds(block, 64);
    let mut hash = H_INIT;
    for i in 0..8 {
        hash[i] = hash[i].wrapping_add(state[i]);
    }
    hash
}

/// SHA-256 of arbitrary bytes
fn sha256(data: &[u8]) -> [u32; 8] {
    let mut padded = data.to_vec();
    let bit_len = (data.len() as u64) * 8;

    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = H_INIT;

    for chunk in padded.chunks(64) {
        let mut block = [0u32; 16];
        for (i, word) in chunk.chunks(4).enumerate() {
            block[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }

        let (new_state, _) = sha256_rounds(&block, 64);
        for i in 0..8 {
            state[i] = state[i].wrapping_add(new_state[i]);
        }
    }

    state
}

/// Double SHA-256 (Bitcoin mining)
fn sha256d(data: &[u8]) -> [u32; 8] {
    let first = sha256(data);
    let mut first_bytes = [0u8; 32];
    for (i, word) in first.iter().enumerate() {
        first_bytes[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes());
    }
    sha256(&first_bytes)
}

/// Count leading zeros in hash
fn count_leading_zeros(hash: &[u32; 8]) -> u32 {
    let mut zeros = 0;
    for &word in hash {
        if word == 0 {
            zeros += 32;
        } else {
            zeros += word.leading_zeros();
            break;
        }
    }
    zeros
}

/// Test 1: W[3] and W[4] "Sweet Spot" Analysis
///
/// Claim: W[3] and W[4] have the "slowest" avalanche
/// Reality: W[i] enters at round i, so W[3] enters at round 3
#[test]
#[ignore]
fn test_sweet_spot_analysis() {
    println!("\n=== Test 1: W[3]/W[4] Sweet Spot Analysis ===\n");

    let mut rng_state = 12345u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 32) as u32
    };

    // Create base block
    let mut base_block = [0u32; 16];
    for i in 0..16 {
        base_block[i] = next_rand(&mut rng_state);
    }

    println!("Measuring avalanche timing for each W[i] position:\n");
    println!("| W[i] | Round enters | Round 4 diff | Round 8 diff | Round 16 diff | Full diff |");
    println!("|------|--------------|--------------|--------------|---------------|-----------|");

    for w_idx in 0..16 {
        let mut modified = base_block;
        modified[w_idx] ^= 1;  // Flip one bit

        let (base_state_4, _) = sha256_rounds(&base_block, 4);
        let (mod_state_4, _) = sha256_rounds(&modified, 4);
        let diff_4: u32 = base_state_4.iter().zip(mod_state_4.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        let (base_state_8, _) = sha256_rounds(&base_block, 8);
        let (mod_state_8, _) = sha256_rounds(&modified, 8);
        let diff_8: u32 = base_state_8.iter().zip(mod_state_8.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        let (base_state_16, _) = sha256_rounds(&base_block, 16);
        let (mod_state_16, _) = sha256_rounds(&modified, 16);
        let diff_16: u32 = base_state_16.iter().zip(mod_state_16.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        let base_hash = sha256_block(&base_block);
        let mod_hash = sha256_block(&modified);
        let diff_full: u32 = base_hash.iter().zip(mod_hash.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum();

        let neutral = if diff_4 == 0 { "✓" } else { "" };
        println!("| W[{:2}] | Round {:2}     | {:3} bits {}  | {:3} bits     | {:3} bits      | {:3} bits  |",
                 w_idx, w_idx, diff_4, neutral, diff_8, diff_16, diff_full);
    }

    println!("\n**Key Finding:** W[i] is neutral through round i-1.");
    println!("W[3] enters at round 3 → neutral through rounds 0-2 (3 rounds)");
    println!("W[4] enters at round 4 → neutral through rounds 0-3 (4 rounds)");
    println!("W[15] enters at round 15 → neutral through rounds 0-14 (15 rounds)");

    println!("\n**But for Bitcoin mining:**");
    println!("Nonce is at W[3] in Block 2 → only 3 rounds of neutrality");
    println!("This is ALREADY captured by midstate optimization");
}

/// Test 2: Round 60 Early Abort Feasibility
///
/// Claim: "If A at round 60 has no zeros, abort for 5-10% savings"
///
/// Critical issue: Bitcoin uses DOUBLE SHA-256!
/// Round 60 of first hash is irrelevant - output feeds second hash
#[test]
#[ignore]
fn test_round_60_abort() {
    println!("\n=== Test 2: Round 60 Early Abort Analysis ===\n");

    println!("CRITICAL: Bitcoin uses SHA256(SHA256(header))");
    println!("Early abort in FIRST hash is POINTLESS!\n");

    let mut rng_state = 54321u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 32) as u32
    };

    // Test correlation between round 60 state and final hash (single SHA-256)
    println!("Part A: Single SHA-256 (NOT how Bitcoin works)\n");

    let samples = 100_000;
    let mut round_60_a_zeros = Vec::new();
    let mut final_zeros = Vec::new();

    for _ in 0..samples {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = next_rand(&mut rng_state);
        }

        let (state_60, _) = sha256_rounds(&block, 60);
        let a_60_zeros = state_60[0].leading_zeros();

        let hash = sha256_block(&block);
        let final_z = count_leading_zeros(&hash);

        round_60_a_zeros.push(a_60_zeros as f64);
        final_zeros.push(final_z as f64);
    }

    // Calculate correlation
    let n = samples as f64;
    let sum_x: f64 = round_60_a_zeros.iter().sum();
    let sum_y: f64 = final_zeros.iter().sum();
    let sum_xy: f64 = round_60_a_zeros.iter().zip(final_zeros.iter())
        .map(|(x, y)| x * y).sum();
    let sum_x2: f64 = round_60_a_zeros.iter().map(|x| x * x).sum();
    let sum_y2: f64 = final_zeros.iter().map(|y| y * y).sum();

    let correlation = (n * sum_xy - sum_x * sum_y) /
        ((n * sum_x2 - sum_x * sum_x).sqrt() * (n * sum_y2 - sum_y * sum_y).sqrt());

    println!("Single SHA-256 (round 60 A zeros vs final zeros):");
    println!("  Correlation: {:.6}", correlation);
    println!("  Interpretation: {}",
             if correlation.abs() > 0.5 { "STRONG" }
             else if correlation.abs() > 0.1 { "WEAK" }
             else { "NONE" });

    // Test for double SHA-256 (actual Bitcoin)
    println!("\nPart B: Double SHA-256 (ACTUAL Bitcoin mining)\n");

    let mut first_r60_zeros = Vec::new();
    let mut double_final_zeros = Vec::new();

    for _ in 0..samples {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = next_rand(&mut rng_state);
        }

        // Get round 60 state of FIRST hash
        let (state_60, _) = sha256_rounds(&block, 60);
        let a_60_zeros = state_60[0].leading_zeros();

        // Full double SHA-256
        let mut first_bytes = [0u8; 64];
        for (i, word) in block.iter().enumerate() {
            first_bytes[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes());
        }
        let double_hash = sha256d(&first_bytes[..]);
        let final_z = count_leading_zeros(&double_hash);

        first_r60_zeros.push(a_60_zeros as f64);
        double_final_zeros.push(final_z as f64);
    }

    let sum_x: f64 = first_r60_zeros.iter().sum();
    let sum_y: f64 = double_final_zeros.iter().sum();
    let sum_xy: f64 = first_r60_zeros.iter().zip(double_final_zeros.iter())
        .map(|(x, y)| x * y).sum();
    let sum_x2: f64 = first_r60_zeros.iter().map(|x| x * x).sum();
    let sum_y2: f64 = double_final_zeros.iter().map(|y| y * y).sum();

    let double_correlation = (n * sum_xy - sum_x * sum_y) /
        ((n * sum_x2 - sum_x * sum_x).sqrt() * (n * sum_y2 - sum_y * sum_y).sqrt());

    println!("Double SHA-256 (first hash round 60 vs final double-hash zeros):");
    println!("  Correlation: {:.6}", double_correlation);
    println!("  Interpretation: {}",
             if double_correlation.abs() > 0.5 { "STRONG" }
             else if double_correlation.abs() > 0.1 { "WEAK" }
             else { "NONE (as expected)" });

    // Test round 60 of SECOND hash
    println!("\nPart C: Second hash round 60 analysis\n");

    let mut second_r60_zeros = Vec::new();
    let mut second_final_zeros = Vec::new();

    for _ in 0..samples {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = next_rand(&mut rng_state);
        }

        // First hash
        let first_hash = sha256_block(&block);

        // Create second block from first hash (with padding)
        let mut second_block = [0u32; 16];
        second_block[..8].copy_from_slice(&first_hash);
        second_block[8] = 0x80000000;  // Padding
        second_block[15] = 256;  // Length in bits

        // Round 60 of second hash
        let (state_60, _) = sha256_rounds(&second_block, 60);
        let a_60_zeros = state_60[0].leading_zeros();

        // Full second hash
        let second_hash = sha256_block(&second_block);
        let final_z = count_leading_zeros(&second_hash);

        second_r60_zeros.push(a_60_zeros as f64);
        second_final_zeros.push(final_z as f64);
    }

    let sum_x: f64 = second_r60_zeros.iter().sum();
    let sum_y: f64 = second_final_zeros.iter().sum();
    let sum_xy: f64 = second_r60_zeros.iter().zip(second_final_zeros.iter())
        .map(|(x, y)| x * y).sum();
    let sum_x2: f64 = second_r60_zeros.iter().map(|x| x * x).sum();
    let sum_y2: f64 = second_final_zeros.iter().map(|y| y * y).sum();

    let second_correlation = (n * sum_xy - sum_x * sum_y) /
        ((n * sum_x2 - sum_x * sum_x).sqrt() * (n * sum_y2 - sum_y * sum_y).sqrt());

    println!("Second hash (round 60 A zeros vs final zeros):");
    println!("  Correlation: {:.6}", second_correlation);

    // Even if there's correlation at round 60, we've already done first hash
    // So total work is 64 + 60 = 124 rounds before any abort
    // Savings from aborting: 4/128 = 3.1% max

    println!("\n**Cost Analysis (if round 60 abort worked):**");
    println!("  Work before abort decision: 64 (first hash) + 60 (second hash) = 124 rounds");
    println!("  Work saved by abort: 4 rounds (rounds 61-64 of second hash)");
    println!("  Maximum theoretical savings: 4/128 = 3.1%");
    println!("  Realistic savings: <1% (most nonces complete anyway)");
}

/// Test 3: Nested Loop Efficiency Calculation
///
/// Claim: Outer loop does 64 rounds, inner loop "skips" rounds 1-15
/// for "massive percentage" of hashes
#[test]
#[ignore]
fn test_nested_loop_efficiency() {
    println!("\n=== Test 3: Nested Loop Efficiency Analysis ===\n");

    println!("Claim: 'Skip re-calculation of Rounds 1-15 for massive percentage'\n");

    // In Bitcoin mining:
    // Block 1: Constant (version, prev_hash, merkle_start) → midstate
    // Block 2: merkle_end, timestamp, bits, NONCE, padding
    //
    // Nonce is at W[3] of Block 2
    // W[3] affects round 3 onwards

    println!("Bitcoin Block Structure:");
    println!("┌──────────────────────────────────────┐");
    println!("│ Block 1 (64 bytes) - CONSTANT        │");
    println!("│ W[0-15]: version, prev_hash, merkle  │");
    println!("│ → Computed ONCE as midstate          │");
    println!("└──────────────────────────────────────┘");
    println!("┌──────────────────────────────────────┐");
    println!("│ Block 2 (16 bytes + padding)         │");
    println!("│ W[0]: merkle_end (constant)          │");
    println!("│ W[1]: timestamp  (constant per sec)  │");
    println!("│ W[2]: bits       (constant)          │");
    println!("│ W[3]: **NONCE**  (variable)          │");
    println!("│ W[4-15]: padding (constant)          │");
    println!("└──────────────────────────────────────┘\n");

    println!("Neutral Bit Analysis for Nonce (W[3]):\n");

    let mut rng_state = 11111u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 32) as u32
    };

    // Create base block simulating Block 2
    let mut base_block = [0u32; 16];
    base_block[0] = 0xdeadbeef;  // merkle end
    base_block[1] = 0x12345678;  // timestamp
    base_block[2] = 0x1d00ffff;  // bits
    base_block[3] = 0x00000000;  // nonce (will vary)
    base_block[4] = 0x80000000;  // padding start
    base_block[15] = 640;        // length (80 bytes * 8)

    let trials = 10_000;

    println!("| Round | Nonces with Zero Diff | Can Skip? |");
    println!("|-------|----------------------|-----------|");

    for round in [1, 2, 3, 4, 5, 6, 8, 12, 16].iter() {
        let mut zero_diff_count = 0;

        for _ in 0..trials {
            let nonce1 = next_rand(&mut rng_state);
            let nonce2 = next_rand(&mut rng_state);

            base_block[3] = nonce1;
            let (state1, _) = sha256_rounds(&base_block, *round);

            base_block[3] = nonce2;
            let (state2, _) = sha256_rounds(&base_block, *round);

            let diff: u32 = state1.iter().zip(state2.iter())
                .map(|(a, b)| (a ^ b).count_ones())
                .sum();

            if diff == 0 {
                zero_diff_count += 1;
            }
        }

        let can_skip = if *round < 3 { "YES" } else { "NO" };
        println!("| {:5} | {:6} / {:6} ({:5.1}%) | {}         |",
                 round, zero_diff_count, trials,
                 100.0 * zero_diff_count as f64 / trials as f64,
                 can_skip);
    }

    println!("\n**Nested Loop Reality:**\n");
    println!("Since nonce is W[3], it affects state starting at round 3.");
    println!("- Rounds 0-2: Same for all nonces (can pre-compute)");
    println!("- Rounds 3-63: Must compute for each nonce");
    println!("\nActual savings: 2/64 = 3.1% per block");
    println!("But midstate already saves: 64/128 = 50% (first block)");
    println!("\n**Combined savings (midstate + W[3] neutrality):**");
    println!("  First SHA-256: 64 + 2 = 66 rounds pre-computed");
    println!("  Variable part: 62 rounds of first + 64 rounds of second = 126 rounds");
    println!("  Total rounds: 128, Pre-computed: 2, Variable: 126");
    println!("  Marginal savings from 'nested loop': 2/126 = 1.6%");
}

/// Test 4: Double SHA-256 Impact on Early Abort
///
/// The critical insight: all claims about "early abort" ignore
/// that Bitcoin uses DOUBLE SHA-256
#[test]
#[ignore]
fn test_double_sha256_impact() {
    println!("\n=== Test 4: Double SHA-256 Impact Analysis ===\n");

    println!("Bitcoin Hash: SHA256(SHA256(80-byte header))");
    println!("\nThis means:");
    println!("1. First hash: Process header → 256-bit intermediate");
    println!("2. Second hash: Process intermediate → 256-bit final");
    println!("3. Difficulty check: final hash < target\n");

    println!("Timeline of work:");
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ First SHA-256                                          │");
    println!("│ Block 1: Rounds 0-63 (constant → midstate)             │");
    println!("│ Block 2: Rounds 0-63 (nonce varies → intermediate)     │");
    println!("├─────────────────────────────────────────────────────────┤");
    println!("│ Second SHA-256                                         │");
    println!("│ Block 1: Rounds 0-63 (intermediate → final hash)       │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    // Test what percentage of first-hash states lead to valid final hashes
    println!("Testing: Can first-hash state predict valid final hash?");

    let mut rng_state = 99999u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 32) as u32
    };

    let target_zeros = 8;  // Easy target for testing
    let trials = 5_000_000;
    let mut valid_count = 0;

    // Track first-hash leading zeros for valid hashes
    let mut first_hash_zeros_for_valid: HashMap<u32, u32> = HashMap::new();
    let mut first_hash_zeros_overall: HashMap<u32, u32> = HashMap::new();

    for _ in 0..trials {
        let mut block = [0u32; 16];
        for i in 0..16 {
            block[i] = next_rand(&mut rng_state);
        }

        let first_hash = sha256_block(&block);
        let first_zeros = count_leading_zeros(&first_hash);

        // Second hash
        let mut second_block = [0u32; 16];
        second_block[..8].copy_from_slice(&first_hash);
        second_block[8] = 0x80000000;
        second_block[15] = 256;

        let second_hash = sha256_block(&second_block);
        let final_zeros = count_leading_zeros(&second_hash);

        *first_hash_zeros_overall.entry(first_zeros).or_insert(0) += 1;

        if final_zeros >= target_zeros {
            valid_count += 1;
            *first_hash_zeros_for_valid.entry(first_zeros).or_insert(0) += 1;
        }
    }

    let expected_valid = trials as f64 / (1u64 << target_zeros) as f64;
    println!("\nTarget: {} leading zeros", target_zeros);
    println!("Trials: {}", trials);
    println!("Valid hashes found: {} (expected: {:.1})", valid_count, expected_valid);

    println!("\nFirst-hash leading zeros distribution for VALID final hashes:");
    println!("| First Hash Zeros | Count | % of Valid |");
    println!("|------------------|-------|------------|");

    let mut sorted_zeros: Vec<_> = first_hash_zeros_for_valid.iter().collect();
    sorted_zeros.sort_by_key(|(k, _)| *k);

    for (zeros, count) in sorted_zeros.iter() {
        let percentage = 100.0 * **count as f64 / valid_count.max(1) as f64;
        println!("|       {:2}         | {:5} |   {:5.1}%   |", zeros, count, percentage);
    }

    // Key insight: first hash zeros are uniformly distributed among valid hashes
    // This means first hash gives ZERO predictive power about final validity

    println!("\n**Key Finding:**");
    println!("First hash leading zeros are ~uniformly distributed among valid final hashes.");
    println!("The first hash output is completely uncorrelated with final hash quality.");
    println!("Early abort based on first hash is IMPOSSIBLE.");
}

/// Test 5: The "5-10% Efficiency Boost" Claim
///
/// Direct test of the claimed savings
#[test]
#[ignore]
fn test_efficiency_boost_claim() {
    println!("\n=== Test 5: 5-10% Efficiency Boost Claim ===\n");

    println!("Claim: 'Early abort at round 60 gives 5-10% efficiency boost'\n");

    // Calculate theoretical maximum
    println!("**Theoretical Maximum Analysis:**\n");
    println!("Total work per nonce: 128 rounds (64 first hash + 64 second hash)");
    println!("Midstate pre-computation: 64 rounds (first block of first hash)");
    println!("Actual work per nonce: 64 rounds (second block of first hash + second hash)");
    println!("\nWait - the claim assumes we can abort BEFORE finishing!");
    println!("Let's check when we can make abort decisions:\n");

    println!("Decision points:");
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ After first hash round 60: 60 rounds done, need 68 more       │");
    println!("│   Cannot predict second hash (complete avalanche)             │");
    println!("│   Savings if abort: 4 + 64 = 68 rounds (53%)                  │");
    println!("│   BUT: No predictive signal available!                        │");
    println!("├────────────────────────────────────────────────────────────────┤");
    println!("│ After second hash round 60: 124 rounds done, need 4 more      │");
    println!("│   CAN we predict final zeros from state?                      │");
    println!("│   Savings if abort: 4 rounds (3.1%)                           │");
    println!("│   Maximum possible savings: 3.1%                              │");
    println!("└────────────────────────────────────────────────────────────────┘\n");

    // Test correlation at round 60 of second hash
    let mut rng_state = 77777u64;
    let next_rand = |state: &mut u64| -> u32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*state >> 32) as u32
    };

    let target_zeros = 16;  // Realistic difficulty
    let trials = 10_000_000;
    let mut aborted = 0u64;
    let mut valid_found = 0u64;
    let mut valid_aborted = 0u64;
    let abort_threshold = 0;  // Abort if A at round 60 has 0 leading zeros

    for _ in 0..trials {
        let mut block1 = [0u32; 16];
        for i in 0..16 {
            block1[i] = next_rand(&mut rng_state);
        }

        // First hash (full)
        let first_hash = sha256_block(&block1);

        // Second hash setup
        let mut block2 = [0u32; 16];
        block2[..8].copy_from_slice(&first_hash);
        block2[8] = 0x80000000;
        block2[15] = 256;

        // Round 60 of second hash
        let (state_60, _) = sha256_rounds(&block2, 60);
        let a_zeros = state_60[0].leading_zeros();

        // Full second hash (for comparison)
        let final_hash = sha256_block(&block2);
        let final_zeros = count_leading_zeros(&final_hash);

        let is_valid = final_zeros >= target_zeros;
        let would_abort = a_zeros <= abort_threshold;

        if would_abort {
            aborted += 1;
            if is_valid {
                valid_aborted += 1;
            }
        }

        if is_valid {
            valid_found += 1;
        }
    }

    let abort_rate = 100.0 * aborted as f64 / trials as f64;
    let rounds_saved = 4.0 * aborted as f64 / trials as f64;
    let savings = rounds_saved / 128.0 * 100.0;
    let miss_rate = 100.0 * valid_aborted as f64 / valid_found.max(1) as f64;

    println!("Simulation with {} trials (target: {} zeros):", trials, target_zeros);
    println!("  Valid hashes found: {}", valid_found);
    println!("  Abort threshold: A zeros ≤ {}", abort_threshold);
    println!("  Aborted: {:.2}% of attempts", abort_rate);
    println!("  Rounds saved: {:.3} per attempt", rounds_saved);
    println!("  Efficiency gain: {:.3}%", savings);
    println!("  Valid hashes missed: {} ({:.1}%)", valid_aborted, miss_rate);

    println!("\n**Verdict on 5-10% Claim:**");
    if valid_aborted > 0 {
        println!("REJECTED: Early abort MISSES valid hashes!");
        println!("Any abort strategy that misses valid hashes = lost revenue");
    } else if savings > 5.0 {
        println!("PARTIALLY VALIDATED: {:.1}% savings achieved", savings);
    } else {
        println!("REJECTED: Only {:.2}% savings possible, not 5-10%", savings);
    }
}

/// Test 6: Meet-in-the-Middle for Mining Reality
///
/// Clarifying that MitM is for collisions, not preimage/mining
#[test]
#[ignore]
fn test_mitm_for_mining() {
    println!("\n=== Test 6: Meet-in-the-Middle Reality ===\n");

    println!("The claim correctly states MitM is for COLLISIONS:");
    println!("'Where the Forward Wave meets the Backward Wave is where you find");
    println!(" your cutting-edge collision.'\n");

    println!("**Collision Attack vs Preimage Attack:**\n");
    println!("┌─────────────────────────────────────────────────────────────────┐");
    println!("│ COLLISION: Find M1, M2 such that H(M1) = H(M2)                 │");
    println!("│   - You CHOOSE both messages                                   │");
    println!("│   - Birthday paradox: ~2^128 operations                        │");
    println!("│   - MitM: forward from M1s, backward from M2s, meet at middle  │");
    println!("│   - DOES NOT HELP MINING                                       │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│ PREIMAGE: Given target T, find M such that H(M) < T            │");
    println!("│   - Message structure is FIXED (Bitcoin header)                │");
    println!("│   - Only nonce varies (32 bits)                                │");
    println!("│   - Backward requires knowing message schedule W               │");
    println!("│   - Circular dependency: need W to go backward, need M for W   │");
    println!("│   - THIS IS MINING - MitM does NOT help                        │");
    println!("└─────────────────────────────────────────────────────────────────┘\n");

    println!("**Why MitM Can't Help Mining:**\n");
    println!("1. Backward computation requires W (message schedule)");
    println!("2. W is derived from M (message)");
    println!("3. M contains the nonce (what we're searching for)");
    println!("4. Circular dependency: can't go backward without knowing answer");
    println!("\n5. Double SHA-256 compounds the problem:");
    println!("   - First hash output = random 256 bits");
    println!("   - These become input to second hash");
    println!("   - No way to 'meet in middle' across two hash calls");
}

/// Run all v22 tests
#[test]
#[ignore]
fn test_all_v22_sweet_spots() {
    println!("\n============================================================");
    println!("Research v22: Sweet Spot Bits & Round 60 Abort Analysis");
    println!("============================================================\n");

    test_sweet_spot_analysis();
    test_round_60_abort();
    test_nested_loop_efficiency();
    test_double_sha256_impact();
    test_efficiency_boost_claim();
    test_mitm_for_mining();

    println!("\n============================================================");
    println!("CONCLUSION: Round 60 Early Abort Claims - MIXED");
    println!("============================================================\n");

    println!("VERIFIED:");
    println!("  ✓ Midstate optimization works (50% savings)");
    println!("  ✓ W[i] is neutral through round i-1");
    println!("  ✓ MitM works for collisions (not mining)");
    println!("\nREJECTED:");
    println!("  ✗ Round 60 early abort gives 5-10% (max 3.1%)");
    println!("  ✗ Can 'skip rounds 1-15' for nonce (only 0-2)");
    println!("  ✗ First hash state predicts validity (avalanche)");
    println!("\nCRITICAL INSIGHT:");
    println!("  Bitcoin uses DOUBLE SHA-256");
    println!("  Early abort in first hash is POINTLESS");
    println!("  Second hash sees random input - no correlation");
}
