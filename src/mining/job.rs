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
    /// Full coinbase transaction (for solo block submission)
    pub coinbase_tx: Option<Vec<u8>>,
    /// Other transactions in the block (for solo block submission)
    pub transactions: Option<Vec<Vec<u8>>>,
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
            coinbase_tx: None,
            transactions: None,
        }
    }

    /// Build a complete block for submission (header + transactions)
    /// Returns hex-encoded block data suitable for submitblock RPC
    pub fn build_block(&self, nonce: u32) -> Option<String> {
        let coinbase = self.coinbase_tx.as_ref()?;
        let txs = self.transactions.as_ref()?;

        let mut block_data = Vec::new();

        // Block header (80 bytes)
        let header = self.build_header(&[], &[], nonce);
        block_data.extend_from_slice(&header);

        // Transaction count (varint)
        let tx_count = 1 + txs.len(); // coinbase + other txs
        encode_varint(tx_count as u64, &mut block_data);

        // Coinbase transaction
        block_data.extend_from_slice(coinbase);

        // Other transactions
        for tx in txs {
            block_data.extend_from_slice(tx);
        }

        Some(hex::encode(block_data))
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
        hasher.update(first_hash);
        let mut merkle_root: [u8; 32] = hasher.finalize().into();

        // Apply merkle branches
        for branch in &self.merkle_branches {
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(&merkle_root);
            combined[32..64].copy_from_slice(branch);

            let mut hasher = Sha256::new();
            hasher.update(combined);
            let first = hasher.finalize();

            let mut hasher = Sha256::new();
            hasher.update(first);
            merkle_root = hasher.finalize().into();
        }

        merkle_root
    }

    /// Check if a hash meets the pool target
    pub fn meets_target(&self, hash: &[u8; 32]) -> bool {
        hash_meets_target(hash, &self.target)
    }

    /// Check if a hash meets the network target (for block)
    pub fn meets_network_target(&self, hash: &[u8; 32]) -> bool {
        if let Some(ref target) = self.network_target {
            hash_meets_target(hash, target)
        } else {
            false
        }
    }
}

/// Compare hash to target (hash must be <= target)
/// Returns true if hash meets (is less than or equal to) target
///
/// Used for determining if a hash meets the pool or network difficulty.
/// Comparison is done from most significant byte (big-endian).
#[inline]
pub fn hash_meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
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

/// Encode a number as a Bitcoin varint
fn encode_varint(value: u64, out: &mut Vec<u8>) {
    if value < 0xfd {
        out.push(value as u8);
    } else if value <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(value as u16).to_le_bytes());
    } else if value <= 0xffffffff {
        out.push(0xfe);
        out.extend_from_slice(&(value as u32).to_le_bytes());
    } else {
        out.push(0xff);
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Convert pool difficulty to target bytes (big-endian)
///
/// Pool diff1 target = 0x00000000FFFF0000...0000 (32 bytes)
/// This is the standard stratum pool difficulty base.
/// Target = diff1 / difficulty
///
/// For difficulty d:
/// - d = 1: target = 0x00000000FFFF0000...
/// - d = 2: target = 0x000000007FFF8000...
/// - d = 0.5: target = 0x00000001FFFE0000...
pub fn difficulty_to_target(difficulty: f64) -> [u8; 32] {
    if difficulty <= 0.0 {
        return [0xff; 32];
    }

    // diff1 value: 0xFFFF shifted left by 208 bits (26 bytes)
    // As a float: 0xFFFF * 2^208 ≈ 2.695953529e67
    // We calculate: target = diff1 / difficulty = (0xFFFF * 2^208) / d

    let mut target = [0u8; 32];

    // Compute target as u64: (0xFFFF << 48) / difficulty
    // This gives us 8 bytes of precision starting at byte offset 4
    let target_scaled = 0xFFFF_0000_0000_0000_u64 as f64 / difficulty;

    if target_scaled >= (1_u128 << 96) as f64 {
        // Overflows our placement, fill with 0xFF from appropriate byte
        let overflow_bytes = ((target_scaled.log2() - 64.0) / 8.0).ceil() as usize;
        let start = 4_usize.saturating_sub(overflow_bytes);
        for byte in target.iter_mut().skip(start) {
            *byte = 0xff;
        }
        return target;
    }

    if target_scaled < 1.0 {
        return [0x00; 32];
    }

    // Place the value at bytes 4-11 (8 bytes)
    let target_u64 = target_scaled as u64;

    // Big-endian placement
    target[4] = ((target_u64 >> 56) & 0xff) as u8;
    target[5] = ((target_u64 >> 48) & 0xff) as u8;
    target[6] = ((target_u64 >> 40) & 0xff) as u8;
    target[7] = ((target_u64 >> 32) & 0xff) as u8;
    target[8] = ((target_u64 >> 24) & 0xff) as u8;
    target[9] = ((target_u64 >> 16) & 0xff) as u8;
    target[10] = ((target_u64 >> 8) & 0xff) as u8;
    target[11] = (target_u64 & 0xff) as u8;

    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_meets_target() {
        let target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let good_hash = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let bad_hash = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

        assert!(hash_meets_target(&good_hash, &target));
        assert!(!hash_meets_target(&bad_hash, &target));
    }

    #[test]
    fn test_hash_meets_target_equal() {
        let target = [0x00, 0x00, 0x00, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // Equal hash should pass
        assert!(hash_meets_target(&target, &target));
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

    #[test]
    fn test_encode_varint_small() {
        let mut out = Vec::new();
        super::encode_varint(0, &mut out);
        assert_eq!(out, vec![0x00]);

        out.clear();
        super::encode_varint(100, &mut out);
        assert_eq!(out, vec![100]);

        out.clear();
        super::encode_varint(252, &mut out);
        assert_eq!(out, vec![252]);
    }

    #[test]
    fn test_encode_varint_two_bytes() {
        let mut out = Vec::new();
        super::encode_varint(253, &mut out);
        assert_eq!(out, vec![0xfd, 253, 0]);

        out.clear();
        super::encode_varint(0xffff, &mut out);
        assert_eq!(out, vec![0xfd, 0xff, 0xff]);
    }

    #[test]
    fn test_encode_varint_four_bytes() {
        let mut out = Vec::new();
        super::encode_varint(0x10000, &mut out);
        assert_eq!(out, vec![0xfe, 0x00, 0x00, 0x01, 0x00]);

        out.clear();
        super::encode_varint(0xffffffff, &mut out);
        assert_eq!(out, vec![0xfe, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn test_build_block_missing_data() {
        let job = Job::new("test".to_string());
        // Without coinbase_tx and transactions, should return None
        assert!(job.build_block(0).is_none());
    }

    #[test]
    fn test_build_block_with_data() {
        let mut job = Job::new("test".to_string());
        job.version = 0x20000000;
        job.prev_hash = [0x11; 32];
        job.nbits = 0x1d00ffff;
        job.ntime = 0x12345678;
        job.merkle_root_override = Some([0x22; 32]);
        job.coinbase_tx = Some(vec![0x01, 0x00, 0x00, 0x00]); // Minimal tx
        job.transactions = Some(vec![]); // No other transactions

        let block_hex = job.build_block(0xDEADBEEF);
        assert!(block_hex.is_some());

        let hex = block_hex.unwrap();
        // Should be at least header (80 bytes * 2 hex chars) + varint (2) + coinbase
        assert!(hex.len() >= 160 + 2 + 8);

        // Decode and verify structure
        let bytes = hex::decode(&hex).unwrap();

        // Check header (80 bytes)
        assert!(bytes.len() >= 80);

        // Check version
        assert_eq!(&bytes[0..4], &0x20000000u32.to_le_bytes());

        // Check nonce at offset 76
        assert_eq!(&bytes[76..80], &0xDEADBEEFu32.to_le_bytes());

        // Check varint tx count (should be 1 for just coinbase)
        assert_eq!(bytes[80], 1);
    }

    #[test]
    fn test_build_block_with_transactions() {
        let mut job = Job::new("test".to_string());
        job.version = 0x20000000;
        job.prev_hash = [0x11; 32];
        job.nbits = 0x1d00ffff;
        job.ntime = 0x12345678;
        job.merkle_root_override = Some([0x22; 32]);
        job.coinbase_tx = Some(vec![0x01, 0x00, 0x00, 0x00]); // Minimal coinbase
        job.transactions = Some(vec![
            vec![0x02, 0x00, 0x00, 0x00], // tx1
            vec![0x03, 0x00, 0x00, 0x00], // tx2
        ]);

        let block_hex = job.build_block(0x12345678);
        assert!(block_hex.is_some());

        let bytes = hex::decode(&block_hex.unwrap()).unwrap();

        // Check tx count (should be 3: coinbase + 2)
        assert_eq!(bytes[80], 3);

        // Check coinbase follows
        assert_eq!(&bytes[81..85], &[0x01, 0x00, 0x00, 0x00]);

        // Check tx1 follows coinbase
        assert_eq!(&bytes[85..89], &[0x02, 0x00, 0x00, 0x00]);

        // Check tx2 follows tx1
        assert_eq!(&bytes[89..93], &[0x03, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_build_block_deterministic() {
        let mut job = Job::new("test".to_string());
        job.version = 0x20000000;
        job.merkle_root_override = Some([0x33; 32]);
        job.coinbase_tx = Some(vec![0xAA, 0xBB, 0xCC]);
        job.transactions = Some(vec![]);

        let block1 = job.build_block(0x11111111);
        let block2 = job.build_block(0x11111111);

        assert_eq!(block1, block2, "Same nonce should produce same block");

        let block3 = job.build_block(0x22222222);
        assert_ne!(block1, block3, "Different nonce should produce different block");
    }
}
