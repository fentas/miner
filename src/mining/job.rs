/// A mining job received from pool or generated locally
#[derive(Debug, Clone)]
pub struct Job {
    /// Unique job identifier
    pub id: String,
    /// Previous block hash (32 bytes)
    pub prev_hash: [u8; 32],
    /// Coinbase part 1 (before extranonce)
    pub coinbase1: Vec<u8>,
    /// Coinbase part 2 (after extranonce)
    pub coinbase2: Vec<u8>,
    /// Merkle branches for block construction
    pub merkle_branches: Vec<[u8; 32]>,
    /// Block version
    pub version: u32,
    /// Difficulty target (nbits)
    pub nbits: u32,
    /// Block timestamp
    pub ntime: u32,
    /// Pool difficulty target
    pub target: [u8; 32],
    /// Network difficulty target (for solo mining)
    pub network_target: Option<[u8; 32]>,
    /// Clean jobs flag (abandon previous work)
    pub clean_jobs: bool,
    /// Pre-computed merkle root (for solo mining, bypasses coinbase computation)
    pub merkle_root_override: Option<[u8; 32]>,
}

impl Job {
    /// Create a new job
    pub fn new(id: String) -> Self {
        Self {
            id,
            prev_hash: [0u8; 32],
            coinbase1: Vec::new(),
            coinbase2: Vec::new(),
            merkle_branches: Vec::new(),
            version: 0x20000000,
            nbits: 0,
            ntime: 0,
            target: [0xff; 32],
            network_target: None,
            clean_jobs: false,
            merkle_root_override: None,
        }
    }

    /// Build the block header for a given nonce and extranonce
    pub fn build_header(&self, extranonce1: &[u8], extranonce2: &[u8], nonce: u32) -> [u8; 80] {
        let mut header = [0u8; 80];

        // Version (4 bytes, little-endian)
        header[0..4].copy_from_slice(&self.version.to_le_bytes());

        // Previous block hash (32 bytes)
        header[4..36].copy_from_slice(&self.prev_hash);

        // Merkle root (32 bytes) - use override if set (solo mining), otherwise compute
        let merkle_root = if let Some(ref root) = self.merkle_root_override {
            *root
        } else {
            self.compute_merkle_root(extranonce1, extranonce2)
        };
        header[36..68].copy_from_slice(&merkle_root);

        // Timestamp (4 bytes, little-endian)
        header[68..72].copy_from_slice(&self.ntime.to_le_bytes());

        // Bits/target (4 bytes, little-endian)
        header[72..76].copy_from_slice(&self.nbits.to_le_bytes());

        // Nonce (4 bytes, little-endian)
        header[76..80].copy_from_slice(&nonce.to_le_bytes());

        header
    }

    /// Compute the merkle root for given extranonces
    fn compute_merkle_root(&self, extranonce1: &[u8], extranonce2: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};

        // Build coinbase transaction
        let mut coinbase = Vec::with_capacity(
            self.coinbase1.len() + extranonce1.len() + extranonce2.len() + self.coinbase2.len()
        );
        coinbase.extend_from_slice(&self.coinbase1);
        coinbase.extend_from_slice(extranonce1);
        coinbase.extend_from_slice(extranonce2);
        coinbase.extend_from_slice(&self.coinbase2);

        // Double SHA256 of coinbase
        let mut hasher = Sha256::new();
        hasher.update(&coinbase);
        let first_hash = hasher.finalize();

        let mut hasher = Sha256::new();
        hasher.update(&first_hash);
        let mut merkle_root: [u8; 32] = hasher.finalize().into();

        // Apply merkle branches
        for branch in &self.merkle_branches {
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(&merkle_root);
            combined[32..64].copy_from_slice(branch);

            let mut hasher = Sha256::new();
            hasher.update(&combined);
            let first = hasher.finalize();

            let mut hasher = Sha256::new();
            hasher.update(&first);
            merkle_root = hasher.finalize().into();
        }

        merkle_root
    }

    /// Check if a hash meets the pool target
    pub fn meets_target(&self, hash: &[u8; 32]) -> bool {
        compare_hash_to_target(hash, &self.target)
    }

    /// Check if a hash meets the network target (for block)
    pub fn meets_network_target(&self, hash: &[u8; 32]) -> bool {
        if let Some(ref target) = self.network_target {
            compare_hash_to_target(hash, target)
        } else {
            false
        }
    }
}

/// Compare hash to target (hash must be <= target)
/// Returns true if hash meets (is less than or equal to) target
fn compare_hash_to_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    // Compare from most significant byte (index 0 = MSB in big-endian)
    for i in 0..32 {
        if hash[i] < target[i] {
            return true;
        }
        if hash[i] > target[i] {
            return false;
        }
    }
    true // Equal
}

/// Convert difficulty to target bytes
pub fn difficulty_to_target(difficulty: f64) -> [u8; 32] {
    // Bitcoin difficulty 1 target
    const DIFF1_TARGET: [u8; 32] = [
        0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    if difficulty <= 0.0 {
        return [0xff; 32];
    }

    // target = diff1_target / difficulty
    let mut target = [0u8; 32];
    let mut carry: u64 = 0;

    for i in 0..32 {
        let val = (DIFF1_TARGET[i] as f64 / difficulty) + carry as f64;
        target[i] = (val as u64 % 256) as u8;
        carry = val as u64 / 256;
    }

    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_hash_to_target() {
        let target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let good_hash = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let bad_hash = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        assert!(compare_hash_to_target(&good_hash, &target));
        assert!(!compare_hash_to_target(&bad_hash, &target));
    }

    #[test]
    fn test_compare_hash_to_target_equal() {
        let target = [0x00, 0x00, 0x00, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // Equal hash should pass
        assert!(compare_hash_to_target(&target, &target));
    }

    #[test]
    fn test_job_new() {
        let job = Job::new("test-job-1".to_string());
        assert_eq!(job.id, "test-job-1");
        assert_eq!(job.prev_hash, [0u8; 32]);
        assert!(job.coinbase1.is_empty());
        assert!(job.coinbase2.is_empty());
        assert!(job.merkle_branches.is_empty());
        assert_eq!(job.version, 0x20000000);
        assert_eq!(job.nbits, 0);
        assert_eq!(job.ntime, 0);
        assert_eq!(job.target, [0xff; 32]);
        assert!(job.network_target.is_none());
        assert!(!job.clean_jobs);
    }

    #[test]
    fn test_job_build_header() {
        let mut job = Job::new("test".to_string());
        job.version = 0x20000000;
        job.ntime = 0x12345678;
        job.nbits = 0xAABBCCDD;
        job.prev_hash = [0x11; 32];

        let header = job.build_header(&[0x01, 0x02], &[0x03, 0x04], 0xDEADBEEF);

        // Check version (little-endian)
        assert_eq!(&header[0..4], &0x20000000u32.to_le_bytes());

        // Check prev_hash
        assert_eq!(&header[4..36], &[0x11; 32]);

        // Check ntime (little-endian)
        assert_eq!(&header[68..72], &0x12345678u32.to_le_bytes());

        // Check nbits (little-endian)
        assert_eq!(&header[72..76], &0xAABBCCDDu32.to_le_bytes());

        // Check nonce (little-endian)
        assert_eq!(&header[76..80], &0xDEADBEEFu32.to_le_bytes());
    }

    #[test]
    fn test_job_meets_target() {
        let mut job = Job::new("test".to_string());
        // Easy target (all 0xFF)
        job.target = [0xff; 32];

        let any_hash = [0x12; 32];
        assert!(job.meets_target(&any_hash));

        // Hard target (leading zeros)
        job.target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let bad_hash = [0x01; 32];
        assert!(!job.meets_target(&bad_hash));
    }

    #[test]
    fn test_job_meets_network_target() {
        let mut job = Job::new("test".to_string());

        // No network target set
        let hash = [0x00; 32];
        assert!(!job.meets_network_target(&hash));

        // Set network target
        job.network_target = Some([0xff; 32]);
        assert!(job.meets_network_target(&hash));
    }

    #[test]
    fn test_difficulty_to_target_zero() {
        let target = difficulty_to_target(0.0);
        assert_eq!(target, [0xff; 32]);
    }

    #[test]
    fn test_difficulty_to_target_negative() {
        let target = difficulty_to_target(-1.0);
        assert_eq!(target, [0xff; 32]);
    }

    #[test]
    fn test_difficulty_to_target_one() {
        let target = difficulty_to_target(1.0);
        // Difficulty 1 should return the diff1 target
        assert_eq!(target[0], 0x00);
        assert_eq!(target[1], 0x00);
        assert_eq!(target[2], 0x00);
        assert_eq!(target[3], 0x00);
        assert_eq!(target[4], 0xff);
        assert_eq!(target[5], 0xff);
    }

    #[test]
    fn test_compute_merkle_root_no_branches() {
        let mut job = Job::new("test".to_string());
        job.coinbase1 = vec![0x01, 0x02, 0x03];
        job.coinbase2 = vec![0x04, 0x05, 0x06];

        let merkle_root = job.compute_merkle_root(&[0xAA], &[0xBB]);

        // Should be 32 bytes
        assert_eq!(merkle_root.len(), 32);

        // With same inputs, should get same output
        let merkle_root2 = job.compute_merkle_root(&[0xAA], &[0xBB]);
        assert_eq!(merkle_root, merkle_root2);

        // Different inputs should give different output
        let merkle_root3 = job.compute_merkle_root(&[0xCC], &[0xDD]);
        assert_ne!(merkle_root, merkle_root3);
    }

    #[test]
    fn test_compute_merkle_root_with_branches() {
        let mut job = Job::new("test".to_string());
        job.coinbase1 = vec![0x01];
        job.coinbase2 = vec![0x02];
        job.merkle_branches = vec![[0x11; 32], [0x22; 32]];

        let merkle_root = job.compute_merkle_root(&[0xAA], &[0xBB]);
        assert_eq!(merkle_root.len(), 32);
    }
}
