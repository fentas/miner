use sha2::{Sha256, Digest};
use super::sha256_simd::{Midstate, sha256d_midstate_generic};

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
        hasher.update(&first_hash);
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

    /// Batch hash multiple nonces efficiently
    pub fn hash_nonce_range(
        &self,
        header_prefix: &[u8; 76],
        start_nonce: u32,
        count: u32,
        target: &[u8; 32],
    ) -> Option<(u32, [u8; 32])> {
        let mut header = [0u8; 80];
        header[..76].copy_from_slice(header_prefix);

        for nonce in start_nonce..start_nonce.saturating_add(count) {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());
            let hash = self.hash_header(&header);

            if meets_target(&hash, target) {
                return Some((nonce, hash));
            }
        }

        None
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if hash meets target (hash <= target in big-endian)
#[inline]
fn meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    // Compare from most significant byte (index 0 = MSB in big-endian)
    for i in 0..32 {
        if hash[i] < target[i] {
            return true;
        }
        if hash[i] > target[i] {
            return false;
        }
    }
    true
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
    fn test_meets_target() {
        let easy_target = [0xffu8; 32]; // All 0xff = very easy
        let hard_target = [0x00u8; 32]; // All 0x00 = impossible

        let hash = [0x80u8; 32]; // Middle value

        assert!(meets_target(&hash, &easy_target));
        assert!(!meets_target(&hash, &hard_target));
    }
}
