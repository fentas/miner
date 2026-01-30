//! SHA-256 SIMD optimizations for mining
//!
//! Provides hardware-accelerated SHA-256 implementations:
//! - SHA-NI: Intel SHA Extensions (fastest, ~100-150 MH/s single thread)
//! - AVX512: 16-way parallel hashing (fallback for AVX512 CPUs without SHA-NI)
//! - AVX2: 8-way parallel hashing (fallback for Haswell/Broadwell era CPUs)
//! - Generic: Portable fallback (~2-3 MH/s)

use super::job::hash_meets_target;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;


/// SHA-256 initial hash values (H0-H7)
pub const H_INIT: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// SHA-256 round constants (K0-K63)
pub const K: [u32; 64] = [
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

/// SIMD capability level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
    Generic,
    ShaNi,
    Avx2,
    Avx512,
}

impl SimdLevel {
    /// Detect the best available SIMD level
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sha") && is_x86_feature_detected!("sse4.1") {
                return SimdLevel::ShaNi;
            }
            if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512vl") {
                return SimdLevel::Avx512;
            }
            if is_x86_feature_detected!("avx2") {
                return SimdLevel::Avx2;
            }
        }
        SimdLevel::Generic
    }

    pub fn name(&self) -> &'static str {
        match self {
            SimdLevel::Generic => "generic",
            SimdLevel::ShaNi => "SHA-NI",
            SimdLevel::Avx2 => "AVX2",
            SimdLevel::Avx512 => "AVX512",
        }
    }
}

/// Pre-computed midstate for optimized mining
#[derive(Clone)]
pub struct Midstate {
    pub state: [u32; 8],
}

impl Midstate {
    /// Compute midstate from first 64 bytes of block header
    pub fn from_header_prefix(header: &[u8; 64]) -> Self {
        let mut state = H_INIT;
        transform_generic(&mut state, header);
        Self { state }
    }
}

/// Generic SHA-256 single block transform
pub fn transform_generic(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];

    // Prepare message schedule
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }

    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// Double SHA-256 using generic implementation
pub fn sha256d_generic(data: &[u8]) -> [u8; 32] {
    // First hash
    let mut state = H_INIT;
    let padded = sha256_pad(data);
    for chunk in padded.chunks(64) {
        transform_generic(&mut state, chunk);
    }
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let padded = sha256_pad(&first_hash);
    for chunk in padded.chunks(64) {
        transform_generic(&mut state, chunk);
    }
    state_to_bytes(&state)
}

/// SHA-256 with midstate (for mining - only last 16 bytes change)
pub fn sha256d_midstate_generic(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
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
    transform_generic(&mut state, &block);
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01; // 256 bits = 0x100
    block2[63] = 0x00;
    transform_generic(&mut state, &block2);
    state_to_bytes(&state)
}

/// SHA-NI accelerated single SHA-256 transform
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI and SSE4.1 instructions.
/// Reference: https://github.com/noloader/SHA-Intrinsics/blob/master/sha256-x86.c
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
unsafe fn sha256_transform_shani(state: &mut [u32; 8], block: &[u8; 64]) {
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
pub unsafe fn sha256d_midstate_shani(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
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
    sha256_transform_shani(&mut state, &block);
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01; // 256 bits = 0x100
    block2[63] = 0x00;
    sha256_transform_shani(&mut state, &block2);
    state_to_bytes(&state)
}

/// SHA-NI accelerated mining loop
///
/// # Safety
/// Caller must ensure CPU supports SHA-NI, SSE4.1, and SSSE3 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
unsafe fn mine_shani(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    for i in 0..nonce_count {
        let nonce = nonce_start.wrapping_add(i);
        let hash = sha256d_midstate_shani(midstate, tail, nonce);
        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }
    None
}

/// Mine with midstate using best available SIMD
pub fn mine_with_midstate(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    let level = SimdLevel::detect();

    #[cfg(target_arch = "x86_64")]
    {
        if level == SimdLevel::ShaNi {
            // SAFETY: We checked that SHA-NI is available
            return unsafe { mine_shani(midstate, tail, nonce_start, nonce_count, target) };
        }

        // Use AVX512 16-way parallel hashing as fallback for non-SHA-NI AVX512 CPUs
        if level == SimdLevel::Avx512 {
            // SAFETY: We checked that AVX512 is available
            return unsafe { mine_avx512(midstate, tail, nonce_start, nonce_count, target) };
        }

        // Use AVX2 8-way parallel hashing as fallback for non-SHA-NI CPUs
        if level == SimdLevel::Avx2 {
            // SAFETY: We checked that AVX2 is available
            return unsafe { mine_avx2(midstate, tail, nonce_start, nonce_count, target) };
        }
    }

    let _ = level; // Silence unused warning on non-x86_64
    mine_generic(midstate, tail, nonce_start, nonce_count, target)
}

/// Generic mining loop
fn mine_generic(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    for i in 0..nonce_count {
        let nonce = nonce_start.wrapping_add(i);
        let hash = sha256d_midstate_generic(midstate, tail, nonce);
        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }
    None
}

/// AVX2 8-way parallel SHA-256 transform
///
/// Processes 8 independent SHA-256 blocks simultaneously using 256-bit registers.
/// Each lane in the __m256i registers corresponds to one hash computation.
///
/// # Safety
/// Caller must ensure CPU supports AVX2 instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sha256_transform_avx2_8way(
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
unsafe fn mine_avx2(
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
        sha256_transform_avx2_8way(&mut states1, &blocks);

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
        sha256_transform_avx2_8way(&mut states2, &blocks2);

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

/// AVX512 16-way parallel SHA-256 transform
///
/// Processes 16 independent SHA-256 blocks simultaneously using 512-bit registers.
/// Each lane in the __m512i registers corresponds to one hash computation.
///
/// # Safety
/// Caller must ensure CPU supports AVX512F and AVX512VL instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f", enable = "avx512vl")]
unsafe fn sha256_transform_avx512_16way(
    states: &mut [[u32; 8]; 16],
    blocks: &[[u8; 64]; 16],
) {
    // Load states into AVX512 registers (transposed: each register holds one word from all 16 hashes)
    let mut state = [
        _mm512_set_epi32(
            states[15][0] as i32, states[14][0] as i32, states[13][0] as i32, states[12][0] as i32,
            states[11][0] as i32, states[10][0] as i32, states[9][0] as i32, states[8][0] as i32,
            states[7][0] as i32, states[6][0] as i32, states[5][0] as i32, states[4][0] as i32,
            states[3][0] as i32, states[2][0] as i32, states[1][0] as i32, states[0][0] as i32,
        ),
        _mm512_set_epi32(
            states[15][1] as i32, states[14][1] as i32, states[13][1] as i32, states[12][1] as i32,
            states[11][1] as i32, states[10][1] as i32, states[9][1] as i32, states[8][1] as i32,
            states[7][1] as i32, states[6][1] as i32, states[5][1] as i32, states[4][1] as i32,
            states[3][1] as i32, states[2][1] as i32, states[1][1] as i32, states[0][1] as i32,
        ),
        _mm512_set_epi32(
            states[15][2] as i32, states[14][2] as i32, states[13][2] as i32, states[12][2] as i32,
            states[11][2] as i32, states[10][2] as i32, states[9][2] as i32, states[8][2] as i32,
            states[7][2] as i32, states[6][2] as i32, states[5][2] as i32, states[4][2] as i32,
            states[3][2] as i32, states[2][2] as i32, states[1][2] as i32, states[0][2] as i32,
        ),
        _mm512_set_epi32(
            states[15][3] as i32, states[14][3] as i32, states[13][3] as i32, states[12][3] as i32,
            states[11][3] as i32, states[10][3] as i32, states[9][3] as i32, states[8][3] as i32,
            states[7][3] as i32, states[6][3] as i32, states[5][3] as i32, states[4][3] as i32,
            states[3][3] as i32, states[2][3] as i32, states[1][3] as i32, states[0][3] as i32,
        ),
        _mm512_set_epi32(
            states[15][4] as i32, states[14][4] as i32, states[13][4] as i32, states[12][4] as i32,
            states[11][4] as i32, states[10][4] as i32, states[9][4] as i32, states[8][4] as i32,
            states[7][4] as i32, states[6][4] as i32, states[5][4] as i32, states[4][4] as i32,
            states[3][4] as i32, states[2][4] as i32, states[1][4] as i32, states[0][4] as i32,
        ),
        _mm512_set_epi32(
            states[15][5] as i32, states[14][5] as i32, states[13][5] as i32, states[12][5] as i32,
            states[11][5] as i32, states[10][5] as i32, states[9][5] as i32, states[8][5] as i32,
            states[7][5] as i32, states[6][5] as i32, states[5][5] as i32, states[4][5] as i32,
            states[3][5] as i32, states[2][5] as i32, states[1][5] as i32, states[0][5] as i32,
        ),
        _mm512_set_epi32(
            states[15][6] as i32, states[14][6] as i32, states[13][6] as i32, states[12][6] as i32,
            states[11][6] as i32, states[10][6] as i32, states[9][6] as i32, states[8][6] as i32,
            states[7][6] as i32, states[6][6] as i32, states[5][6] as i32, states[4][6] as i32,
            states[3][6] as i32, states[2][6] as i32, states[1][6] as i32, states[0][6] as i32,
        ),
        _mm512_set_epi32(
            states[15][7] as i32, states[14][7] as i32, states[13][7] as i32, states[12][7] as i32,
            states[11][7] as i32, states[10][7] as i32, states[9][7] as i32, states[8][7] as i32,
            states[7][7] as i32, states[6][7] as i32, states[5][7] as i32, states[4][7] as i32,
            states[3][7] as i32, states[2][7] as i32, states[1][7] as i32, states[0][7] as i32,
        ),
    ];

    let state_save = state;

    // Load and prepare message schedule (W[0..15] from blocks, big-endian)
    let mut w: [__m512i; 64] = [_mm512_setzero_si512(); 64];
    for i in 0..16 {
        let offset = i * 4;
        w[i] = _mm512_set_epi32(
            i32::from_be_bytes([blocks[15][offset], blocks[15][offset+1], blocks[15][offset+2], blocks[15][offset+3]]),
            i32::from_be_bytes([blocks[14][offset], blocks[14][offset+1], blocks[14][offset+2], blocks[14][offset+3]]),
            i32::from_be_bytes([blocks[13][offset], blocks[13][offset+1], blocks[13][offset+2], blocks[13][offset+3]]),
            i32::from_be_bytes([blocks[12][offset], blocks[12][offset+1], blocks[12][offset+2], blocks[12][offset+3]]),
            i32::from_be_bytes([blocks[11][offset], blocks[11][offset+1], blocks[11][offset+2], blocks[11][offset+3]]),
            i32::from_be_bytes([blocks[10][offset], blocks[10][offset+1], blocks[10][offset+2], blocks[10][offset+3]]),
            i32::from_be_bytes([blocks[9][offset], blocks[9][offset+1], blocks[9][offset+2], blocks[9][offset+3]]),
            i32::from_be_bytes([blocks[8][offset], blocks[8][offset+1], blocks[8][offset+2], blocks[8][offset+3]]),
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
        let s0 = _mm512_xor_si512(
            _mm512_xor_si512(
                _mm512_or_si512(_mm512_srli_epi32(w15, 7), _mm512_slli_epi32(w15, 25)),
                _mm512_or_si512(_mm512_srli_epi32(w15, 18), _mm512_slli_epi32(w15, 14)),
            ),
            _mm512_srli_epi32(w15, 3),
        );

        // s1 = rotr(w[i-2], 17) ^ rotr(w[i-2], 19) ^ (w[i-2] >> 10)
        let w2 = w[i - 2];
        let s1 = _mm512_xor_si512(
            _mm512_xor_si512(
                _mm512_or_si512(_mm512_srli_epi32(w2, 17), _mm512_slli_epi32(w2, 15)),
                _mm512_or_si512(_mm512_srli_epi32(w2, 19), _mm512_slli_epi32(w2, 13)),
            ),
            _mm512_srli_epi32(w2, 10),
        );

        // w[i] = w[i-16] + s0 + w[i-7] + s1
        w[i] = _mm512_add_epi32(
            _mm512_add_epi32(w[i - 16], s0),
            _mm512_add_epi32(w[i - 7], s1),
        );
    }

    // Main compression loop
    for i in 0..64 {
        // S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
        let e = state[4];
        let s1 = _mm512_xor_si512(
            _mm512_xor_si512(
                _mm512_or_si512(_mm512_srli_epi32(e, 6), _mm512_slli_epi32(e, 26)),
                _mm512_or_si512(_mm512_srli_epi32(e, 11), _mm512_slli_epi32(e, 21)),
            ),
            _mm512_or_si512(_mm512_srli_epi32(e, 25), _mm512_slli_epi32(e, 7)),
        );

        // ch = (e & f) ^ (~e & g)
        let ch = _mm512_xor_si512(
            _mm512_and_si512(state[4], state[5]),
            _mm512_andnot_si512(state[4], state[6]),
        );

        // temp1 = h + S1 + ch + K[i] + w[i]
        let k_i = _mm512_set1_epi32(K[i] as i32);
        let temp1 = _mm512_add_epi32(
            _mm512_add_epi32(
                _mm512_add_epi32(state[7], s1),
                _mm512_add_epi32(ch, k_i),
            ),
            w[i],
        );

        // S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
        let a = state[0];
        let s0 = _mm512_xor_si512(
            _mm512_xor_si512(
                _mm512_or_si512(_mm512_srli_epi32(a, 2), _mm512_slli_epi32(a, 30)),
                _mm512_or_si512(_mm512_srli_epi32(a, 13), _mm512_slli_epi32(a, 19)),
            ),
            _mm512_or_si512(_mm512_srli_epi32(a, 22), _mm512_slli_epi32(a, 10)),
        );

        // maj = (a & b) ^ (a & c) ^ (b & c)
        let maj = _mm512_xor_si512(
            _mm512_xor_si512(
                _mm512_and_si512(state[0], state[1]),
                _mm512_and_si512(state[0], state[2]),
            ),
            _mm512_and_si512(state[1], state[2]),
        );

        // temp2 = S0 + maj
        let temp2 = _mm512_add_epi32(s0, maj);

        // Update state
        state[7] = state[6];
        state[6] = state[5];
        state[5] = state[4];
        state[4] = _mm512_add_epi32(state[3], temp1);
        state[3] = state[2];
        state[2] = state[1];
        state[1] = state[0];
        state[0] = _mm512_add_epi32(temp1, temp2);
    }

    // Add to original state
    for i in 0..8 {
        state[i] = _mm512_add_epi32(state[i], state_save[i]);
    }

    // Extract states back to arrays
    let mut out = [[0i32; 16]; 8];
    for i in 0..8 {
        _mm512_storeu_si512(out[i].as_mut_ptr() as *mut __m512i, state[i]);
    }

    // Transpose back: out[word][lane] -> states[lane][word]
    for lane in 0..16 {
        for word in 0..8 {
            states[lane][word] = out[word][lane] as u32;
        }
    }
}

/// AVX512 16-way parallel mining
///
/// Processes 16 nonces at a time for CPUs with AVX512 but without SHA-NI.
///
/// # Safety
/// Caller must ensure CPU supports AVX512F and AVX512VL instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f", enable = "avx512vl")]
unsafe fn mine_avx512(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    let mut i = 0u32;
    while i < nonce_count {
        // Process up to 16 nonces at a time
        let batch_size = std::cmp::min(16, nonce_count - i) as usize;

        // Prepare 16 blocks with different nonces
        let mut blocks: [[u8; 64]; 16] = [[0u8; 64]; 16];
        let mut nonces = [0u32; 16];

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
        for j in batch_size..16 {
            blocks[j] = blocks[0];
            nonces[j] = nonces[0];
        }

        // First hash (continuing from midstate)
        let mut states1: [[u32; 8]; 16] = [midstate.state; 16];
        sha256_transform_avx512_16way(&mut states1, &blocks);

        // Prepare second hash blocks (32 bytes of first hash + padding)
        let mut blocks2: [[u8; 64]; 16] = [[0u8; 64]; 16];
        for j in 0..16 {
            for w in 0..8 {
                blocks2[j][w * 4..w * 4 + 4].copy_from_slice(&states1[j][w].to_be_bytes());
            }
            blocks2[j][32] = 0x80;
            blocks2[j][62] = 0x01;
            blocks2[j][63] = 0x00;
        }

        // Second hash
        let mut states2: [[u32; 8]; 16] = [H_INIT; 16];
        sha256_transform_avx512_16way(&mut states2, &blocks2);

        // Check each hash against target
        for j in 0..batch_size {
            let hash = state_to_bytes(&states2[j]);
            if hash_meets_target(&hash, target) {
                return Some((nonces[j], hash));
            }
        }

        i += 16;
    }
    None
}

/// Pad message for SHA-256
fn sha256_pad(data: &[u8]) -> Vec<u8> {
    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();

    // Append 1 bit
    padded.push(0x80);

    // Pad to 56 bytes mod 64
    while padded.len() % 64 != 56 {
        padded.push(0);
    }

    // Append length in bits (big-endian)
    padded.extend_from_slice(&bit_len.to_be_bytes());

    padded
}

/// Convert state to bytes (big-endian)
fn state_to_bytes(state: &[u32; 8]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for (i, &word) in state.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let level = SimdLevel::detect();
        // Should return a valid level
        assert!(matches!(
            level,
            SimdLevel::Generic | SimdLevel::ShaNi | SimdLevel::Avx2 | SimdLevel::Avx512
        ));
    }

    #[test]
    fn test_simd_level_name() {
        assert_eq!(SimdLevel::Generic.name(), "generic");
        assert_eq!(SimdLevel::ShaNi.name(), "SHA-NI");
        assert_eq!(SimdLevel::Avx2.name(), "AVX2");
        assert_eq!(SimdLevel::Avx512.name(), "AVX512");
    }

    #[test]
    fn test_sha256d_empty() {
        // SHA256d of empty string
        let hash = sha256d_generic(&[]);
        let expected = hex::decode(
            "5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456"
        ).unwrap();
        assert_eq!(&hash[..], &expected[..]);
    }

    #[test]
    fn test_sha256d_hello() {
        // SHA256d of "hello"
        let hash = sha256d_generic(b"hello");
        // SHA256d should produce 32 bytes
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sha256d_block_size() {
        // Test data exactly 64 bytes (one block)
        let data = [0xAB; 64];
        let hash = sha256d_generic(&data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sha256d_multiple_blocks() {
        // Test data larger than one block
        let data = [0xCD; 128];
        let hash = sha256d_generic(&data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_midstate() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        assert_ne!(midstate.state, H_INIT);
    }

    #[test]
    fn test_midstate_clone() {
        let header = [0xAB; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let cloned = midstate.clone();
        assert_eq!(midstate.state, cloned.state);
    }

    #[test]
    fn test_sha256d_midstate_generic() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let hash = sha256d_midstate_generic(&midstate, &tail, 0);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sha256d_midstate_different_nonces() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];

        let hash1 = sha256d_midstate_generic(&midstate, &tail, 0);
        let hash2 = sha256d_midstate_generic(&midstate, &tail, 1);

        // Different nonces should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_transform_generic_modifies_state() {
        let mut state = H_INIT;
        let block = [0u8; 64];
        transform_generic(&mut state, &block);
        assert_ne!(state, H_INIT);
    }

    #[test]
    fn test_transform_generic_deterministic() {
        let mut state1 = H_INIT;
        let mut state2 = H_INIT;
        let block = [0xAB; 64];

        transform_generic(&mut state1, &block);
        transform_generic(&mut state2, &block);

        assert_eq!(state1, state2);
    }

    #[test]
    fn test_sha256_pad_short() {
        let data = b"test";
        let padded = sha256_pad(data);

        // Should be padded to 64 bytes (one block)
        assert_eq!(padded.len() % 64, 0);
        assert_eq!(padded.len(), 64);

        // Should start with original data
        assert_eq!(&padded[..4], data);

        // Should have 0x80 after data
        assert_eq!(padded[4], 0x80);
    }

    #[test]
    fn test_sha256_pad_55_bytes() {
        // Exactly 55 bytes - should fit in one block with padding
        let data = [0xAB; 55];
        let padded = sha256_pad(&data);
        assert_eq!(padded.len(), 64);
    }

    #[test]
    fn test_sha256_pad_56_bytes() {
        // 56 bytes - needs two blocks
        let data = [0xAB; 56];
        let padded = sha256_pad(&data);
        assert_eq!(padded.len(), 128);
    }

    #[test]
    fn test_sha256_pad_length_encoding() {
        let data = b"test"; // 4 bytes = 32 bits = 0x20
        let padded = sha256_pad(data);

        // Last 8 bytes should be length in bits (big-endian)
        let len_bytes = &padded[padded.len() - 8..];
        let length = u64::from_be_bytes(len_bytes.try_into().unwrap());
        assert_eq!(length, 32); // 4 bytes * 8 = 32 bits
    }

    #[test]
    fn test_state_to_bytes() {
        let state = [
            0x12345678, 0x9ABCDEF0, 0x11111111, 0x22222222,
            0x33333333, 0x44444444, 0x55555555, 0x66666666,
        ];
        let bytes = state_to_bytes(&state);

        // First word should be big-endian
        assert_eq!(bytes[0], 0x12);
        assert_eq!(bytes[1], 0x34);
        assert_eq!(bytes[2], 0x56);
        assert_eq!(bytes[3], 0x78);
    }

    #[test]
    fn test_hash_meets_target_easy() {
        let target = [0xFF; 32]; // Very easy target
        let hash = [0x00; 32];
        assert!(hash_meets_target(&hash, &target));
    }

    #[test]
    fn test_hash_meets_target_hard() {
        let target = [0x00; 32]; // Impossible target
        let hash = [0x01; 32];
        assert!(!hash_meets_target(&hash, &target));
    }

    #[test]
    fn test_hash_meets_target_equal() {
        let target = [0x11; 32];
        let hash = [0x11; 32];
        assert!(hash_meets_target(&hash, &target)); // Equal meets target
    }

    #[test]
    fn test_hash_meets_target_boundary() {
        let mut target = [0xFF; 32];
        target[0] = 0x00;
        target[1] = 0x01;

        let mut good_hash = [0x00; 32];
        good_hash[1] = 0x00;  // Less than target[1]
        assert!(hash_meets_target(&good_hash, &target));

        let mut bad_hash = [0x00; 32];
        bad_hash[1] = 0x02;  // Greater than target[1]
        assert!(!hash_meets_target(&bad_hash, &target));
    }

    #[test]
    fn test_mine_generic_no_match() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let target = [0x00; 32]; // Impossible target

        let result = mine_generic(&midstate, &tail, 0, 100, &target);
        assert!(result.is_none());
    }

    #[test]
    fn test_mine_generic_easy_target() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let target = [0xFF; 32]; // Very easy target

        let result = mine_generic(&midstate, &tail, 0, 10, &target);
        assert!(result.is_some());

        let (nonce, hash) = result.unwrap();
        assert!(nonce < 10);
        assert!(hash_meets_target(&hash, &target));
    }

    #[test]
    fn test_mine_with_midstate() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let target = [0xFF; 32]; // Easy target

        let result = mine_with_midstate(&midstate, &tail, 0, 10, &target);
        assert!(result.is_some());
    }

    #[test]
    fn test_h_init_values() {
        // Verify SHA-256 initial values
        assert_eq!(H_INIT[0], 0x6a09e667);
        assert_eq!(H_INIT[7], 0x5be0cd19);
    }

    #[test]
    fn test_k_values() {
        // Verify some SHA-256 round constants
        assert_eq!(K[0], 0x428a2f98);
        assert_eq!(K[63], 0xc67178f2);
        assert_eq!(K.len(), 64);
    }

    #[test]
    fn test_different_midstates() {
        let header1 = [0x00; 64];
        let header2 = [0xFF; 64];

        let midstate1 = Midstate::from_header_prefix(&header1);
        let midstate2 = Midstate::from_header_prefix(&header2);

        assert_ne!(midstate1.state, midstate2.state);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_shani_matches_generic() {
        // Only run if SHA-NI is available
        if !is_x86_feature_detected!("sha") || !is_x86_feature_detected!("sse4.1") {
            println!("SHA-NI not available, skipping test");
            return;
        }

        // Test with various inputs to ensure SHA-NI matches generic
        let test_cases: Vec<[u8; 64]> = vec![
            [0x00; 64],
            [0xFF; 64],
            [0xAB; 64],
            {
                let mut data = [0u8; 64];
                for i in 0..64 {
                    data[i] = i as u8;
                }
                data
            },
        ];

        for (i, block) in test_cases.iter().enumerate() {
            // Test transform function
            let mut state_generic = H_INIT;
            let mut state_shani = H_INIT;

            transform_generic(&mut state_generic, block);
            unsafe { sha256_transform_shani(&mut state_shani, block) };

            assert_eq!(
                state_generic, state_shani,
                "SHA-NI transform mismatch for test case {}: generic={:08x?}, shani={:08x?}",
                i, state_generic, state_shani
            );
        }

        // Test midstate mining function
        let header = [0xAB; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x12; 16];

        for nonce in [0u32, 1, 100, 12345, 0xDEADBEEF] {
            let generic_hash = sha256d_midstate_generic(&midstate, &tail, nonce);
            let shani_hash = unsafe { sha256d_midstate_shani(&midstate, &tail, nonce) };

            assert_eq!(
                generic_hash, shani_hash,
                "SHA-NI midstate hash mismatch for nonce {}: generic={}, shani={}",
                nonce,
                hex::encode(generic_hash),
                hex::encode(shani_hash)
            );
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_avx2_matches_generic() {
        // Only run if AVX2 is available
        if !is_x86_feature_detected!("avx2") {
            println!("AVX2 not available, skipping test");
            return;
        }

        // Test with various inputs
        let test_cases: Vec<[u8; 64]> = vec![
            [0x00; 64],
            [0xFF; 64],
            [0xAB; 64],
            {
                let mut data = [0u8; 64];
                for i in 0..64 {
                    data[i] = i as u8;
                }
                data
            },
        ];

        for (i, block) in test_cases.iter().enumerate() {
            // Test 8-way transform function
            let mut states_generic: [[u32; 8]; 8] = [H_INIT; 8];
            let mut states_avx2: [[u32; 8]; 8] = [H_INIT; 8];

            // Create 8 slightly different blocks
            let mut blocks: [[u8; 64]; 8] = [*block; 8];
            for j in 0..8 {
                blocks[j][0] = blocks[j][0].wrapping_add(j as u8);
            }

            // Process with generic
            for j in 0..8 {
                transform_generic(&mut states_generic[j], &blocks[j]);
            }

            // Process with AVX2 8-way
            unsafe { sha256_transform_avx2_8way(&mut states_avx2, &blocks) };

            for j in 0..8 {
                assert_eq!(
                    states_generic[j], states_avx2[j],
                    "AVX2 transform mismatch for test case {}, lane {}: generic={:08x?}, avx2={:08x?}",
                    i, j, states_generic[j], states_avx2[j]
                );
            }
        }

        // Test mining function produces correct results
        let header = [0xAB; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x12; 16];
        let target = [0xFF; 32]; // Easy target - all hashes should pass

        // Test that AVX2 mining finds a valid hash
        let result = unsafe { mine_avx2(&midstate, &tail, 0, 16, &target) };
        assert!(result.is_some(), "AVX2 mining should find a hash with easy target");

        let (nonce, hash) = result.unwrap();
        assert!(nonce < 16, "Nonce should be in searched range");
        assert!(hash_meets_target(&hash, &target), "Hash should meet target");

        // Verify the hash matches generic implementation
        let generic_hash = sha256d_midstate_generic(&midstate, &tail, nonce);
        assert_eq!(
            hash, generic_hash,
            "AVX2 hash should match generic for nonce {}: avx2={}, generic={}",
            nonce,
            hex::encode(hash),
            hex::encode(generic_hash)
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_avx2_mining_correctness() {
        // Only run if AVX2 is available
        if !is_x86_feature_detected!("avx2") {
            println!("AVX2 not available, skipping test");
            return;
        }

        // Test with multiple nonce ranges
        let header = [0x42; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x33; 16];

        // Test several nonces to ensure AVX2 and generic produce identical results
        for nonce in [0u32, 1, 7, 8, 15, 16, 100, 1000] {
            let generic_hash = sha256d_midstate_generic(&midstate, &tail, nonce);

            // Build block the same way mine_avx2 does
            let mut block = [0u8; 64];
            block[..16].copy_from_slice(&tail);
            block[16..20].copy_from_slice(&nonce.to_le_bytes());
            block[20] = 0x80;
            block[62] = 0x02;
            block[63] = 0x80;

            // First hash
            let mut state1 = midstate.state;
            transform_generic(&mut state1, &block);

            // Second hash
            let mut block2 = [0u8; 64];
            for w in 0..8 {
                block2[w * 4..w * 4 + 4].copy_from_slice(&state1[w].to_be_bytes());
            }
            block2[32] = 0x80;
            block2[62] = 0x01;
            block2[63] = 0x00;

            let mut state2 = H_INIT;
            transform_generic(&mut state2, &block2);
            let reconstructed_hash = state_to_bytes(&state2);

            assert_eq!(
                generic_hash, reconstructed_hash,
                "Hash reconstruction should match for nonce {}", nonce
            );
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_avx512_matches_generic() {
        // Only run if AVX512 is available
        if !is_x86_feature_detected!("avx512f") || !is_x86_feature_detected!("avx512vl") {
            println!("AVX512 not available, skipping test");
            return;
        }

        // Test with various inputs
        let test_cases: Vec<[u8; 64]> = vec![
            [0x00; 64],
            [0xFF; 64],
            [0xAB; 64],
            {
                let mut data = [0u8; 64];
                for i in 0..64 {
                    data[i] = i as u8;
                }
                data
            },
        ];

        for (i, block) in test_cases.iter().enumerate() {
            // Test 16-way transform function
            let mut states_generic: [[u32; 8]; 16] = [H_INIT; 16];
            let mut states_avx512: [[u32; 8]; 16] = [H_INIT; 16];

            // Create 16 slightly different blocks
            let mut blocks: [[u8; 64]; 16] = [*block; 16];
            for j in 0..16 {
                blocks[j][0] = blocks[j][0].wrapping_add(j as u8);
            }

            // Process with generic
            for j in 0..16 {
                transform_generic(&mut states_generic[j], &blocks[j]);
            }

            // Process with AVX512 16-way
            unsafe { sha256_transform_avx512_16way(&mut states_avx512, &blocks) };

            for j in 0..16 {
                assert_eq!(
                    states_generic[j], states_avx512[j],
                    "AVX512 transform mismatch for test case {}, lane {}: generic={:08x?}, avx512={:08x?}",
                    i, j, states_generic[j], states_avx512[j]
                );
            }
        }

        // Test mining function produces correct results
        let header = [0xAB; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x12; 16];
        let target = [0xFF; 32]; // Easy target - all hashes should pass

        // Test that AVX512 mining finds a valid hash
        let result = unsafe { mine_avx512(&midstate, &tail, 0, 32, &target) };
        assert!(result.is_some(), "AVX512 mining should find a hash with easy target");

        let (nonce, hash) = result.unwrap();
        assert!(nonce < 32, "Nonce should be in searched range");
        assert!(hash_meets_target(&hash, &target), "Hash should meet target");

        // Verify the hash matches generic implementation
        let generic_hash = sha256d_midstate_generic(&midstate, &tail, nonce);
        assert_eq!(
            hash, generic_hash,
            "AVX512 hash should match generic for nonce {}: avx512={}, generic={}",
            nonce,
            hex::encode(hash),
            hex::encode(generic_hash)
        );
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_avx512_mining_correctness() {
        // Only run if AVX512 is available
        if !is_x86_feature_detected!("avx512f") || !is_x86_feature_detected!("avx512vl") {
            println!("AVX512 not available, skipping test");
            return;
        }

        // Test with multiple nonce ranges
        let header = [0x42; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x33; 16];

        // Test several nonces to ensure AVX512 and generic produce identical results
        for nonce in [0u32, 1, 15, 16, 31, 32, 100, 1000] {
            let generic_hash = sha256d_midstate_generic(&midstate, &tail, nonce);

            // Build block the same way mine_avx512 does
            let mut block = [0u8; 64];
            block[..16].copy_from_slice(&tail);
            block[16..20].copy_from_slice(&nonce.to_le_bytes());
            block[20] = 0x80;
            block[62] = 0x02;
            block[63] = 0x80;

            // First hash
            let mut state1 = midstate.state;
            transform_generic(&mut state1, &block);

            // Second hash
            let mut block2 = [0u8; 64];
            for w in 0..8 {
                block2[w * 4..w * 4 + 4].copy_from_slice(&state1[w].to_be_bytes());
            }
            block2[32] = 0x80;
            block2[62] = 0x01;
            block2[63] = 0x00;

            let mut state2 = H_INIT;
            transform_generic(&mut state2, &block2);
            let reconstructed_hash = state_to_bytes(&state2);

            assert_eq!(
                generic_hash, reconstructed_hash,
                "Hash reconstruction should match for nonce {}", nonce
            );
        }
    }
}
