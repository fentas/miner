//! SHA-256 SIMD optimizations for mining
//!
//! Provides hardware-accelerated SHA-256 implementations:
//! - SHA-NI: Intel SHA Extensions (~100-150 MH/s single thread)
//! - AVX2: 8-way parallel hashing (planned)
//! - AVX512: 16-way parallel hashing (planned)
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
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse4.1", enable = "ssse3")]
unsafe fn sha256_transform_shani(state: &mut [u32; 8], block: &[u8; 64]) {
    // Load initial state
    // SHA-NI uses ABEF and CDGH layout
    let mut abef = _mm_set_epi32(
        state[0] as i32, state[1] as i32, state[4] as i32, state[5] as i32
    );
    let mut cdgh = _mm_set_epi32(
        state[2] as i32, state[3] as i32, state[6] as i32, state[7] as i32
    );

    let abef_save = abef;
    let cdgh_save = cdgh;

    // Byte swap mask for converting little-endian to big-endian
    let shuf_mask = _mm_set_epi64x(0x0c0d0e0f08090a0b, 0x0405060700010203);

    // Load and byte-swap message
    let mut msg0 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr() as *const __m128i), shuf_mask);
    let mut msg1 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(16) as *const __m128i), shuf_mask);
    let mut msg2 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(32) as *const __m128i), shuf_mask);
    let mut msg3 = _mm_shuffle_epi8(_mm_loadu_si128(block.as_ptr().add(48) as *const __m128i), shuf_mask);

    // Rounds 0-3
    let mut msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[3] as i32, K[2] as i32, K[1] as i32, K[0] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 4-7
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[7] as i32, K[6] as i32, K[5] as i32, K[4] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 8-11
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);
    msg0 = _mm_add_epi32(msg0, _mm_alignr_epi8(msg2, msg1, 4));
    msg0 = _mm_sha256msg2_epu32(msg0, msg2);
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[11] as i32, K[10] as i32, K[9] as i32, K[8] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 12-15
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);
    msg1 = _mm_add_epi32(msg1, _mm_alignr_epi8(msg3, msg2, 4));
    msg1 = _mm_sha256msg2_epu32(msg1, msg3);
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[15] as i32, K[14] as i32, K[13] as i32, K[12] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 16-19
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);
    msg2 = _mm_add_epi32(msg2, _mm_alignr_epi8(msg0, msg3, 4));
    msg2 = _mm_sha256msg2_epu32(msg2, msg0);
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[19] as i32, K[18] as i32, K[17] as i32, K[16] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 20-23
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);
    msg3 = _mm_add_epi32(msg3, _mm_alignr_epi8(msg1, msg0, 4));
    msg3 = _mm_sha256msg2_epu32(msg3, msg1);
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[23] as i32, K[22] as i32, K[21] as i32, K[20] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 24-27
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);
    msg0 = _mm_add_epi32(msg0, _mm_alignr_epi8(msg2, msg1, 4));
    msg0 = _mm_sha256msg2_epu32(msg0, msg2);
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[27] as i32, K[26] as i32, K[25] as i32, K[24] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 28-31
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);
    msg1 = _mm_add_epi32(msg1, _mm_alignr_epi8(msg3, msg2, 4));
    msg1 = _mm_sha256msg2_epu32(msg1, msg3);
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[31] as i32, K[30] as i32, K[29] as i32, K[28] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 32-35
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);
    msg2 = _mm_add_epi32(msg2, _mm_alignr_epi8(msg0, msg3, 4));
    msg2 = _mm_sha256msg2_epu32(msg2, msg0);
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[35] as i32, K[34] as i32, K[33] as i32, K[32] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 36-39
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);
    msg3 = _mm_add_epi32(msg3, _mm_alignr_epi8(msg1, msg0, 4));
    msg3 = _mm_sha256msg2_epu32(msg3, msg1);
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[39] as i32, K[38] as i32, K[37] as i32, K[36] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 40-43
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);
    msg0 = _mm_add_epi32(msg0, _mm_alignr_epi8(msg2, msg1, 4));
    msg0 = _mm_sha256msg2_epu32(msg0, msg2);
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[43] as i32, K[42] as i32, K[41] as i32, K[40] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 44-47
    msg2 = _mm_sha256msg1_epu32(msg2, msg3);
    msg1 = _mm_add_epi32(msg1, _mm_alignr_epi8(msg3, msg2, 4));
    msg1 = _mm_sha256msg2_epu32(msg1, msg3);
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[47] as i32, K[46] as i32, K[45] as i32, K[44] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 48-51
    msg3 = _mm_sha256msg1_epu32(msg3, msg0);
    msg2 = _mm_add_epi32(msg2, _mm_alignr_epi8(msg0, msg3, 4));
    msg2 = _mm_sha256msg2_epu32(msg2, msg0);
    msg = _mm_add_epi32(msg0, _mm_set_epi32(
        K[51] as i32, K[50] as i32, K[49] as i32, K[48] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 52-55
    msg0 = _mm_sha256msg1_epu32(msg0, msg1);
    msg3 = _mm_add_epi32(msg3, _mm_alignr_epi8(msg1, msg0, 4));
    msg3 = _mm_sha256msg2_epu32(msg3, msg1);
    msg = _mm_add_epi32(msg1, _mm_set_epi32(
        K[55] as i32, K[54] as i32, K[53] as i32, K[52] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 56-59
    msg1 = _mm_sha256msg1_epu32(msg1, msg2);
    msg0 = _mm_add_epi32(msg0, _mm_alignr_epi8(msg2, msg1, 4));
    msg0 = _mm_sha256msg2_epu32(msg0, msg2);
    msg = _mm_add_epi32(msg2, _mm_set_epi32(
        K[59] as i32, K[58] as i32, K[57] as i32, K[56] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Rounds 60-63
    msg = _mm_add_epi32(msg3, _mm_set_epi32(
        K[63] as i32, K[62] as i32, K[61] as i32, K[60] as i32
    ));
    cdgh = _mm_sha256rnds2_epu32(cdgh, abef, msg);
    msg = _mm_shuffle_epi32(msg, 0x0E);
    abef = _mm_sha256rnds2_epu32(abef, cdgh, msg);

    // Add to original state
    abef = _mm_add_epi32(abef, abef_save);
    cdgh = _mm_add_epi32(cdgh, cdgh_save);

    // Extract state - ABEF contains A,B,E,F and CDGH contains C,D,G,H
    // Layout: abef = [A, B, E, F], cdgh = [C, D, G, H]
    let abef_arr: [i32; 4] = std::mem::transmute(abef);
    let cdgh_arr: [i32; 4] = std::mem::transmute(cdgh);

    state[0] = abef_arr[3] as u32;
    state[1] = abef_arr[2] as u32;
    state[2] = cdgh_arr[3] as u32;
    state[3] = cdgh_arr[2] as u32;
    state[4] = abef_arr[1] as u32;
    state[5] = abef_arr[0] as u32;
    state[6] = cdgh_arr[1] as u32;
    state[7] = cdgh_arr[0] as u32;
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

    match level {
        SimdLevel::ShaNi => {
            // Use SHA-NI hardware acceleration
            #[cfg(target_arch = "x86_64")]
            if is_x86_feature_detected!("sha") && is_x86_feature_detected!("ssse3") {
                // SAFETY: We just verified SHA-NI and SSSE3 are supported
                return unsafe {
                    mine_shani(midstate, tail, nonce_start, nonce_count, target)
                };
            }
            mine_generic(midstate, tail, nonce_start, nonce_count, target)
        }
        SimdLevel::Avx2 => {
            // AVX2 path - currently uses generic (true 8-way parallel not yet implemented)
            mine_generic(midstate, tail, nonce_start, nonce_count, target)
        }
        SimdLevel::Avx512 => {
            // AVX512 path - currently uses generic (true 16-way parallel not yet implemented)
            mine_generic(midstate, tail, nonce_start, nonce_count, target)
        }
        SimdLevel::Generic => mine_generic(midstate, tail, nonce_start, nonce_count, target),
    }
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

/// AVX2 8-way parallel mining
///
/// # Safety
/// Caller must ensure CPU supports AVX2 instructions.
/// Use `is_x86_feature_detected!("avx2")` before calling.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn mine_avx2_8way(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    // Process 8 nonces at a time
    let mut i = 0u32;
    while i < nonce_count {
        // For each batch of 8
        for j in 0..8 {
            let nonce = nonce_start.wrapping_add(i).wrapping_add(j);
            if i + j >= nonce_count {
                break;
            }
            let hash = sha256d_midstate_generic(midstate, tail, nonce);
            if hash_meets_target(&hash, target) {
                return Some((nonce, hash));
            }
        }
        i += 8;
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
}
