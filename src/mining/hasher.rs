use sha2::{Sha256, Digest};
use super::sha256_simd::{Midstate, sha256d_midstate_generic, mine_with_midstate};

/// SHA256 double hasher for Bitcoin block headers
pub struct Hasher {
    // Cached mid-state for header prefix (first 64 bytes)
    midstate: Option<Midstate>,
    midstate_header_prefix: [u8; 64],
}

impl Hasher {
    pub fn new() -> Self {
        Self {
            midstate: None,
            midstate_header_prefix: [0u8; 64],
        }
    }

    /// Compute double SHA256 hash of a block header
    #[inline]
    pub fn hash_header(&self, header: &[u8; 80]) -> [u8; 32] {
        // First SHA256
        let mut hasher = Sha256::new();
        hasher.update(header);
        let first_hash = hasher.finalize();

        // Second SHA256
        let mut hasher = Sha256::new();
        hasher.update(first_hash);
        hasher.finalize().into()
    }

    /// Prepare midstate for optimized hashing when only nonce changes
    /// The midstate is the SHA256 state after processing the first 64 bytes
    pub fn prepare_midstate(&mut self, header_prefix: &[u8; 64]) {
        if self.midstate_header_prefix != *header_prefix {
            self.midstate_header_prefix = *header_prefix;
            // Compute actual midstate using SIMD-optimized implementation
            self.midstate = Some(Midstate::from_header_prefix(header_prefix));
        }
    }

    /// Hash header with midstate optimization (only last 16 bytes vary)
    /// This is significantly faster when mining as only the nonce changes
    #[inline]
    pub fn hash_with_midstate(&self, header: &[u8; 80]) -> [u8; 32] {
        if let Some(ref midstate) = self.midstate {
            // Extract tail (bytes 64-76) and nonce (bytes 76-80)
            let mut tail = [0u8; 16];
            tail[..12].copy_from_slice(&header[64..76]);
            // Nonce is at bytes 76-80, but sha256d_midstate_generic takes it separately
            let nonce = u32::from_le_bytes([header[76], header[77], header[78], header[79]]);
            sha256d_midstate_generic(midstate, &tail, nonce)
        } else {
            // Fall back to full hash if midstate not prepared
            self.hash_header(header)
        }
    }

    /// Batch hash multiple nonces efficiently using SIMD-optimized mining
    ///
    /// This uses the midstate optimization and SIMD (AVX2/SHA-NI) for maximum performance.
    /// The header_prefix contains 76 bytes: first 64 bytes for midstate, bytes 64-76 for tail.
    ///
    /// The midstate is cached and only recomputed when the first 64 bytes change.
    pub fn hash_nonce_range(
        &mut self,
        header_prefix: &[u8; 76],
        start_nonce: u32,
        count: u32,
        target: &[u8; 32],
    ) -> Option<(u32, [u8; 32])> {
        // Check if we need to recompute midstate (first 64 bytes changed)
        let prefix_64: &[u8; 64] = header_prefix[..64].try_into().unwrap();
        if self.midstate.is_none() || self.midstate_header_prefix != *prefix_64 {
            self.midstate_header_prefix = *prefix_64;
            self.midstate = Some(Midstate::from_header_prefix(prefix_64));
        }

        // Extract tail (bytes 64-76, padded to 16 bytes - nonce goes in last 4)
        let mut tail = [0u8; 16];
        tail[..12].copy_from_slice(&header_prefix[64..76]);
        // Note: tail[12..16] will be filled with nonce by mine_with_midstate

        // Use SIMD-optimized mining function (AVX2 8-way or SHA-NI)
        mine_with_midstate(self.midstate.as_ref().unwrap(), &tail, start_nonce, count, target)
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Format hash as hex string (reversed for display)
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    let mut reversed = *hash;
    reversed.reverse();
    hex::encode(reversed)
}

/// Parse hex string to hash bytes
pub fn hex_to_hash(hex_str: &str) -> Result<[u8; 32], hex::FromHexError> {
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(hex_str, &mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mining::job::hash_meets_target;

    #[test]
    fn test_double_sha256() {
        let hasher = Hasher::new();
        // Test with known Bitcoin genesis block header
        let header = [0u8; 80]; // Simplified test
        let hash = hasher.hash_header(&header);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_hash_to_hex() {
        let hash = [0u8; 32];
        let hex = hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c == '0'));
    }

    #[test]
    fn test_midstate_optimization() {
        let mut hasher = Hasher::new();
        let header = [0u8; 80];

        // Get hash via full computation
        let hash_full = hasher.hash_header(&header);

        // Prepare midstate from first 64 bytes
        let mut prefix = [0u8; 64];
        prefix.copy_from_slice(&header[..64]);
        hasher.prepare_midstate(&prefix);

        // Get hash via midstate optimization
        let hash_midstate = hasher.hash_with_midstate(&header);

        // Both methods should produce the same result
        assert_eq!(hash_full, hash_midstate, "Midstate hash should match full hash");
    }

    #[test]
    fn test_midstate_with_nonce_variation() {
        let mut hasher = Hasher::new();
        let mut header1 = [0u8; 80];
        let mut header2 = [0u8; 80];

        // Set different nonces
        header1[76..80].copy_from_slice(&1u32.to_le_bytes());
        header2[76..80].copy_from_slice(&2u32.to_le_bytes());

        // Prepare midstate (same for both since nonce is in last 16 bytes)
        let mut prefix = [0u8; 64];
        prefix.copy_from_slice(&header1[..64]);
        hasher.prepare_midstate(&prefix);

        // Hash both with midstate
        let hash1_full = hasher.hash_header(&header1);
        let hash1_mid = hasher.hash_with_midstate(&header1);
        let hash2_full = hasher.hash_header(&header2);
        let hash2_mid = hasher.hash_with_midstate(&header2);

        // Verify midstate matches full for both
        assert_eq!(hash1_full, hash1_mid);
        assert_eq!(hash2_full, hash2_mid);

        // Verify different nonces produce different hashes
        assert_ne!(hash1_full, hash2_full);
    }

    #[test]
    fn test_hash_meets_target() {
        let easy_target = [0xffu8; 32]; // All 0xff = very easy
        let hard_target = [0x00u8; 32]; // All 0x00 = impossible

        let hash = [0x80u8; 32]; // Middle value

        assert!(hash_meets_target(&hash, &easy_target));
        assert!(!hash_meets_target(&hash, &hard_target));
    }

    #[test]
    fn test_hash_meets_target_equal() {
        let target = [0x11u8; 32];
        let hash = [0x11u8; 32];
        // Equal should pass
        assert!(hash_meets_target(&hash, &target));
    }

    #[test]
    fn test_hash_meets_target_boundary() {
        // Test boundary condition - first byte differs
        let target = [0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff,
                      0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                      0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                      0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];

        // Hash with leading zeros passes
        let good_hash = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(hash_meets_target(&good_hash, &target));

        // Hash with higher value in 4th byte fails
        let bad_hash = [0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(!hash_meets_target(&bad_hash, &target));
    }

    #[test]
    fn test_hex_to_hash_valid() {
        let hex_str = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = hex_to_hash(hex_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0u8; 32]);
    }

    #[test]
    fn test_hex_to_hash_nonzero() {
        let hex_str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
        let result = hex_to_hash(hex_str);
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert_eq!(hash[0], 0x00);
        assert_eq!(hash[5], 0x19);
    }

    #[test]
    fn test_hex_to_hash_invalid_length() {
        let hex_str = "00000000"; // Too short
        let result = hex_to_hash(hex_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_hex_to_hash_invalid_chars() {
        let hex_str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
        let result = hex_to_hash(hex_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_to_hex_reverses_correctly() {
        let mut hash = [0u8; 32];
        hash[0] = 0xab;
        hash[31] = 0xcd;

        let hex = hash_to_hex(&hash);
        // hex should be reversed, so first chars are cd, last are ab
        assert!(hex.starts_with("cd"));
        assert!(hex.ends_with("ab"));
    }

    #[test]
    fn test_hash_nonce_range_no_match() {
        let mut hasher = Hasher::new();
        let mut header_prefix = [0u8; 76];
        // Set some data
        header_prefix[0] = 0x01;

        // Very hard target - basically impossible to hit
        let hard_target = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                           0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                           0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                           0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let result = hasher.hash_nonce_range(&header_prefix, 0, 100, &hard_target);
        assert!(result.is_none());
    }

    #[test]
    fn test_hash_nonce_range_easy_target() {
        let mut hasher = Hasher::new();
        let header_prefix = [0u8; 76];

        // Very easy target - everything passes
        let easy_target = [0xff; 32];

        let result = hasher.hash_nonce_range(&header_prefix, 0, 10, &easy_target);
        assert!(result.is_some());
        let (nonce, hash) = result.unwrap();
        assert_eq!(nonce, 0); // First nonce should match
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_hasher_default() {
        let hasher = Hasher::default();
        let header = [0u8; 80];
        let hash = hasher.hash_header(&header);
        assert_eq!(hash.len(), 32);
    }

    /// Performance regression test to ensure hash_nonce_range uses midstate optimization.
    /// This test catches accidental fallback to slow non-midstate SHA256 implementation.
    ///
    /// Current implementation uses sha256d_midstate_generic which is faster than sha2 crate
    /// because it skips recomputing the first 64 bytes (midstate optimization).
    ///
    /// NOTE: This test must be run in release mode for accurate results:
    ///   cargo test --release test_hash_nonce_range_uses_midstate_optimization
    #[test]
    #[ignore] // Run with --ignored flag, requires release mode for accurate results
    fn test_hash_nonce_range_uses_midstate_optimization() {
        use std::time::Instant;

        let mut hasher = Hasher::new();
        let header_prefix = [0u8; 76];

        // Very hard target - we don't want to find a match, just measure speed
        let hard_target = [0x00u8; 32];

        // Warm up (more iterations for better cache warming)
        for _ in 0..10 {
            let _ = hasher.hash_nonce_range(&header_prefix, 0, 10000, &hard_target);
        }

        // Measure time for a significant batch
        let batch_size: u32 = 500_000;
        let iterations = 5;
        let mut total_elapsed = std::time::Duration::ZERO;

        for i in 0..iterations {
            let start = Instant::now();
            let _ = hasher.hash_nonce_range(&header_prefix, i * batch_size, batch_size, &hard_target);
            total_elapsed += start.elapsed();
        }

        let total_hashes = (batch_size as u64) * (iterations as u64);
        let hashes_per_sec = (total_hashes as f64) / total_elapsed.as_secs_f64();

        // Midstate-optimized path should achieve >1 MH/s in release mode
        // The slow sha2 crate path without midstate would be around 0.5 MH/s
        // We set threshold at 1 MH/s to catch regression to non-midstate path
        let min_hashrate = 1_000_000.0; // 1 MH/s minimum

        eprintln!(
            "hash_nonce_range performance: {:.2} MH/s ({} hashes in {:.2}s)",
            hashes_per_sec / 1_000_000.0,
            total_hashes,
            total_elapsed.as_secs_f64()
        );

        assert!(
            hashes_per_sec > min_hashrate,
            "hash_nonce_range performance regression detected! \
             Got {:.2} MH/s, expected >{:.2} MH/s. \
             This likely means midstate optimization is not being used. \
             Check that hash_nonce_range calls mine_with_midstate instead of hash_header.",
            hashes_per_sec / 1_000_000.0,
            min_hashrate / 1_000_000.0
        );
    }

    /// Quick sanity test to verify hash_nonce_range uses mine_with_midstate path.
    /// This doesn't measure performance but verifies correct output matching.
    #[test]
    fn test_hash_nonce_range_correctness() {
        use crate::mining::sha256_simd::{Midstate, sha256d_midstate_generic};

        let mut hasher = Hasher::new();
        let header_prefix = [0xABu8; 76];

        // Use easy target so we get a result
        let easy_target = [0xFF; 32];

        // Test with hash_nonce_range
        let result = hasher.hash_nonce_range(&header_prefix, 0, 10, &easy_target);
        assert!(result.is_some(), "Should find match with easy target");

        let (nonce, hash) = result.unwrap();

        // Verify by computing the same hash directly via sha256d_midstate_generic
        let mut prefix_64 = [0u8; 64];
        prefix_64.copy_from_slice(&header_prefix[..64]);
        let midstate = Midstate::from_header_prefix(&prefix_64);

        let mut tail = [0u8; 16];
        tail[..12].copy_from_slice(&header_prefix[64..76]);

        let expected_hash = sha256d_midstate_generic(&midstate, &tail, nonce);

        assert_eq!(
            hash, expected_hash,
            "hash_nonce_range should use mine_with_midstate internally. \
             If hashes don't match, the implementation may have regressed."
        );
    }

    #[test]
    fn test_hash_with_midstate_no_midstate() {
        let hasher = Hasher::new();
        let header = [0u8; 80];

        // Without preparing midstate, should fall back to full hash
        let hash_full = hasher.hash_header(&header);
        let hash_midstate = hasher.hash_with_midstate(&header);

        assert_eq!(hash_full, hash_midstate);
    }

    #[test]
    fn test_prepare_midstate_same_prefix() {
        let mut hasher = Hasher::new();
        let prefix = [0xab; 64];

        // Prepare twice with same prefix - should not recompute
        hasher.prepare_midstate(&prefix);
        let hash1 = hasher.hash_with_midstate(&[0xab; 80]);

        hasher.prepare_midstate(&prefix);
        let hash2 = hasher.hash_with_midstate(&[0xab; 80]);

        assert_eq!(hash1, hash2);
    }
}
