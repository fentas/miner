//! SHA-NI hardware accelerated SHA-256 implementation
//!
//! Uses Intel SHA Extensions with 2-way interleaving to hide instruction latency.
//! Expected performance: ~20-40 MH/s single thread (depending on CPU).

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use super::{H_INIT, K, Midstate, state_to_bytes, hash_meets_target};

/// Pre-computed K constants as __m128i for faster loading
#[cfg(target_arch = "x86_64")]
static K_SIMD: [[i32; 4]; 16] = [
    [K[0] as i32, K[1] as i32, K[2] as i32, K[3] as i32],
    [K[4] as i32, K[5] as i32, K[6] as i32, K[7] as i32],
    [K[8] as i32, K[9] as i32, K[10] as i32, K[11] as i32],
    [K[12] as i32, K[13] as i32, K[14] as i32, K[15] as i32],
    [K[16] as i32, K[17] as i32, K[18] as i32, K[19] as i32],
    [K[20] as i32, K[21] as i32, K[22] as i32, K[23] as i32],
    [K[24] as i32, K[25] as i32, K[26] as i32, K[27] as i32],
    [K[28] as i32, K[29] as i32, K[30] as i32, K[31] as i32],
    [K[32] as i32, K[33] as i32, K[34] as i32, K[35] as i32],
    [K[36] as i32, K[37] as i32, K[38] as i32, K[39] as i32],
    [K[40] as i32, K[41] as i32, K[42] as i32, K[43] as i32],
    [K[44] as i32, K[45] as i32, K[46] as i32, K[47] as i32],
    [K[48] as i32, K[49] as i32, K[50] as i32, K[51] as i32],
    [K[52] as i32, K[53] as i32, K[54] as i32, K[55] as i32],
    [K[56] as i32, K[57] as i32, K[58] as i32, K[59] as i32],
    [K[60] as i32, K[61] as i32, K[62] as i32, K[63] as i32],
];

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
    // Build the last block for an 80-byte message:
    // - bytes 64-75: tail[0..12] (12 bytes of header data)
    // - bytes 76-79: nonce (4 bytes)
    // - byte 80: 0x80 padding marker
    let mut block = [0u8; 64];
    block[..12].copy_from_slice(&tail[..12]);
    block[12..16].copy_from_slice(&nonce.to_le_bytes());
    // Padding for 80-byte message
    block[16] = 0x80;
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

/// SHA-NI accelerated mining loop (sequential - for comparison)
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
pub unsafe fn mine_sequential(
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

/// 2-way interleaved SHA-256 transform
///
/// Processes two independent blocks simultaneously to hide instruction latency.
/// SHA-NI instructions have ~4-6 cycle latency but can issue every 1-2 cycles.
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
unsafe fn transform_2way(
    state_a: &mut [u32; 8],
    state_b: &mut [u32; 8],
    block_a: &[u8; 64],
    block_b: &[u8; 64],
) {
    // Load initial states
    let tmp_a = _mm_loadu_si128(state_a.as_ptr() as *const __m128i);
    let tmp_b = _mm_loadu_si128(state_b.as_ptr() as *const __m128i);
    let mut state1_a = _mm_loadu_si128(state_a.as_ptr().add(4) as *const __m128i);
    let mut state1_b = _mm_loadu_si128(state_b.as_ptr().add(4) as *const __m128i);

    let tmp_a = _mm_shuffle_epi32(tmp_a, 0xB1);
    let tmp_b = _mm_shuffle_epi32(tmp_b, 0xB1);
    state1_a = _mm_shuffle_epi32(state1_a, 0x1B);
    state1_b = _mm_shuffle_epi32(state1_b, 0x1B);
    let mut state0_a = _mm_alignr_epi8(tmp_a, state1_a, 8);
    let mut state0_b = _mm_alignr_epi8(tmp_b, state1_b, 8);
    state1_a = _mm_blend_epi16(state1_a, tmp_a, 0xF0);
    state1_b = _mm_blend_epi16(state1_b, tmp_b, 0xF0);

    let abef_save_a = state0_a;
    let abef_save_b = state0_b;
    let cdgh_save_a = state1_a;
    let cdgh_save_b = state1_b;

    let shuf_mask = _mm_set_epi64x(0x0c0d0e0f08090a0b, 0x0405060700010203);

    // Load and byte-swap messages - interleaved
    let mut msg0_a = _mm_shuffle_epi8(_mm_loadu_si128(block_a.as_ptr() as *const __m128i), shuf_mask);
    let mut msg0_b = _mm_shuffle_epi8(_mm_loadu_si128(block_b.as_ptr() as *const __m128i), shuf_mask);
    let mut msg1_a = _mm_shuffle_epi8(_mm_loadu_si128(block_a.as_ptr().add(16) as *const __m128i), shuf_mask);
    let mut msg1_b = _mm_shuffle_epi8(_mm_loadu_si128(block_b.as_ptr().add(16) as *const __m128i), shuf_mask);
    let mut msg2_a = _mm_shuffle_epi8(_mm_loadu_si128(block_a.as_ptr().add(32) as *const __m128i), shuf_mask);
    let mut msg2_b = _mm_shuffle_epi8(_mm_loadu_si128(block_b.as_ptr().add(32) as *const __m128i), shuf_mask);
    let mut msg3_a = _mm_shuffle_epi8(_mm_loadu_si128(block_a.as_ptr().add(48) as *const __m128i), shuf_mask);
    let mut msg3_b = _mm_shuffle_epi8(_mm_loadu_si128(block_b.as_ptr().add(48) as *const __m128i), shuf_mask);

    // Macro for 2-way interleaved rounds
    macro_rules! rounds_2way {
        ($round:expr, $msg:ident, $msg_a:ident, $msg_b:ident, $state0_a:ident, $state0_b:ident, $state1_a:ident, $state1_b:ident) => {
            let k = _mm_loadu_si128(K_SIMD[$round / 4].as_ptr() as *const __m128i);
            let mut $msg = _mm_add_epi32($msg_a, k);
            $state1_a = _mm_sha256rnds2_epu32($state1_a, $state0_a, $msg);
            $msg = _mm_add_epi32($msg_b, k);
            $state1_b = _mm_sha256rnds2_epu32($state1_b, $state0_b, $msg);
            $msg = _mm_shuffle_epi32(_mm_add_epi32($msg_a, k), 0x0E);
            $state0_a = _mm_sha256rnds2_epu32($state0_a, $state1_a, $msg);
            $msg = _mm_shuffle_epi32(_mm_add_epi32($msg_b, k), 0x0E);
            $state0_b = _mm_sha256rnds2_epu32($state0_b, $state1_b, $msg);
        };
    }

    // Rounds 0-3
    rounds_2way!(0, msg, msg0_a, msg0_b, state0_a, state0_b, state1_a, state1_b);

    // Rounds 4-7
    rounds_2way!(4, msg, msg1_a, msg1_b, state0_a, state0_b, state1_a, state1_b);
    msg0_a = _mm_sha256msg1_epu32(msg0_a, msg1_a);
    msg0_b = _mm_sha256msg1_epu32(msg0_b, msg1_b);

    // Rounds 8-11
    rounds_2way!(8, msg, msg2_a, msg2_b, state0_a, state0_b, state1_a, state1_b);
    msg1_a = _mm_sha256msg1_epu32(msg1_a, msg2_a);
    msg1_b = _mm_sha256msg1_epu32(msg1_b, msg2_b);

    // Rounds 12-15
    {
        let k = _mm_loadu_si128(K_SIMD[3].as_ptr() as *const __m128i);
        let mut msg = _mm_add_epi32(msg3_a, k);
        state1_a = _mm_sha256rnds2_epu32(state1_a, state0_a, msg);
        msg = _mm_add_epi32(msg3_b, k);
        state1_b = _mm_sha256rnds2_epu32(state1_b, state0_b, msg);

        let tmp_a = _mm_alignr_epi8(msg3_a, msg2_a, 4);
        let tmp_b = _mm_alignr_epi8(msg3_b, msg2_b, 4);
        msg0_a = _mm_add_epi32(msg0_a, tmp_a);
        msg0_b = _mm_add_epi32(msg0_b, tmp_b);
        msg0_a = _mm_sha256msg2_epu32(msg0_a, msg3_a);
        msg0_b = _mm_sha256msg2_epu32(msg0_b, msg3_b);

        msg = _mm_shuffle_epi32(_mm_add_epi32(msg3_a, k), 0x0E);
        state0_a = _mm_sha256rnds2_epu32(state0_a, state1_a, msg);
        msg = _mm_shuffle_epi32(_mm_add_epi32(msg3_b, k), 0x0E);
        state0_b = _mm_sha256rnds2_epu32(state0_b, state1_b, msg);

        msg2_a = _mm_sha256msg1_epu32(msg2_a, msg3_a);
        msg2_b = _mm_sha256msg1_epu32(msg2_b, msg3_b);
    }

    // Macro for rounds 16-51 with full message schedule
    macro_rules! rounds_full_2way {
        ($round:expr, $cur_a:ident, $cur_b:ident, $prev_a:ident, $prev_b:ident, $prev2_a:ident, $prev2_b:ident, $next_a:ident, $next_b:ident) => {
            let k = _mm_loadu_si128(K_SIMD[$round / 4].as_ptr() as *const __m128i);
            let mut msg = _mm_add_epi32($cur_a, k);
            state1_a = _mm_sha256rnds2_epu32(state1_a, state0_a, msg);
            msg = _mm_add_epi32($cur_b, k);
            state1_b = _mm_sha256rnds2_epu32(state1_b, state0_b, msg);

            let tmp_a = _mm_alignr_epi8($cur_a, $prev_a, 4);
            let tmp_b = _mm_alignr_epi8($cur_b, $prev_b, 4);
            $next_a = _mm_add_epi32($next_a, tmp_a);
            $next_b = _mm_add_epi32($next_b, tmp_b);
            $next_a = _mm_sha256msg2_epu32($next_a, $cur_a);
            $next_b = _mm_sha256msg2_epu32($next_b, $cur_b);

            msg = _mm_shuffle_epi32(_mm_add_epi32($cur_a, k), 0x0E);
            state0_a = _mm_sha256rnds2_epu32(state0_a, state1_a, msg);
            msg = _mm_shuffle_epi32(_mm_add_epi32($cur_b, k), 0x0E);
            state0_b = _mm_sha256rnds2_epu32(state0_b, state1_b, msg);

            $prev2_a = _mm_sha256msg1_epu32($prev2_a, $cur_a);
            $prev2_b = _mm_sha256msg1_epu32($prev2_b, $cur_b);
        };
    }

    // Rounds 16-19
    rounds_full_2way!(16, msg0_a, msg0_b, msg3_a, msg3_b, msg3_a, msg3_b, msg1_a, msg1_b);
    // Rounds 20-23
    rounds_full_2way!(20, msg1_a, msg1_b, msg0_a, msg0_b, msg0_a, msg0_b, msg2_a, msg2_b);
    // Rounds 24-27
    rounds_full_2way!(24, msg2_a, msg2_b, msg1_a, msg1_b, msg1_a, msg1_b, msg3_a, msg3_b);
    // Rounds 28-31
    rounds_full_2way!(28, msg3_a, msg3_b, msg2_a, msg2_b, msg2_a, msg2_b, msg0_a, msg0_b);
    // Rounds 32-35
    rounds_full_2way!(32, msg0_a, msg0_b, msg3_a, msg3_b, msg3_a, msg3_b, msg1_a, msg1_b);
    // Rounds 36-39
    rounds_full_2way!(36, msg1_a, msg1_b, msg0_a, msg0_b, msg0_a, msg0_b, msg2_a, msg2_b);
    // Rounds 40-43
    rounds_full_2way!(40, msg2_a, msg2_b, msg1_a, msg1_b, msg1_a, msg1_b, msg3_a, msg3_b);
    // Rounds 44-47
    rounds_full_2way!(44, msg3_a, msg3_b, msg2_a, msg2_b, msg2_a, msg2_b, msg0_a, msg0_b);
    // Rounds 48-51
    rounds_full_2way!(48, msg0_a, msg0_b, msg3_a, msg3_b, msg3_a, msg3_b, msg1_a, msg1_b);

    // Rounds 52-55 (no msg1 expansion needed after)
    {
        let k = _mm_loadu_si128(K_SIMD[13].as_ptr() as *const __m128i);
        let mut msg = _mm_add_epi32(msg1_a, k);
        state1_a = _mm_sha256rnds2_epu32(state1_a, state0_a, msg);
        msg = _mm_add_epi32(msg1_b, k);
        state1_b = _mm_sha256rnds2_epu32(state1_b, state0_b, msg);

        let tmp_a = _mm_alignr_epi8(msg1_a, msg0_a, 4);
        let tmp_b = _mm_alignr_epi8(msg1_b, msg0_b, 4);
        msg2_a = _mm_add_epi32(msg2_a, tmp_a);
        msg2_b = _mm_add_epi32(msg2_b, tmp_b);
        msg2_a = _mm_sha256msg2_epu32(msg2_a, msg1_a);
        msg2_b = _mm_sha256msg2_epu32(msg2_b, msg1_b);

        msg = _mm_shuffle_epi32(_mm_add_epi32(msg1_a, k), 0x0E);
        state0_a = _mm_sha256rnds2_epu32(state0_a, state1_a, msg);
        msg = _mm_shuffle_epi32(_mm_add_epi32(msg1_b, k), 0x0E);
        state0_b = _mm_sha256rnds2_epu32(state0_b, state1_b, msg);
    }

    // Rounds 56-59
    {
        let k = _mm_loadu_si128(K_SIMD[14].as_ptr() as *const __m128i);
        let mut msg = _mm_add_epi32(msg2_a, k);
        state1_a = _mm_sha256rnds2_epu32(state1_a, state0_a, msg);
        msg = _mm_add_epi32(msg2_b, k);
        state1_b = _mm_sha256rnds2_epu32(state1_b, state0_b, msg);

        let tmp_a = _mm_alignr_epi8(msg2_a, msg1_a, 4);
        let tmp_b = _mm_alignr_epi8(msg2_b, msg1_b, 4);
        msg3_a = _mm_add_epi32(msg3_a, tmp_a);
        msg3_b = _mm_add_epi32(msg3_b, tmp_b);
        msg3_a = _mm_sha256msg2_epu32(msg3_a, msg2_a);
        msg3_b = _mm_sha256msg2_epu32(msg3_b, msg2_b);

        msg = _mm_shuffle_epi32(_mm_add_epi32(msg2_a, k), 0x0E);
        state0_a = _mm_sha256rnds2_epu32(state0_a, state1_a, msg);
        msg = _mm_shuffle_epi32(_mm_add_epi32(msg2_b, k), 0x0E);
        state0_b = _mm_sha256rnds2_epu32(state0_b, state1_b, msg);
    }

    // Rounds 60-63 (final)
    {
        let k = _mm_loadu_si128(K_SIMD[15].as_ptr() as *const __m128i);
        let mut msg = _mm_add_epi32(msg3_a, k);
        state1_a = _mm_sha256rnds2_epu32(state1_a, state0_a, msg);
        msg = _mm_add_epi32(msg3_b, k);
        state1_b = _mm_sha256rnds2_epu32(state1_b, state0_b, msg);
        msg = _mm_shuffle_epi32(_mm_add_epi32(msg3_a, k), 0x0E);
        state0_a = _mm_sha256rnds2_epu32(state0_a, state1_a, msg);
        msg = _mm_shuffle_epi32(_mm_add_epi32(msg3_b, k), 0x0E);
        state0_b = _mm_sha256rnds2_epu32(state0_b, state1_b, msg);
    }

    // Add to original state
    state0_a = _mm_add_epi32(state0_a, abef_save_a);
    state0_b = _mm_add_epi32(state0_b, abef_save_b);
    state1_a = _mm_add_epi32(state1_a, cdgh_save_a);
    state1_b = _mm_add_epi32(state1_b, cdgh_save_b);

    // Extract states back to memory
    let tmp_a = _mm_shuffle_epi32(state0_a, 0x1B);
    let tmp_b = _mm_shuffle_epi32(state0_b, 0x1B);
    state1_a = _mm_shuffle_epi32(state1_a, 0xB1);
    state1_b = _mm_shuffle_epi32(state1_b, 0xB1);
    let out0_a = _mm_blend_epi16(tmp_a, state1_a, 0xF0);
    let out0_b = _mm_blend_epi16(tmp_b, state1_b, 0xF0);
    let out1_a = _mm_alignr_epi8(state1_a, tmp_a, 8);
    let out1_b = _mm_alignr_epi8(state1_b, tmp_b, 8);

    _mm_storeu_si128(state_a.as_mut_ptr() as *mut __m128i, out0_a);
    _mm_storeu_si128(state_a.as_mut_ptr().add(4) as *mut __m128i, out1_a);
    _mm_storeu_si128(state_b.as_mut_ptr() as *mut __m128i, out0_b);
    _mm_storeu_si128(state_b.as_mut_ptr().add(4) as *mut __m128i, out1_b);
}

/// 2-way interleaved double SHA-256 with midstate for mining
///
/// Processes two nonces simultaneously to hide SHA-NI instruction latency.
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
unsafe fn sha256d_midstate_2way(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce0: u32,
    nonce1: u32,
) -> ([u8; 32], [u8; 32]) {
    // Build blocks for both nonces (80-byte message layout)
    let mut block_a = [0u8; 64];
    let mut block_b = [0u8; 64];

    block_a[..12].copy_from_slice(&tail[..12]);
    block_b[..12].copy_from_slice(&tail[..12]);
    block_a[12..16].copy_from_slice(&nonce0.to_le_bytes());
    block_b[12..16].copy_from_slice(&nonce1.to_le_bytes());
    // Padding for 80-byte message
    block_a[16] = 0x80;
    block_b[16] = 0x80;
    block_a[62] = 0x02;
    block_b[62] = 0x02;
    block_a[63] = 0x80;
    block_b[63] = 0x80;

    // First hash (continuing from midstate) - 2-way
    let mut state_a = midstate.state;
    let mut state_b = midstate.state;
    transform_2way(&mut state_a, &mut state_b, &block_a, &block_b);

    let first_hash_a = state_to_bytes(&state_a);
    let first_hash_b = state_to_bytes(&state_b);

    // Second hash - 2-way
    let mut state_a = H_INIT;
    let mut state_b = H_INIT;

    let mut block2_a = [0u8; 64];
    let mut block2_b = [0u8; 64];
    block2_a[..32].copy_from_slice(&first_hash_a);
    block2_b[..32].copy_from_slice(&first_hash_b);
    block2_a[32] = 0x80;
    block2_b[32] = 0x80;
    block2_a[62] = 0x01;
    block2_b[62] = 0x01;

    transform_2way(&mut state_a, &mut state_b, &block2_a, &block2_b);

    (state_to_bytes(&state_a), state_to_bytes(&state_b))
}

/// SHA-NI accelerated mining loop
///
/// Uses sequential processing. 2-way interleaving was tested but showed
/// no improvement due to SHA-NI already being well-pipelined on modern CPUs.
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

/// SHA-NI accelerated mining loop with 2-way interleaving (experimental)
///
/// Processes 2 nonces per iteration to try hiding instruction latency.
/// Testing showed no improvement - kept for reference.
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
#[allow(dead_code)]
pub unsafe fn mine_2way(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    // Process 2 nonces at a time
    let pairs = nonce_count / 2;
    for i in 0..pairs {
        let nonce0 = nonce_start.wrapping_add(i * 2);
        let nonce1 = nonce_start.wrapping_add(i * 2 + 1);

        let (hash0, hash1) = sha256d_midstate_2way(midstate, tail, nonce0, nonce1);

        if hash_meets_target(&hash0, target) {
            return Some((nonce0, hash0));
        }
        if hash_meets_target(&hash1, target) {
            return Some((nonce1, hash1));
        }
    }

    // Handle odd remainder
    if nonce_count % 2 == 1 {
        let nonce = nonce_start.wrapping_add(nonce_count - 1);
        let hash = sha256d_midstate(midstate, tail, nonce);
        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }

    None
}
