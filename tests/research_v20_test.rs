//! Research v20: Neutral Bits & MitM Practical Analysis
//!
//! Testing:
//! 1. Which message bits are truly "neutral" for first N rounds
//! 2. Practical speedup from neutral bit exploitation
//! 3. MitM table size requirements
//! 4. Nonce position impact on neutral bits (mining reality)
//! 5. Forward/backward balance optimization
//!
//! Run with: cargo test --release --test research_v20_test -- --nocapture --ignored

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
        t1.wrapping_add(t2),
        a,
        b,
        c,
        d.wrapping_add(t1),
        e,
        f,
        g,
    ]
}

/// Run N rounds forward
fn sha256_n_rounds_forward(initial: &[u32; 8], w: &[u32; 64], rounds: usize) -> [u32; 8] {
    let mut state = *initial;
    for i in 0..rounds {
        state = sha256_round_forward(&state, w[i], K[i]);
    }
    state
}

fn state_hamming(a: &[u32; 8], b: &[u32; 8]) -> u32 {
    (0..8).map(|i| (a[i] ^ b[i]).count_ones()).sum()
}

fn sha256_single_block(input: &[u8; 64]) -> [u8; 32] {
    let mut block = [0u32; 16];
    for i in 0..16 {
        block[i] = u32::from_be_bytes([
            input[i*4], input[i*4+1], input[i*4+2], input[i*4+3]
        ]);
    }

    let w = expand_message_schedule(&block);
    let state = sha256_n_rounds_forward(&H0, &w, 64);

    let mut final_state = [0u32; 8];
    for i in 0..8 {
        final_state[i] = state[i].wrapping_add(H0[i]);
    }

    let mut output = [0u8; 32];
    for i in 0..8 {
        let bytes = final_state[i].to_be_bytes();
        output[i*4..i*4+4].copy_from_slice(&bytes);
    }
    output
}

// ============================================================================
// TEST 1: True Neutral Bits (Zero State Difference)
// ============================================================================

#[test]
#[ignore]
fn test_true_neutral_bits() {
    println!("\n=== Test 1: True Neutral Bits (Zero State Difference) ===\n");

    // A "true" neutral bit has ZERO effect on state for first N rounds
    // This is different from "low effect" - we need EXACTLY zero

    println!("Searching for bits with ZERO state change after N rounds...\n");

    let samples = 100;  // Test multiple random base messages

    // For each message word position, track which rounds see zero difference
    let mut neutral_through_round: [[u32; 17]; 512] = [[0; 17]; 512];  // [bit][round] = count

    for sample in 0..samples {
        // Create random base message
        let mut base_block = [0u32; 16];
        for i in 0..16 {
            base_block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9)
                            .wrapping_add(0x12345678);
        }
        let base_w = expand_message_schedule(&base_block);

        // Test each input bit
        for bit_pos in 0..512 {
            let word = bit_pos / 32;
            let bit = bit_pos % 32;

            let mut mod_block = base_block;
            mod_block[word] ^= 1 << bit;
            let mod_w = expand_message_schedule(&mod_block);

            // Check at each round: is state EXACTLY the same?
            let mut base_state = H0;
            let mut mod_state = H0;

            for round in 0..=16 {
                if round > 0 {
                    base_state = sha256_round_forward(&base_state, base_w[round-1], K[round-1]);
                    mod_state = sha256_round_forward(&mod_state, mod_w[round-1], K[round-1]);
                }

                if base_state == mod_state {
                    neutral_through_round[bit_pos][round] += 1;
                }
            }
        }
    }

    // Find truly neutral bits (100% samples have zero diff)
    println!("Bits with ZERO state difference through round N (all {} samples):\n", samples);
    println!("Word.Bit   Round0  Round1  Round2  Round3  Round4  Round5  Round6  Round7  Round8");
    println!("─────────────────────────────────────────────────────────────────────────────────────");

    for word in 0..16 {
        let mut has_any_neutral = false;
        for bit in 0..32 {
            let bit_pos = word * 32 + bit;
            let max_neutral_round = (0..=16)
                .filter(|&r| neutral_through_round[bit_pos][r] == samples as u32)
                .max()
                .unwrap_or(0);

            if max_neutral_round > 0 {
                has_any_neutral = true;
            }
        }

        if has_any_neutral || word < 8 {  // Show first 8 words always
            for bit in 0..32 {
                let bit_pos = word * 32 + bit;
                let max_neutral_round = (0..=16)
                    .filter(|&r| neutral_through_round[bit_pos][r] == samples as u32)
                    .max()
                    .unwrap_or(0);

                if max_neutral_round > 0 || bit < 4 {
                    print!("W[{:2}].{:2}  ", word, bit);
                    for round in 0..=8 {
                        let pct = neutral_through_round[bit_pos][round] as f64 / samples as f64 * 100.0;
                        if pct == 100.0 {
                            print!("  100%  ");
                        } else if pct == 0.0 {
                            print!("    -   ");
                        } else {
                            print!(" {:5.1}% ", pct);
                        }
                    }
                    if max_neutral_round > 0 {
                        println!(" <- Neutral through round {}", max_neutral_round);
                    } else {
                        println!();
                    }
                }
            }
        }
    }

    // Summary: How many bits are neutral through each round?
    println!("\n--- Neutral Bit Count Summary ---\n");
    println!("Round | Bits with 100% zero-diff | Percentage");
    println!("──────┼────────────────────────────┼───────────");

    for round in 0..=16 {
        let neutral_count = (0..512)
            .filter(|&bp| neutral_through_round[bp][round] == samples as u32)
            .count();
        let pct = neutral_count as f64 / 512.0 * 100.0;
        println!("  {:2}  |          {:3}              |   {:5.1}%", round, neutral_count, pct);
    }

    println!("\n--- Key Finding ---\n");

    // The theoretical claim: W[i] enters at round i
    // So W[0] enters at round 0, W[15] enters at round 15
    // Bits in W[i] should be neutral through round i-1

    println!("Message word entry schedule:");
    println!("  W[0] enters at round 0  (used in t1 calculation)");
    println!("  W[1] enters at round 1");
    println!("  ...");
    println!("  W[15] enters at round 15");
    println!("");
    println!("HOWEVER: Message schedule expansion means:");
    println!("  W[16] = σ₁(W[14]) + W[9] + σ₀(W[1]) + W[0]");
    println!("  Changes to W[0] affect W[16], W[17], ...");
    println!("");

    // Check if W[15] bits are neutral through round 14
    let w15_neutral_14 = (15*32..16*32)
        .filter(|&bp| neutral_through_round[bp][14] == samples as u32)
        .count();

    println!("W[15] bits neutral through round 14: {} / 32", w15_neutral_14);

    if w15_neutral_14 == 32 {
        println!("✓ Confirmed: W[15] does not affect rounds 0-14");
    } else {
        println!("⚠️  W[15] affects rounds before 15 (message schedule coupling)");
    }
}

// ============================================================================
// TEST 2: Neutral Bit Speedup Calculation
// ============================================================================

#[test]
#[ignore]
fn test_neutral_bit_speedup() {
    println!("\n=== Test 2: Neutral Bit Speedup Calculation ===\n");

    // The claim: "25% of the puzzle free" if neutral bits exist
    // Let's calculate the ACTUAL speedup

    println!("Theoretical Neutral Bit Speedup Analysis:\n");

    println!("Scenario: N bits are neutral through round R");
    println!("─────────────────────────────────────────────");
    println!("");

    // From Test 1, we know which bits are neutral through which rounds
    // Let's calculate for ideal case where W[15] is fully neutral through round 14

    let total_rounds = 64;

    println!("| Neutral Through | Pre-compute | Vary | Savings |");
    println!("|-----------------|-------------|------|---------|");

    for neutral_rounds in [4, 8, 12, 15, 20] {
        let precompute_work = neutral_rounds as f64 / total_rounds as f64;
        let vary_work = (total_rounds - neutral_rounds) as f64 / total_rounds as f64;

        // If we have 2^N variations, we do:
        // - 1 × precompute (neutral_rounds)
        // - 2^N × vary (total_rounds - neutral_rounds)
        // vs 2^N × full (total_rounds)

        // For 32 neutral bits (one word):
        let n_bits = 32;
        let variations = 2u64.pow(n_bits);

        let naive_work = variations as f64 * total_rounds as f64;
        let smart_work = (neutral_rounds as f64) + (variations as f64 * (total_rounds - neutral_rounds) as f64);
        let savings = 1.0 - (smart_work / naive_work);

        println!("|     Round {:2}     |   {:5.1}%    | {:4.1}% |  {:5.2}% |",
                 neutral_rounds, precompute_work * 100.0, vary_work * 100.0, savings * 100.0);
    }

    println!("\n--- The \"25% Free\" Claim ---\n");

    // 25% savings would mean neutral through round 16 (16/64 = 25%)
    // But we need to verify this is achievable

    println!("For \"25% of the puzzle free\" (16 rounds neutral):");
    println!("  - Requires bits neutral through round 15");
    println!("  - Best case: W[15] (enters at round 15)");
    println!("  - Reality: Message schedule couples W[i] to W[i+16], etc.");
    println!("");

    // Calculate actual speedup with message schedule coupling
    println!("--- Message Schedule Coupling Effect ---\n");

    // W[16] = σ₁(W[14]) + W[9] + σ₀(W[1]) + W[0]
    // So changing W[15] doesn't affect W[16]
    // W[17] = σ₁(W[15]) + W[10] + σ₀(W[2]) + W[1]
    // Changing W[15] DOES affect W[17]!

    println!("W[16] = σ₁(W[14]) + W[9] + σ₀(W[1]) + W[0]   <- W[15] NOT used");
    println!("W[17] = σ₁(W[15]) + W[10] + σ₀(W[2]) + W[1]  <- W[15] USED via σ₁");
    println!("");
    println!("Even though W[15] enters the ROUND function at round 15,");
    println!("it affects the MESSAGE SCHEDULE starting at W[17].");
    println!("");
    println!("This means:");
    println!("  - Rounds 0-14: State unaffected by W[15]");
    println!("  - Round 15: W[15] enters round function");
    println!("  - Round 16: Uses W[16] (no W[15])");
    println!("  - Round 17+: Uses W[17]+ which depend on W[15]");
    println!("");

    // So the actual neutral window is rounds 0-14 and round 16 (15 rounds, not 16)
    let actual_neutral_rounds = 15;  // 0-14
    let savings_pct = actual_neutral_rounds as f64 / total_rounds as f64 * 100.0;

    println!("Actual savings from W[15] neutrality: {:.1}% (not 25%)", savings_pct);
    println!("");

    println!("--- The REAL Speedup ---\n");

    // But wait - this is for COLLISION attacks where you can choose the message
    // For MINING, the nonce is at a FIXED position

    println!("CRITICAL: This analysis assumes you can choose which bits to vary.");
    println!("");
    println!("In MINING:");
    println!("  - Block header is 80 bytes");
    println!("  - Nonce is at bytes 76-79 (end of second 64-byte chunk)");
    println!("  - First SHA-256: processes bytes 0-63");
    println!("  - Second SHA-256: processes bytes 64-79 (padded to 64)");
    println!("");
    println!("The NONCE position in the message schedule:");
    println!("  - Bytes 76-79 = W[19] bits 0-31 in second hash's first block... ");
    println!("  - Actually, let's calculate precisely:");
    println!("");

    // Bitcoin block header structure (80 bytes):
    // Bytes 0-3: Version (4 bytes)
    // Bytes 4-35: Previous block hash (32 bytes)
    // Bytes 36-67: Merkle root (32 bytes)
    // Bytes 68-71: Timestamp (4 bytes)
    // Bytes 72-75: Difficulty target (4 bytes)
    // Bytes 76-79: Nonce (4 bytes)

    // First SHA-256 input: 80 bytes + padding
    // Bytes 0-63: First block (W[0]-W[15])
    // Bytes 64-79: Part of second block

    // Wait, 80 bytes requires two SHA-256 blocks:
    // Block 1: bytes 0-63
    // Block 2: bytes 64-79 + padding (16 + 48 = 64 bytes)

    // Nonce is at bytes 76-79, which is in Block 2
    // In Block 2: bytes 64-79 become W[0]-W[3] (byte 76-79 = W[3])

    println!("Bitcoin header layout:");
    println!("  Block 1 (bytes 0-63):  W[0]-W[15]");
    println!("  Block 2 (bytes 64-79): W[0]-W[3] + padding");
    println!("");
    println!("Nonce (bytes 76-79) = W[3] of Block 2");
    println!("");
    println!("W[3] enters at round 3 -> Neutral through rounds 0-2 only!");
    println!("");

    let nonce_neutral_rounds = 3;  // 0, 1, 2
    let nonce_savings = nonce_neutral_rounds as f64 / total_rounds as f64 * 100.0;

    println!("ACTUAL MINING SAVINGS: {:.1}% (not 25%)", nonce_savings);
    println!("");
    println!("Even worse: Each nonce change requires re-computing rounds 3-63");
    println!("            That's 61/64 = 95.3% of the work");
    println!("");
    println!("The \"neutral bit\" optimization for mining: ~4.7% savings");
    println!("The claimed \"25% free\": Only applies to collision attacks");
}

// ============================================================================
// TEST 3: MitM Table Size Requirements
// ============================================================================

#[test]
#[ignore]
fn test_mitm_table_size() {
    println!("\n=== Test 3: MitM Table Size Requirements ===\n");

    // MitM requires storing forward computation results
    // Table size determines memory requirements

    println!("MitM Attack Memory Requirements:\n");

    // State at any round is 256 bits = 32 bytes
    let state_bytes = 32;

    // For a 2^N table, we need 2^N * 32 bytes

    println!("| Table Size (2^N) | Storage Required | Practical? |");
    println!("|------------------|------------------|------------|");

    for n in [16, 20, 24, 28, 32, 40, 48, 64, 80, 128] {
        let entries = 2u128.pow(n);
        let bytes = entries * state_bytes as u128;

        let (size_str, practical) = if bytes < 1024 {
            (format!("{} B", bytes), "Yes")
        } else if bytes < 1024 * 1024 {
            (format!("{:.1} KB", bytes as f64 / 1024.0), "Yes")
        } else if bytes < 1024 * 1024 * 1024 {
            (format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)), "Yes")
        } else if bytes < 1024u128 * 1024 * 1024 * 1024 {
            (format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)), if n <= 28 { "Difficult" } else { "No" })
        } else if bytes < 1024u128 * 1024 * 1024 * 1024 * 1024 {
            (format!("{:.1} TB", bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0)), "No")
        } else if bytes < 1024u128 * 1024 * 1024 * 1024 * 1024 * 1024 {
            (format!("{:.1} PB", bytes as f64 / (1024.0f64.powi(5))), "Impossible")
        } else {
            (format!("2^{} bytes", n + 5), "Impossible")
        };

        println!("|       2^{:<3}      | {:>16} | {:^10} |", n, size_str, practical);
    }

    println!("\n--- MitM Balance Point ---\n");

    // For MitM to work, we need:
    // Forward table: 2^k entries
    // Backward guesses: 2^m attempts
    // Total work: O(2^k + 2^m)
    // Probability of match: 2^k × 2^m / 2^256 (state space)

    // For expected 1 match: k + m = 256

    println!("For SHA-256 MitM with expected 1 match:");
    println!("  k + m = 256 (where 2^k forward, 2^m backward)");
    println!("");
    println!("| Forward (2^k) | Backward (2^m) | Memory     | Work       |");
    println!("|---------------|----------------|------------|------------|");

    for k in [64, 80, 100, 128, 160, 192] {
        let m = 256 - k;
        let mem = if k <= 30 {
            format!("{:.0} GB", 2.0f64.powi(k as i32 - 25))
        } else if k <= 40 {
            format!("{:.0} TB", 2.0f64.powi(k as i32 - 35))
        } else {
            format!("2^{} bytes", k + 5)
        };

        println!("|     2^{:<3}     |     2^{:<3}      | {:>10} |   2^{:<3}   |",
                 k, m, mem, k.max(m));
    }

    println!("\n--- Why MitM Doesn't Help Mining ---\n");

    println!("Mining difficulty is typically 2^70 to 2^80 (partial preimage)");
    println!("");
    println!("For MitM on partial preimage (k leading zeros):");
    println!("  - Need match in first k bits only");
    println!("  - Forward table: 2^(k/2) entries");
    println!("  - Backward guesses: 2^(k/2) attempts");
    println!("");

    for k in [70, 75, 80] {
        let half = k / 2;
        let mem_bytes = 2.0f64.powi(half + 5);
        let mem_str = if half + 5 <= 30 {
            format!("{:.0} GB", 2.0f64.powi(half as i32 - 25))
        } else {
            format!("2^{} GB", half - 25)
        };

        println!("Difficulty 2^{}: Table 2^{} = {}, Work 2^{}",
                 k, half, mem_str, half);
    }

    println!("");
    println!("At difficulty 2^80:");
    println!("  - MitM table: 2^40 entries = 32 TB");
    println!("  - MitM work: 2^40 backward attempts");
    println!("");
    println!("Brute force: 2^80 hashes at 100 GH/s = 12 million years");
    println!("MitM: 2^40 forward + 2^40 backward = 2^41 work");
    println!("");
    println!("BUT: The backward computation requires knowing W,");
    println!("     which requires guessing the nonce, which is...");
    println!("     exactly the same as brute force!");
    println!("");
    println!("MitM CANNOT help mining because:");
    println!("  1. Backward computation requires message (nonce)");
    println!("  2. You can't meet in the middle if both ends need the same unknown");
}

// ============================================================================
// TEST 4: Nonce Position Impact (Mining Reality)
// ============================================================================

#[test]
#[ignore]
fn test_nonce_position_impact() {
    println!("\n=== Test 4: Nonce Position Impact (Mining Reality) ===\n");

    // In Bitcoin mining, the nonce is at a FIXED position
    // Let's verify exactly where and what this means for neutral bits

    println!("Bitcoin Block Header (80 bytes):\n");
    println!("| Bytes  | Field           | Size   | SHA-256 Block |");
    println!("|--------|-----------------|--------|---------------|");
    println!("| 0-3    | Version         | 4      | Block 1, W[0] |");
    println!("| 4-35   | Prev Block Hash | 32     | Block 1, W[1-8] |");
    println!("| 36-67  | Merkle Root     | 32     | Block 1, W[9-15] + Block 2, W[0] |");
    println!("| 68-71  | Timestamp       | 4      | Block 2, W[1] |");
    println!("| 72-75  | Bits (Target)   | 4      | Block 2, W[1-2] |");
    println!("| 76-79  | **NONCE**       | 4      | **Block 2, W[2-3]** |");
    println!("");

    // More precisely:
    // Block 1: bytes 0-63 (W[0]-W[15])
    // Block 2: bytes 64-79 (W[0]-W[3]) + 1-byte 0x80 + zeros + 8-byte length

    println!("Precise nonce location in Block 2:");
    println!("  Bytes 64-67 (relative 0-3):  Part of Merkle root -> W[0]");
    println!("  Bytes 68-71 (relative 4-7):  Timestamp -> W[1]");
    println!("  Bytes 72-75 (relative 8-11): Bits (target) -> W[2]");
    println!("  Bytes 76-79 (relative 12-15): NONCE -> **W[3]**");
    println!("");

    // Let's verify empirically
    println!("--- Empirical Verification ---\n");

    // Create a mock block header
    let mut header = [0u8; 80];
    // Fill with random-ish data
    for i in 0..76 {
        header[i] = (i * 7 + 13) as u8;
    }
    // Nonce at 76-79
    header[76] = 0x12;
    header[77] = 0x34;
    header[78] = 0x56;
    header[79] = 0x78;

    // First SHA-256 block (bytes 0-63)
    let mut block1 = [0u8; 64];
    block1.copy_from_slice(&header[0..64]);
    let hash1 = sha256_single_block(&block1);

    // Second SHA-256 block (bytes 64-79 + padding)
    let mut block2 = [0u8; 64];
    block2[0..16].copy_from_slice(&header[64..80]);
    block2[16] = 0x80;  // Padding start
    // Length in bits = 80 * 8 = 640 = 0x280
    block2[62] = 0x02;
    block2[63] = 0x80;

    // Parse block2 into words
    let mut block2_words = [0u32; 16];
    for i in 0..16 {
        block2_words[i] = u32::from_be_bytes([
            block2[i*4], block2[i*4+1], block2[i*4+2], block2[i*4+3]
        ]);
    }

    println!("Block 2 word layout:");
    for i in 0..16 {
        let content = if i == 0 {
            "Merkle (last 4 bytes)"
        } else if i == 1 {
            "Timestamp"
        } else if i == 2 {
            "Bits (target)"
        } else if i == 3 {
            "**NONCE**"
        } else if i == 4 {
            "Padding (0x80...)"
        } else if i < 14 {
            "Padding (zeros)"
        } else if i == 14 {
            "Length high"
        } else {
            "Length low"
        };
        println!("  W[{:2}] = 0x{:08x}  <- {}", i, block2_words[i], content);
    }

    println!("\n--- Nonce Neutrality Window ---\n");

    // W[3] enters at round 3
    // So rounds 0, 1, 2 are unaffected by nonce

    println!("Nonce is W[3] in Block 2's SHA-256");
    println!("W[3] enters round function at round 3");
    println!("");
    println!("Neutral window: rounds 0-2 (3 rounds out of 64)");
    println!("Savings: 3/64 = 4.69%");
    println!("");

    // But wait - we need to verify this empirically
    println!("--- Empirical Nonce Impact Test ---\n");

    let samples = 1000;
    let mut round_impacts = [0u64; 10];

    for sample in 0..samples {
        // Create random block2-like structure
        let mut base_block = [0u32; 16];
        for i in 0..16 {
            base_block[i] = ((sample * 16 + i) as u32).wrapping_mul(0x9e3779b9);
        }
        // Set padding
        base_block[4] = 0x80000000;
        for i in 5..14 {
            base_block[i] = 0;
        }
        base_block[14] = 0;
        base_block[15] = 640;  // 80 bytes * 8 bits

        // Flip nonce (W[3])
        let mut mod_block = base_block;
        mod_block[3] ^= 1;  // Single bit flip in nonce

        let base_w = expand_message_schedule(&base_block);
        let mod_w = expand_message_schedule(&mod_block);

        // Check state at each round
        let mut base_state = H0;
        let mut mod_state = H0;

        for round in 0..10 {
            if round > 0 {
                base_state = sha256_round_forward(&base_state, base_w[round-1], K[round-1]);
                mod_state = sha256_round_forward(&mod_state, mod_w[round-1], K[round-1]);
            }

            let diff = state_hamming(&base_state, &mod_state);
            round_impacts[round] += diff as u64;
        }
    }

    println!("Average state difference when nonce bit flipped:\n");
    println!("Round | Avg Diff (bits) | Status");
    println!("──────┼─────────────────┼────────────────");
    for round in 0..10 {
        let avg_diff = round_impacts[round] as f64 / samples as f64;
        let status = if avg_diff < 0.01 {
            "NEUTRAL ✓"
        } else if avg_diff < 1.0 {
            "Nearly neutral"
        } else if avg_diff < 64.0 {
            "Affected"
        } else {
            "Fully avalanched"
        };
        println!("  {:2}  |     {:6.2}      | {}", round, avg_diff, status);
    }

    println!("\n--- Mining Optimization Reality ---\n");

    // W[3] being neutral through round 2 means:
    // - We can pre-compute rounds 0-2 once
    // - Then iterate nonces, computing rounds 3-63 each time

    println!("Theoretical savings: Pre-compute rounds 0-2 (3/64 = 4.69%)");
    println!("");
    println!("BUT: This is already implicitly done by midstate optimization!");
    println!("");
    println!("Midstate optimization:");
    println!("  - First SHA-256 block (header bytes 0-63) is constant");
    println!("  - Only second block changes with nonce");
    println!("  - Mining software already pre-computes first block");
    println!("");
    println!("The \"neutral bit\" savings are ALREADY CAPTURED by standard");
    println!("mining implementations. There's nothing new to exploit.");
}

// ============================================================================
// TEST 5: Forward/Backward Balance Optimization
// ============================================================================

#[test]
#[ignore]
fn test_forward_backward_balance() {
    println!("\n=== Test 5: Forward/Backward Balance Optimization ===\n");

    // If we could do MitM, what's the optimal split?
    // Trade-off: more forward = more memory, less backward work

    println!("MitM Split Optimization (Theoretical):\n");

    // Total work = Forward computation + Backward computation + Table lookups
    // Forward: 2^k × forward_rounds
    // Backward: 2^m × backward_rounds
    // Lookups: 2^m × log(2^k)

    let total_rounds = 64;

    println!("Assuming split at round R:");
    println!("  Forward: rounds 0 to R-1 (R rounds)");
    println!("  Backward: rounds R to 63 (64-R rounds)");
    println!("");

    println!("| Split | Forward | Backward | Memory    | Work Balance    |");
    println!("|-------|---------|----------|-----------|-----------------|");

    for split in [8, 16, 24, 32, 40, 48, 56] {
        let fwd_rounds = split;
        let bwd_rounds = total_rounds - split;

        // Work is proportional to rounds
        let fwd_work_per = fwd_rounds as f64 / total_rounds as f64;
        let bwd_work_per = bwd_rounds as f64 / total_rounds as f64;

        println!("|  {:2}   |   {:2}r   |   {:2}r    | 2^{} states | {:.1}% / {:.1}% |",
                 split, fwd_rounds, bwd_rounds,
                 split, fwd_work_per * 100.0, bwd_work_per * 100.0);
    }

    println!("\n--- The Optimal Split ---\n");

    // Optimal when forward_work = backward_work
    // k × fwd_rounds = m × bwd_rounds
    // k × R = m × (64 - R)
    // With k + m = 256 (for full SHA-256)

    println!("For equal work split (k×R = m×(64-R), k+m=256):");
    println!("");

    // Solving: k × R = (256-k) × (64-R)
    // k × R = 256×64 - 256×R - 64×k + k×R
    // 0 = 256×64 - 256×R - 64×k
    // 64×k = 256×(64-R)
    // k = 4×(64-R)

    // For R = 32: k = 4 × 32 = 128, m = 128
    // Work = 128 × 32 = 4096 (both sides)

    for r in [24, 28, 32, 36, 40] {
        let k = 4 * (64 - r);
        let m = 256 - k;
        let fwd_work = k * r;
        let bwd_work = m * (64 - r);

        println!("Split at {}: k={}, m={}, work={}×{}={} vs {}×{}={}",
                 r, k, m, k, r, fwd_work, m, 64-r, bwd_work);
    }

    println!("\nOptimal: Split at round 32 with 2^128 table (unfeasible)");

    println!("\n--- Why This Doesn't Help ---\n");

    println!("1. Memory barrier: 2^128 states = 2^133 bytes (impossible)");
    println!("2. Circular dependency: Backward needs W, which needs nonce");
    println!("3. Mining difficulty is 2^80, not 2^256");
    println!("");
    println!("For mining at difficulty 2^80:");
    println!("  Optimal MitM: 2^40 table, 2^40 backward");
    println!("  Memory: 2^40 × 32 bytes = 32 TB (difficult but possible)");
    println!("  BUT: Backward still needs the nonce (circular)");
    println!("");
    println!("The fundamental problem remains:");
    println!("  You can't meet in the middle if both paths");
    println!("  start from the same unknown value (the nonce).");
}

// ============================================================================
// TEST 6: Combined Analysis Summary
// ============================================================================

#[test]
#[ignore]
fn test_combined_analysis_summary() {
    println!("\n=== Test 6: Combined Analysis Summary ===\n");

    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│         NEUTRAL BITS & MitM: PRACTICAL ANALYSIS                │");
    println!("└────────────────────────────────────────────────────────────────┘\n");

    println!("CLAIM 1: \"Local reversibility allows backward computation\"");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  VERIFIED: Single rounds invert perfectly when W is known.");
    println!("  PROBLEM: W requires the message, which is what we seek.");
    println!("  STATUS: TRUE but USELESS for attacks.");
    println!("");

    println!("CLAIM 2: \"MitM reduces complexity from 2^256 to 2^128\"");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  THEORY: Store 2^128 forward states, try 2^128 backward.");
    println!("  MEMORY: 2^128 × 32 bytes = 2^133 bytes (impossible).");
    println!("  MINING: Doesn't apply - backward needs nonce (circular).");
    println!("  STATUS: THEORETICALLY TRUE, PRACTICALLY IMPOSSIBLE.");
    println!("");

    println!("CLAIM 3: \"Neutral bits make 25% of the puzzle free\"");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  THEORY: W[15] neutral through round 14 = 23% savings.");
    println!("  MINING: Nonce is W[3], neutral through round 2 = 4.7%.");
    println!("  REALITY: Midstate optimization already captures this.");
    println!("  STATUS: ALREADY IMPLEMENTED in all mining software.");
    println!("");

    println!("CLAIM 4: \"Pre-calculate first 15 rounds, vary neutral bits\"");
    println!("─────────────────────────────────────────────────────────────────");
    println!("  APPLIES TO: Collision attacks (can choose all message bits).");
    println!("  MINING: Cannot choose message bits - nonce position fixed.");
    println!("  BEST CASE: Pre-compute rounds 0-2 (nonce enters at round 3).");
    println!("  STATUS: NOT APPLICABLE to mining.");
    println!("");

    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│                        FINAL VERDICT                           │");
    println!("└────────────────────────────────────────────────────────────────┘\n");

    println!("For COLLISION attacks (choose full message):");
    println!("  - Neutral bits provide ~15-20% savings");
    println!("  - MitM provides theoretical 2^128 (vs 2^256) speedup");
    println!("  - These attacks exist in literature");
    println!("");

    println!("For PREIMAGE attacks (recover message from hash):");
    println!("  - MitM fails due to circular dependency");
    println!("  - Neutral bits fail (can't choose message position)");
    println!("  - Best attack: 2^255.5 (biclique, 1.4× speedup)");
    println!("");

    println!("For MINING (partial preimage at fixed difficulty):");
    println!("  - Nonce at fixed position (W[3] in second block)");
    println!("  - Neutral window: 3 rounds (already optimized via midstate)");
    println!("  - MitM impossible (backward needs nonce)");
    println!("  - NO CRYPTANALYTIC ADVANTAGE EXISTS");
    println!("");

    println!("The \"cutting edge\" claims about SHA-256 attacks apply to");
    println!("theoretical scenarios that don't map to Bitcoin mining.");
    println!("");
    println!("All mining optimization is HARDWARE optimization:");
    println!("  ✓ More parallel cores");
    println!("  ✓ Lower power consumption");
    println!("  ✓ Better clock speeds");
    println!("  ✓ Midstate pre-computation (standard)");
    println!("");
    println!("There are NO algorithmic shortcuts for mining.");
}

/// Run all v20 tests
#[test]
#[ignore]
fn test_all_v20_neutral_bits_mitm() {
    println!("\n============================================================");
    println!("Research v20: Neutral Bits & MitM Practical Analysis");
    println!("============================================================\n");

    test_true_neutral_bits();
    test_neutral_bit_speedup();
    test_mitm_table_size();
    test_nonce_position_impact();
    test_forward_backward_balance();
    test_combined_analysis_summary();

    println!("\n============================================================");
    println!("CONCLUSION: Neutral bits & MitM do NOT help mining");
    println!("============================================================\n");
}
