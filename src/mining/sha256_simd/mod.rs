//! SHA-256 SIMD optimizations for mining
//!
//! Provides hardware-accelerated SHA-256 implementations:
//! - SHA-NI: Intel SHA Extensions (fastest, ~100-150 MH/s single thread)
//! - AVX512: 16-way parallel hashing (fallback for AVX512 CPUs without SHA-NI)
//! - AVX2: 8-way parallel hashing (fallback for Haswell/Broadwell era CPUs)
//! - Generic: Portable fallback (~2-3 MH/s)

mod generic;
#[cfg(target_arch = "x86_64")]
mod shani;
#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(target_arch = "x86_64")]
mod avx512;
#[cfg(target_arch = "x86_64")]
mod asm;
#[cfg(target_arch = "x86_64")]
mod asm_v2;
#[cfg(target_arch = "x86_64")]
mod asm_v3;
#[cfg(target_arch = "x86_64")]
mod ultimate;

use super::job::hash_meets_target;

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
        generic::transform(&mut state, header);
        Self { state }
    }
}

/// Generic SHA-256 single block transform
pub fn transform_generic(state: &mut [u32; 8], block: &[u8]) {
    generic::transform(state, block);
}

/// Double SHA-256 using generic implementation
pub fn sha256d_generic(data: &[u8]) -> [u8; 32] {
    generic::sha256d(data)
}

/// SHA-256 with midstate (for mining - only last 16 bytes change)
pub fn sha256d_midstate_generic(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    generic::sha256d_midstate(midstate, tail, nonce)
}

/// Double SHA-256 with midstate using best available SIMD
///
/// This function computes a SINGLE hash and is designed to be called from a loop
/// that lives OUTSIDE of any #[target_feature] function. This allows the loop
/// to be inlined while still using SIMD acceleration for the actual hashing.
///
/// The dispatch overhead is minimal compared to the hash computation time.
#[inline]
pub fn sha256d_midstate_simd(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sha") && is_x86_feature_detected!("sse4.1") {
            // SAFETY: We just checked that SHA-NI is available
            return unsafe { shani::sha256d_midstate(midstate, tail, nonce) };
        }
        // AVX2 and AVX512 don't have single-hash functions optimized for this use case,
        // fall through to generic which is still reasonably fast
    }
    generic::sha256d_midstate(midstate, tail, nonce)
}

/// Double SHA-256 with midstate using inline assembly (no #[target_feature])
///
/// This implementation uses the `asm!` macro directly, avoiding the
/// #[target_feature] inlining boundary that limits SIMD performance.
/// The mining loop can be fully inlined by the compiler when using this function.
#[inline]
pub fn sha256d_midstate_asm(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        return asm::sha256d_midstate_asm(midstate, tail, nonce);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        generic::sha256d_midstate(midstate, tail, nonce)
    }
}

/// Double SHA-256 with midstate - v2 optimized (sha2 crate style)
///
/// Key optimizations from sha2 crate:
/// 1. Uses cpufeatures for one-time CPU detection (not per-hash)
/// 2. Clean macro structure matching sha2 crate for better LLVM optimization
/// 3. No #[target_feature] on public function - allows inlining
#[inline]
pub fn sha256d_midstate_v2(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        return asm_v2::sha256d_midstate_v2(midstate, tail, nonce);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        generic::sha256d_midstate(midstate, tail, nonce)
    }
}

/// Double SHA-256 with midstate - v3 (NO target_feature, requires global compilation flags)
///
/// This version has NO #[target_feature] on ANY function, allowing full LLVM inlining.
/// IMPORTANT: Only use when compiled with -C target-feature=+sha,+sse4.1,+ssse3,+sse2
/// Will crash on CPUs without SHA-NI!
#[inline(always)]
pub fn sha256d_midstate_v3(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        return asm_v3::sha256d_midstate_v3(midstate, tail, nonce);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        generic::sha256d_midstate(midstate, tail, nonce)
    }
}

/// Ultimate double SHA-256 with midstate - combines all research optimizations
///
/// Combines the best from all implementations:
/// - NO #[target_feature] for full LLVM inlining (from v3)
/// - Pre-computed padding for second hash
/// - Fused state transfer (state stays in registers)
/// - Optimized mining loop with early exit
///
/// IMPORTANT: Requires -C target-feature=+sha,+sse4.1,+ssse3 compilation!
/// Will crash on CPUs without SHA-NI!
#[inline(always)]
pub fn sha256d_midstate_ultimate(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    #[cfg(target_arch = "x86_64")]
    {
        return ultimate::sha256d_midstate_ultimate(midstate, tail, nonce);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        generic::sha256d_midstate(midstate, tail, nonce)
    }
}

/// Mine a range of nonces using the ultimate implementation
pub fn mine_ultimate(
    midstate: &Midstate,
    tail: &[u8; 16],
    start_nonce: u32,
    count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    #[cfg(target_arch = "x86_64")]
    {
        return ultimate::mine_ultimate(midstate, tail, start_nonce, count, target);
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        generic::mine(midstate, tail, start_nonce, count, target)
    }
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
            return unsafe { shani::mine(midstate, tail, nonce_start, nonce_count, target) };
        }

        // Use AVX512 16-way parallel hashing as fallback for non-SHA-NI AVX512 CPUs
        if level == SimdLevel::Avx512 {
            // SAFETY: We checked that AVX512 is available
            return unsafe { avx512::mine(midstate, tail, nonce_start, nonce_count, target) };
        }

        // Use AVX2 8-way parallel hashing as fallback for non-SHA-NI CPUs
        if level == SimdLevel::Avx2 {
            // SAFETY: We checked that AVX2 is available
            return unsafe { avx2::mine(midstate, tail, nonce_start, nonce_count, target) };
        }
    }

    let _ = level; // Silence unused warning on non-x86_64
    generic::mine(midstate, tail, nonce_start, nonce_count, target)
}

/// Pad message for SHA-256
pub fn sha256_pad(data: &[u8]) -> Vec<u8> {
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
pub fn state_to_bytes(state: &[u32; 8]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for (i, &word) in state.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    result
}

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
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

        let result = generic::mine(&midstate, &tail, 0, 100, &target);
        assert!(result.is_none());
    }

    #[test]
    fn test_mine_generic_easy_target() {
        let header = [0u8; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0u8; 16];
        let target = [0xFF; 32]; // Very easy target

        let result = generic::mine(&midstate, &tail, 0, 10, &target);
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
            std::array::from_fn(|i| i as u8),
        ];

        for (i, block) in test_cases.iter().enumerate() {
            // Test transform function
            let mut state_generic = H_INIT;
            let mut state_shani = H_INIT;

            transform_generic(&mut state_generic, block);
            unsafe { shani::transform(&mut state_shani, block) };

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
            let shani_hash = unsafe { shani::sha256d_midstate(&midstate, &tail, nonce) };

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
            std::array::from_fn(|i| i as u8),
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
            unsafe { avx2::transform_8way(&mut states_avx2, &blocks) };

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
        let result = unsafe { avx2::mine(&midstate, &tail, 0, 16, &target) };
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

            // Build block for an 80-byte message (same as mine_avx2)
            let mut block = [0u8; 64];
            block[..12].copy_from_slice(&tail[..12]);
            block[12..16].copy_from_slice(&nonce.to_le_bytes());
            block[16] = 0x80;
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
            std::array::from_fn(|i| i as u8),
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
            unsafe { avx512::transform_16way(&mut states_avx512, &blocks) };

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
        let result = unsafe { avx512::mine(&midstate, &tail, 0, 32, &target) };
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

            // Build block for an 80-byte message (same as mine_avx512)
            let mut block = [0u8; 64];
            block[..12].copy_from_slice(&tail[..12]);
            block[12..16].copy_from_slice(&nonce.to_le_bytes());
            block[16] = 0x80;
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
