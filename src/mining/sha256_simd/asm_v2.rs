//! SHA-256 Optimized Implementation (v2)
//!
//! Key optimizations from sha2 crate analysis:
//! 1. cpufeatures crate for one-time CPU detection (not per-hash)
//! 2. Fused double-transform: keeps state in XMM registers for both SHA256 passes
//! 3. Clean macro structure that LLVM can optimize
//! 4. No #[target_feature] on public function - allows inlining
//!
//! Architecture:
//! - Public fn: sha256d_midstate_v2() - NO #[target_feature], does CPU check once
//! - Internal fn: digest_double() - has #[target_feature], does actual work
//! - Both in same compilation unit, so LLVM CAN inline the internal fn

use super::{H_INIT, Midstate, state_to_bytes};

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// Pre-computed K constants as [i32; 4] for direct loading
// Order is REVERSED within each group because _mm_set_epi32(e3,e2,e1,e0) puts e0 at lowest addr
// Using i32 to match _mm_set_epi32 signature
static K32X4: [[i32; 4]; 16] = [
    [0xe9b5dba5u32 as i32, 0xb5c0fbcfu32 as i32, 0x71374491u32 as i32, 0x428a2f98u32 as i32],
    [0xab1c5ed5u32 as i32, 0x923f82a4u32 as i32, 0x59f111f1u32 as i32, 0x3956c25bu32 as i32],
    [0x550c7dc3u32 as i32, 0x243185beu32 as i32, 0x12835b01u32 as i32, 0xd807aa98u32 as i32],
    [0xc19bf174u32 as i32, 0x9bdc06a7u32 as i32, 0x80deb1feu32 as i32, 0x72be5d74u32 as i32],
    [0x240ca1ccu32 as i32, 0x0fc19dc6u32 as i32, 0xefbe4786u32 as i32, 0xe49b69c1u32 as i32],
    [0x76f988dau32 as i32, 0x5cb0a9dcu32 as i32, 0x4a7484aau32 as i32, 0x2de92c6fu32 as i32],
    [0xbf597fc7u32 as i32, 0xb00327c8u32 as i32, 0xa831c66du32 as i32, 0x983e5152u32 as i32],
    [0x14292967u32 as i32, 0x06ca6351u32 as i32, 0xd5a79147u32 as i32, 0xc6e00bf3u32 as i32],
    [0x53380d13u32 as i32, 0x4d2c6dfcu32 as i32, 0x2e1b2138u32 as i32, 0x27b70a85u32 as i32],
    [0x92722c85u32 as i32, 0x81c2c92eu32 as i32, 0x766a0abbu32 as i32, 0x650a7354u32 as i32],
    [0xc76c51a3u32 as i32, 0xc24b8b70u32 as i32, 0xa81a664bu32 as i32, 0xa2bfe8a1u32 as i32],
    [0x106aa070u32 as i32, 0xf40e3585u32 as i32, 0xd6990624u32 as i32, 0xd192e819u32 as i32],
    [0x34b0bcb5u32 as i32, 0x2748774cu32 as i32, 0x1e376c08u32 as i32, 0x19a4c116u32 as i32],
    [0x682e6ff3u32 as i32, 0x5b9cca4fu32 as i32, 0x4ed8aa4au32 as i32, 0x391c0cb3u32 as i32],
    [0x8cc70208u32 as i32, 0x84c87814u32 as i32, 0x78a5636fu32 as i32, 0x748f82eeu32 as i32],
    [0xc67178f2u32 as i32, 0xbef9a3f7u32 as i32, 0xa4506cebu32 as i32, 0x90befffau32 as i32],
];

// One-time CPU feature detection using cpufeatures crate
// This creates a static that is checked once on first call
cpufeatures::new!(shani_cpuid, "sha", "sse2", "ssse3", "sse4.1");

/// Message schedule expansion
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn schedule(v0: __m128i, v1: __m128i, v2: __m128i, v3: __m128i) -> __m128i {
    let t1 = _mm_sha256msg1_epu32(v0, v1);
    let t2 = _mm_alignr_epi8(v3, v2, 4);
    let t3 = _mm_add_epi32(t1, t2);
    _mm_sha256msg2_epu32(t3, v3)
}

/// Process 4 rounds with given message word
/// Note: abef/cdgh passed by reference (&mut) for clarity
#[cfg(target_arch = "x86_64")]
macro_rules! rounds4 {
    ($abef:expr, $cdgh:expr, $msg:expr, $i:expr) => {{
        let k = K32X4[$i];
        let kv = _mm_set_epi32(k[0], k[1], k[2], k[3]);
        let t1 = _mm_add_epi32($msg, kv);
        $cdgh = _mm_sha256rnds2_epu32($cdgh, $abef, t1);
        let t2 = _mm_shuffle_epi32(t1, 0x0E);
        $abef = _mm_sha256rnds2_epu32($abef, $cdgh, t2);
    }};
}

/// Process 4 rounds with message schedule expansion
#[cfg(target_arch = "x86_64")]
macro_rules! schedule_rounds4 {
    ($abef:expr, $cdgh:expr, $w0:expr, $w1:expr, $w2:expr, $w3:expr, $w4:expr, $i:expr) => {{
        $w4 = schedule($w0, $w1, $w2, $w3);
        rounds4!($abef, $cdgh, $w4, $i);
    }};
}

/// Single block transform - returns updated (abef, cdgh) state
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse2", enable = "ssse3", enable = "sse4.1")]
#[inline]
unsafe fn transform_block(
    mut abef: __m128i,
    mut cdgh: __m128i,
    block: &[u8; 64],
) -> (__m128i, __m128i) {
    let mask: __m128i = _mm_set_epi64x(
        0x0C0D_0E0F_0809_0A0Bu64 as i64,
        0x0405_0607_0001_0203u64 as i64,
    );

    let abef_save = abef;
    let cdgh_save = cdgh;

    let data_ptr = block.as_ptr() as *const __m128i;
    let mut w0 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(0)), mask);
    let mut w1 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(1)), mask);
    let mut w2 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(2)), mask);
    let mut w3 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(3)), mask);
    let mut w4;

    rounds4!(abef, cdgh, w0, 0);
    rounds4!(abef, cdgh, w1, 1);
    rounds4!(abef, cdgh, w2, 2);
    rounds4!(abef, cdgh, w3, 3);
    schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 4);
    schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 5);
    schedule_rounds4!(abef, cdgh, w2, w3, w4, w0, w1, 6);
    schedule_rounds4!(abef, cdgh, w3, w4, w0, w1, w2, 7);
    schedule_rounds4!(abef, cdgh, w4, w0, w1, w2, w3, 8);
    schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 9);
    schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 10);
    schedule_rounds4!(abef, cdgh, w2, w3, w4, w0, w1, 11);
    schedule_rounds4!(abef, cdgh, w3, w4, w0, w1, w2, 12);
    schedule_rounds4!(abef, cdgh, w4, w0, w1, w2, w3, 13);
    schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 14);
    schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 15);

    abef = _mm_add_epi32(abef, abef_save);
    cdgh = _mm_add_epi32(cdgh, cdgh_save);

    (abef, cdgh)
}

/// Load state from [u32; 8] into ABEF/CDGH format
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse2", enable = "ssse3", enable = "sse4.1")]
#[inline]
unsafe fn load_state(state: &[u32; 8]) -> (__m128i, __m128i) {
    let state_ptr = state.as_ptr() as *const __m128i;
    let dcba = _mm_loadu_si128(state_ptr.add(0));
    let efgh = _mm_loadu_si128(state_ptr.add(1));

    let cdab = _mm_shuffle_epi32(dcba, 0xB1);
    let efgh = _mm_shuffle_epi32(efgh, 0x1B);
    let abef = _mm_alignr_epi8(cdab, efgh, 8);
    let cdgh = _mm_blend_epi16(efgh, cdab, 0xF0);

    (abef, cdgh)
}

/// Store state from ABEF/CDGH format back to [u32; 8]
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse2", enable = "ssse3", enable = "sse4.1")]
#[inline]
unsafe fn store_state(abef: __m128i, cdgh: __m128i, state: &mut [u32; 8]) {
    let feba = _mm_shuffle_epi32(abef, 0x1B);
    let dchg = _mm_shuffle_epi32(cdgh, 0xB1);
    let dcba = _mm_blend_epi16(feba, dchg, 0xF0);
    let hgef = _mm_alignr_epi8(dchg, feba, 8);

    let state_ptr = state.as_mut_ptr() as *mut __m128i;
    _mm_storeu_si128(state_ptr.add(0), dcba);
    _mm_storeu_si128(state_ptr.add(1), hgef);
}

/// Fused double SHA-256 with midstate - KEEPS STATE IN XMM REGISTERS
///
/// This is the KEY optimization: both transforms use XMM registers throughout.
/// First hash result is extracted in-register without going through memory.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sha", enable = "sse2", enable = "ssse3", enable = "sse4.1")]
unsafe fn sha256d_fused(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    // ========== FIRST HASH (midstate + tail + nonce + padding) ==========

    // Build block 1 for an 80-byte message:
    // [0..12]:   tail[0..12] (header bytes 64-75)
    // [12..16]:  nonce (header bytes 76-79)
    // [16]:      0x80 padding (byte 80)
    // [62..64]:  0x0280 length (640 bits = 80 bytes)
    let mut block1 = [0u8; 64];
    block1[..12].copy_from_slice(&tail[..12]);
    block1[12..16].copy_from_slice(&nonce.to_le_bytes());
    block1[16] = 0x80;
    block1[62] = 0x02;
    block1[63] = 0x80;

    // Load midstate into XMM
    let (abef, cdgh) = load_state(&midstate.state);

    // Transform block 1 - state stays in XMM registers
    let (abef, cdgh) = transform_block(abef, cdgh, &block1);

    // Extract first hash (state -> bytes)
    let mut state1 = [0u32; 8];
    store_state(abef, cdgh, &mut state1);
    let first_hash = state_to_bytes(&state1);

    // ========== SECOND HASH (H_INIT + first_hash + padding) ==========

    // Build block 2
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01;
    block2[63] = 0x00;

    // Load H_INIT
    let (abef, cdgh) = load_state(&H_INIT);

    // Transform block 2
    let (abef, cdgh) = transform_block(abef, cdgh, &block2);

    // Extract final hash
    let mut final_state = [0u32; 8];
    store_state(abef, cdgh, &mut final_state);
    state_to_bytes(&final_state)
}

/// Public API - Double SHA-256 with midstate (v2 optimized)
///
/// Uses cpufeatures for one-time CPU detection.
/// NO #[target_feature] on this function, allowing LLVM to inline from callers.
#[inline]
pub fn sha256d_midstate_v2(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        // One-time CPU check using cpufeatures (static boolean, very fast)
        if shani_cpuid::get() {
            return unsafe { sha256d_fused(midstate, tail, nonce) };
        }
    }
    // Fallback to generic
    super::generic::sha256d_midstate(midstate, tail, nonce)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::sha256d_midstate_generic;

    #[test]
    fn test_v2_matches_generic() {
        if !shani_cpuid::get() {
            println!("SHA-NI not available, skipping v2 test");
            return;
        }

        let test_cases = vec![
            ([0x00u8; 64], [0x00u8; 16], 0u32),
            ([0xFFu8; 64], [0xFFu8; 16], 0u32),
            ([0xABu8; 64], [0x12u8; 16], 0xDEADBEEFu32),
            ([0x42u8; 64], [0x33u8; 16], 12345u32),
        ];

        for (i, (header, tail, nonce)) in test_cases.iter().enumerate() {
            let midstate = Midstate::from_header_prefix(header);
            let generic_hash = sha256d_midstate_generic(&midstate, tail, *nonce);
            let v2_hash = sha256d_midstate_v2(&midstate, tail, *nonce);

            assert_eq!(
                generic_hash, v2_hash,
                "Test case {}: v2 hash mismatch.\nGeneric: {}\nV2:      {}",
                i,
                hex::encode(generic_hash),
                hex::encode(v2_hash)
            );
        }
    }
}
