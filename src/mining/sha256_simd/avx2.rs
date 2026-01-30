//! AVX2 8-way parallel SHA-256 implementation
//!
//! Fallback for CPUs without SHA-NI but with AVX2 (Haswell/Broadwell era).
//! Processes 8 nonces in parallel using 256-bit registers.

// Allow indexed loops in SIMD code - clearer than iterator patterns for multi-array access
#![allow(clippy::needless_range_loop)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::{H_INIT, K, Midstate, state_to_bytes, hash_meets_target};

/// AVX2 8-way parallel SHA-256 transform
///
/// Processes 8 independent SHA-256 blocks simultaneously using 256-bit registers.
/// Each lane in the __m256i registers corresponds to one hash computation.
///
/// # Safety
/// Caller must ensure CPU supports AVX2 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn transform_8way(
    states: &mut [[u32; 8]; 8],
    blocks: &[[u8; 64]; 8],
) {
    // Load states into AVX2 registers (transposed: each register holds one word from all 8 hashes)
    let mut state = [
        _mm256_set_epi32(states[7][0] as i32, states[6][0] as i32, states[5][0] as i32, states[4][0] as i32,
                         states[3][0] as i32, states[2][0] as i32, states[1][0] as i32, states[0][0] as i32),
        _mm256_set_epi32(states[7][1] as i32, states[6][1] as i32, states[5][1] as i32, states[4][1] as i32,
                         states[3][1] as i32, states[2][1] as i32, states[1][1] as i32, states[0][1] as i32),
        _mm256_set_epi32(states[7][2] as i32, states[6][2] as i32, states[5][2] as i32, states[4][2] as i32,
                         states[3][2] as i32, states[2][2] as i32, states[1][2] as i32, states[0][2] as i32),
        _mm256_set_epi32(states[7][3] as i32, states[6][3] as i32, states[5][3] as i32, states[4][3] as i32,
                         states[3][3] as i32, states[2][3] as i32, states[1][3] as i32, states[0][3] as i32),
        _mm256_set_epi32(states[7][4] as i32, states[6][4] as i32, states[5][4] as i32, states[4][4] as i32,
                         states[3][4] as i32, states[2][4] as i32, states[1][4] as i32, states[0][4] as i32),
        _mm256_set_epi32(states[7][5] as i32, states[6][5] as i32, states[5][5] as i32, states[4][5] as i32,
                         states[3][5] as i32, states[2][5] as i32, states[1][5] as i32, states[0][5] as i32),
        _mm256_set_epi32(states[7][6] as i32, states[6][6] as i32, states[5][6] as i32, states[4][6] as i32,
                         states[3][6] as i32, states[2][6] as i32, states[1][6] as i32, states[0][6] as i32),
        _mm256_set_epi32(states[7][7] as i32, states[6][7] as i32, states[5][7] as i32, states[4][7] as i32,
                         states[3][7] as i32, states[2][7] as i32, states[1][7] as i32, states[0][7] as i32),
    ];

    let state_save = state;

    // Load and prepare message schedule (W[0..15] from blocks, big-endian)
    let mut w: [__m256i; 64] = [_mm256_setzero_si256(); 64];
    for i in 0..16 {
        let offset = i * 4;
        w[i] = _mm256_set_epi32(
            i32::from_be_bytes([blocks[7][offset], blocks[7][offset+1], blocks[7][offset+2], blocks[7][offset+3]]),
            i32::from_be_bytes([blocks[6][offset], blocks[6][offset+1], blocks[6][offset+2], blocks[6][offset+3]]),
            i32::from_be_bytes([blocks[5][offset], blocks[5][offset+1], blocks[5][offset+2], blocks[5][offset+3]]),
            i32::from_be_bytes([blocks[4][offset], blocks[4][offset+1], blocks[4][offset+2], blocks[4][offset+3]]),
            i32::from_be_bytes([blocks[3][offset], blocks[3][offset+1], blocks[3][offset+2], blocks[3][offset+3]]),
            i32::from_be_bytes([blocks[2][offset], blocks[2][offset+1], blocks[2][offset+2], blocks[2][offset+3]]),
            i32::from_be_bytes([blocks[1][offset], blocks[1][offset+1], blocks[1][offset+2], blocks[1][offset+3]]),
            i32::from_be_bytes([blocks[0][offset], blocks[0][offset+1], blocks[0][offset+2], blocks[0][offset+3]]),
        );
    }

    // Expand message schedule W[16..63]
    for i in 16..64 {
        // s0 = rotr(w[i-15], 7) ^ rotr(w[i-15], 18) ^ (w[i-15] >> 3)
        let w15 = w[i - 15];
        let s0 = _mm256_xor_si256(
            _mm256_xor_si256(
                _mm256_or_si256(_mm256_srli_epi32(w15, 7), _mm256_slli_epi32(w15, 25)),
                _mm256_or_si256(_mm256_srli_epi32(w15, 18), _mm256_slli_epi32(w15, 14)),
            ),
            _mm256_srli_epi32(w15, 3),
        );

        // s1 = rotr(w[i-2], 17) ^ rotr(w[i-2], 19) ^ (w[i-2] >> 10)
        let w2 = w[i - 2];
        let s1 = _mm256_xor_si256(
            _mm256_xor_si256(
                _mm256_or_si256(_mm256_srli_epi32(w2, 17), _mm256_slli_epi32(w2, 15)),
                _mm256_or_si256(_mm256_srli_epi32(w2, 19), _mm256_slli_epi32(w2, 13)),
            ),
            _mm256_srli_epi32(w2, 10),
        );

        // w[i] = w[i-16] + s0 + w[i-7] + s1
        w[i] = _mm256_add_epi32(
            _mm256_add_epi32(w[i - 16], s0),
            _mm256_add_epi32(w[i - 7], s1),
        );
    }

    // Main compression loop
    for i in 0..64 {
        // S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
        let e = state[4];
        let s1 = _mm256_xor_si256(
            _mm256_xor_si256(
                _mm256_or_si256(_mm256_srli_epi32(e, 6), _mm256_slli_epi32(e, 26)),
                _mm256_or_si256(_mm256_srli_epi32(e, 11), _mm256_slli_epi32(e, 21)),
            ),
            _mm256_or_si256(_mm256_srli_epi32(e, 25), _mm256_slli_epi32(e, 7)),
        );

        // ch = (e & f) ^ (~e & g)
        let ch = _mm256_xor_si256(
            _mm256_and_si256(state[4], state[5]),
            _mm256_andnot_si256(state[4], state[6]),
        );

        // temp1 = h + S1 + ch + K[i] + w[i]
        let k_i = _mm256_set1_epi32(K[i] as i32);
        let temp1 = _mm256_add_epi32(
            _mm256_add_epi32(
                _mm256_add_epi32(state[7], s1),
                _mm256_add_epi32(ch, k_i),
            ),
            w[i],
        );

        // S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
        let a = state[0];
        let s0 = _mm256_xor_si256(
            _mm256_xor_si256(
                _mm256_or_si256(_mm256_srli_epi32(a, 2), _mm256_slli_epi32(a, 30)),
                _mm256_or_si256(_mm256_srli_epi32(a, 13), _mm256_slli_epi32(a, 19)),
            ),
            _mm256_or_si256(_mm256_srli_epi32(a, 22), _mm256_slli_epi32(a, 10)),
        );

        // maj = (a & b) ^ (a & c) ^ (b & c)
        let maj = _mm256_xor_si256(
            _mm256_xor_si256(
                _mm256_and_si256(state[0], state[1]),
                _mm256_and_si256(state[0], state[2]),
            ),
            _mm256_and_si256(state[1], state[2]),
        );

        // temp2 = S0 + maj
        let temp2 = _mm256_add_epi32(s0, maj);

        // Update state
        state[7] = state[6];
        state[6] = state[5];
        state[5] = state[4];
        state[4] = _mm256_add_epi32(state[3], temp1);
        state[3] = state[2];
        state[2] = state[1];
        state[1] = state[0];
        state[0] = _mm256_add_epi32(temp1, temp2);
    }

    // Add to original state
    for i in 0..8 {
        state[i] = _mm256_add_epi32(state[i], state_save[i]);
    }

    // Extract states back to arrays
    let mut out = [[0i32; 8]; 8];
    for i in 0..8 {
        _mm256_storeu_si256(out[i].as_mut_ptr() as *mut __m256i, state[i]);
    }

    // Transpose back: out[word][lane] -> states[lane][word]
    for lane in 0..8 {
        for word in 0..8 {
            states[lane][word] = out[word][lane] as u32;
        }
    }
}

/// AVX2 8-way parallel mining
///
/// Processes 8 nonces at a time for CPUs without SHA-NI but with AVX2.
///
/// # Safety
/// Caller must ensure CPU supports AVX2 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn mine(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    let mut i = 0u32;
    while i < nonce_count {
        // Process up to 8 nonces at a time
        let batch_size = std::cmp::min(8, nonce_count - i) as usize;

        // Prepare 8 blocks with different nonces
        let mut blocks: [[u8; 64]; 8] = [[0u8; 64]; 8];
        let mut nonces = [0u32; 8];

        for j in 0..batch_size {
            let nonce = nonce_start.wrapping_add(i).wrapping_add(j as u32);
            nonces[j] = nonce;

            // Build the last block (16 bytes of data + nonce + padding)
            blocks[j][..16].copy_from_slice(tail);
            blocks[j][16..20].copy_from_slice(&nonce.to_le_bytes());
            blocks[j][20] = 0x80;
            blocks[j][62] = 0x02;
            blocks[j][63] = 0x80;
        }

        // Fill remaining slots with duplicates (will be ignored)
        for j in batch_size..8 {
            blocks[j] = blocks[0];
            nonces[j] = nonces[0];
        }

        // First hash (continuing from midstate)
        let mut states1: [[u32; 8]; 8] = [midstate.state; 8];
        transform_8way(&mut states1, &blocks);

        // Prepare second hash blocks (32 bytes of first hash + padding)
        let mut blocks2: [[u8; 64]; 8] = [[0u8; 64]; 8];
        for j in 0..8 {
            for w in 0..8 {
                blocks2[j][w * 4..w * 4 + 4].copy_from_slice(&states1[j][w].to_be_bytes());
            }
            blocks2[j][32] = 0x80;
            blocks2[j][62] = 0x01;
            blocks2[j][63] = 0x00;
        }

        // Second hash
        let mut states2: [[u32; 8]; 8] = [H_INIT; 8];
        transform_8way(&mut states2, &blocks2);

        // Check each hash against target
        for j in 0..batch_size {
            let hash = state_to_bytes(&states2[j]);
            if hash_meets_target(&hash, target) {
                return Some((nonces[j], hash));
            }
        }

        i += 8;
    }
    None
}
