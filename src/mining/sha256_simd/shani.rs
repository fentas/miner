//! SHA-NI hardware accelerated SHA-256 implementation
//!
//! Uses Intel SHA Extensions for ~100-150 MH/s single thread performance.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::{H_INIT, K, Midstate, state_to_bytes, hash_meets_target};

/// SHA-NI accelerated single SHA-256 transform
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI and SSE4.1 instructions.
/// Reference: https://github.com/noloader/SHA-Intrinsics/blob/master/sha256-x86.c
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn transform(state: &mut [u32; 8], block: &[u8; 64]) {
    // Load initial state using Intel's reference pattern
    let tmp = _mm_loadu_si128(state.as_ptr() as *const __m128i);
    let mut state1 = _mm_loadu_si128(state.as_ptr().add(4) as *const __m128i);

    let tmp = _mm_shuffle_epi32(tmp, 0xB1);       // CDAB
    state1 = _mm_shuffle_epi32(state1, 0x1B);     // EFGH
    let mut state0 = _mm_alignr_epi8(tmp, state1, 8);   // ABEF
    state1 = _mm_blend_epi16(state1, tmp, 0xF0);  // CDGH

    let abef_save = state0;
    let cdgh_save = state1;

    // Byte swap mask for converting little-endian to big-endian
    let shuf_mask = _mm_set_epi64x(0x0c0d0e0f08090a0b, 0x0405060700010203);

    // Load and byte-swap message blocks
    let mut msg0 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr() as *const __m128i), shuf_mask);
    let mut msg1 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(16) as *const __m128i), shuf_mask);
    let mut msg2 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(32) as *const __m128i), shuf_mask);
    let mut msg3 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(48) as *const __m128i), shuf_mask);

    // Rounds 0-3
    let mut msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[3] as i32, K[2] as i32, K[1] as i32, K[0] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);

    // Rounds 4-7
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[7] as i32, K[6] as i32, K[5] as i32, K[4] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);

    // Rounds 8-11
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[11] as i32, K[10] as i32, K[9] as i32, K[8] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);

    // Rounds 12-15
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[15] as i32, K[14] as i32, K[13] as i32, K[12] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    let mut tmp = _mm_alignr_epi8(msg3, msg2, 4);
    msg0 = _mm_add_epi32(msg0, tmp);
    msg0 = _mm_sha256msg2_epu32(msg0, msg3);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);

    // Rounds 16-19
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[19] as i32, K[18] as i32, K[17] as i32, K[16] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg0, msg3, 4);
    msg1 = _mm_add_epi32(msg1, tmp);
    msg1 = _mm_sha256msg2_epu32(msg1, msg0);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);

    // Rounds 20-23
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[23] as i32, K[22] as i32, K[21] as i32, K[20] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg1, msg0, 4);
    msg2 = _mm_add_epi32(msg2, tmp);
    msg2 = _mm_sha256msg2_epu32(msg2, msg1);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);

    // Rounds 24-27
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[27] as i32, K[26] as i32, K[25] as i32, K[24] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg2, msg1, 4);
    msg3 = _mm_add_epi32(msg3, tmp);
    msg3 = _mm_sha256msg2_epu32(msg3, msg2);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);

    // Rounds 28-31
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[31] as i32, K[30] as i32, K[29] as i32, K[28] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg3, msg2, 4);
    msg0 = _mm_add_epi32(msg0, tmp);
    msg0 = _mm_sha256msg2_epu32(msg0, msg3);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);

    // Rounds 32-35
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[35] as i32, K[34] as i32, K[33] as i32, K[32] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg0, msg3, 4);
    msg1 = _mm_add_epi32(msg1, tmp);
    msg1 = _mm_sha256msg2_epu32(msg1, msg0);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);

    // Rounds 36-39
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[39] as i32, K[38] as i32, K[37] as i32, K[36] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg1, msg0, 4);
    msg2 = _mm_add_epi32(msg2, tmp);
    msg2 = _mm_sha256msg2_epu32(msg2, msg1);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);

    // Rounds 40-43
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[43] as i32, K[42] as i32, K[41] as i32, K[40] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg2, msg1, 4);
    msg3 = _mm_add_epi32(msg3, tmp);
    msg3 = _mm_sha256msg2_epu32(msg3, msg2);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);

    // Rounds 44-47
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[47] as i32, K[46] as i32, K[45] as i32, K[44] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg3, msg2, 4);
    msg0 = _mm_add_epi32(msg0, tmp);
    msg0 = _mm_sha256msg2_epu32(msg0, msg3);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);

    // Rounds 48-51
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[51] as i32, K[50] as i32, K[49] as i32, K[48] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg0, msg3, 4);
    msg1 = _mm_add_epi32(msg1, tmp);
    msg1 = _mm_sha256msg2_epu32(msg1, msg0);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);

    // Rounds 52-55
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[55] as i32, K[54] as i32, K[53] as i32, K[52] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg1, msg0, 4);
    msg2 = _mm_add_epi32(msg2, tmp);
    msg2 = _mm_sha256msg2_epu32(msg2, msg1);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);

    // Rounds 56-59
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[59] as i32, K[58] as i32, K[57] as i32, K[56] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    tmp = _mm_alignr_epi8(msg2, msg1, 4);
    msg3 = _mm_add_epi32(msg3, tmp);
    msg3 = _mm_sha256msg2_epu32(msg3, msg2);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);

    // Rounds 60-63 (final - no message schedule expansion needed)
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[63] as i32, K[62] as i32, K[61] as i32, K[60] as i32
    ));
    state1 = _mm_sha256rnds2_epu32(state1, state0, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    state0 = _mm_sha256rnds2_epu32(state0, state1, msg);

    // Add to original state
    state0 = _mm_add_epi32(state0, abef_save);
    state1 = _mm_add_epi32(state1, cdgh_save);

    // Extract state back to memory using Intel's reference pattern
    // Reference: https://github.com/noloader/SHA-Intrinsics/blob/master/sha256-x86.c
    let tmp = _mm_shuffle_epi32(state0, 0x1B);     // FEBA
    state1 = _mm_shuffle_epi32(state1, 0xB1);      // DCHG
    let out0 = _mm_blend_epi16(tmp, state1, 0xF0); // DCBA
    let out1 = _mm_alignr_epi8(state1, tmp, 8);    // HGFE

    _mm_storeu_si128(state.as_mut_ptr() as *mut __m128i, out0);
    _mm_storeu_si128(state.as_mut_ptr().add(4) as *mut __m128i, out1);
}

/// SHA-NI accelerated double SHA-256 with midstate for mining
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn sha256d_midstate(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    // Build the last block (16 bytes of data + nonce + padding)
    let mut block = [0u8; 64];
    block[..16].copy_from_slice(tail);
    block[16..20].copy_from_slice(&nonce.to_le_bytes());
    // Padding for 80-byte message
    block[20] = 0x80;
    // Length in bits (80 * 8 = 640 = 0x280) at end
    block[62] = 0x02;
    block[63] = 0x80;

    // First hash (continuing from midstate)
    let mut state = midstate.state;
    transform(&mut state, &block);
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01; // 256 bits = 0x100
    block2[63] = 0x00;
    transform(&mut state, &block2);
    state_to_bytes(&state)
}

/// SHA-NI accelerated mining loop
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn mine(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    for i in 0..nonce_count {
        let nonce = nonce_start.wrapping_add(i);
        let hash = sha256d_midstate(midstate, tail, nonce);
        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }
    None
}
