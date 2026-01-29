//! Integration tests for mining functionality

#[test]
fn test_sha256_double_hash_property() {
    // SHA256d(x) should always produce 32 bytes
    use sha2::{Sha256, Digest};

    let test_inputs = [
        b"hello".to_vec(),
        b"world".to_vec(),
        vec![0u8; 80], // Empty block header
        vec![0xff; 80], // All ones block header
    ];

    for input in test_inputs {
        let first = Sha256::digest(&input);
        let second = Sha256::digest(first);
        assert_eq!(second.len(), 32);
    }
}

#[test]
fn test_nonce_increment_doesnt_overflow() {
    // Test that nonce handling works correctly near boundaries
    let max_nonce: u32 = 0xFFFFFFFF;
    let wrapped = max_nonce.wrapping_add(1);
    assert_eq!(wrapped, 0);
}

#[test]
fn test_target_comparison() {
    // Lower target = harder difficulty
    let easy_target = [0xFF; 32];
    let hard_target = [0x00, 0x00, 0x00, 0x1F, 0xFF, 0xFF, 0xFF, 0xFF,
                       0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                       0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                       0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

    // Any hash should be <= easy_target
    let test_hash = [0x00; 32];
    assert!(test_hash <= easy_target);
    assert!(test_hash <= hard_target);

    // High hash should fail hard target
    let high_hash = [0xFF; 32];
    assert!(high_hash <= easy_target);
    assert!(high_hash > hard_target);
}

#[test]
fn test_extranonce_encoding() {
    // Test hex encoding for extranonce values
    let extranonce: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
    let encoded = hex::encode(extranonce);
    assert_eq!(encoded, "01020304");

    let decoded = hex::decode(&encoded).unwrap();
    assert_eq!(decoded, extranonce);
}

#[test]
fn test_ntime_encoding() {
    // Test ntime (unix timestamp) encoding
    let timestamp: u32 = 1706400000; // Some timestamp
    let bytes = timestamp.to_be_bytes();
    let encoded = hex::encode(bytes);

    let decoded = hex::decode(&encoded).unwrap();
    let recovered = u32::from_be_bytes(decoded.try_into().unwrap());
    assert_eq!(recovered, timestamp);
}

#[test]
fn test_block_version_encoding() {
    // Bitcoin block versions are typically small positive integers
    let version: i32 = 0x20000000; // Version with BIP9 signaling
    let bytes = version.to_le_bytes();

    // Should be 4 bytes
    assert_eq!(bytes.len(), 4);

    // Recovery
    let recovered = i32::from_le_bytes(bytes);
    assert_eq!(recovered, version);
}

#[test]
fn test_merkle_root_format() {
    // Merkle roots are 32-byte hashes in little-endian display format
    let merkle_root_hex = "0000000000000000000000000000000000000000000000000000000000000000";
    let decoded = hex::decode(merkle_root_hex).unwrap();
    assert_eq!(decoded.len(), 32);
}

#[test]
fn test_hashrate_units() {
    // Test hashrate unit conversions
    let h_per_sec: f64 = 1_000_000_000.0; // 1 GH/s
    let kh: f64 = h_per_sec / 1_000.0;
    let mh: f64 = h_per_sec / 1_000_000.0;
    let gh: f64 = h_per_sec / 1_000_000_000.0;

    assert!((kh - 1_000_000.0).abs() < 0.001);
    assert!((mh - 1_000.0).abs() < 0.001);
    assert!((gh - 1.0).abs() < 0.001);
}

#[test]
fn test_difficulty_to_target_conversion() {
    // Network difficulty 1 corresponds to target with leading zeros
    // Higher difficulty = more leading zeros required

    // Difficulty 1 target (pool difficulty, not network)
    let diff_1_target: [u8; 32] = [
        0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    // First 4 bytes should be zero for diff 1
    assert_eq!(&diff_1_target[..4], &[0x00, 0x00, 0x00, 0x00]);
}

// ============================================================================
// Integration tests for Job and solo mining functionality
// ============================================================================

#[test]
fn test_job_with_merkle_root_override() {
    use btcminer::mining::Job;

    let mut job = Job::new("test-solo-123".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;

    // Set prev_hash
    job.prev_hash = [0x11; 32];

    // Set a pre-computed merkle root (simulating solo mining)
    let merkle_root = [0xAB; 32];
    job.merkle_root_override = Some(merkle_root);

    // Build header should use the override
    let header = job.build_header(&[], &[], 0);

    // Verify merkle root is in header at offset 36-68
    assert_eq!(&header[36..68], &merkle_root);
}

#[test]
fn test_job_without_merkle_root_override_computes_root() {
    use btcminer::mining::Job;

    let mut job = Job::new("test-pool".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0x22; 32];
    job.coinbase1 = vec![0x01, 0x02, 0x03];
    job.coinbase2 = vec![0x04, 0x05, 0x06];

    // No merkle_root_override - should compute from coinbase
    let header = job.build_header(&[0xAA], &[0xBB], 0);

    // Header should be 80 bytes
    assert_eq!(header.len(), 80);

    // Merkle root should be computed (not all zeros or specific pattern)
    let merkle_in_header = &header[36..68];
    assert_ne!(merkle_in_header, &[0u8; 32]);
}

#[test]
fn test_job_header_contains_all_fields() {
    use btcminer::mining::Job;

    let mut job = Job::new("full-test".to_string());
    job.version = 0x20004000; // Version with BIP signals
    job.ntime = 0x65A12345;
    job.nbits = 0x170c69e6;
    job.prev_hash = [0x33; 32];
    job.merkle_root_override = Some([0x44; 32]);

    let nonce: u32 = 0xDEADBEEF;
    let header = job.build_header(&[], &[], nonce);

    // Check version (bytes 0-4, little-endian)
    assert_eq!(&header[0..4], &job.version.to_le_bytes());

    // Check prev_hash (bytes 4-36)
    assert_eq!(&header[4..36], &job.prev_hash);

    // Check merkle root (bytes 36-68)
    assert_eq!(&header[36..68], job.merkle_root_override.as_ref().unwrap());

    // Check ntime (bytes 68-72, little-endian)
    assert_eq!(&header[68..72], &job.ntime.to_le_bytes());

    // Check nbits (bytes 72-76, little-endian)
    assert_eq!(&header[72..76], &job.nbits.to_le_bytes());

    // Check nonce (bytes 76-80, little-endian)
    assert_eq!(&header[76..80], &nonce.to_le_bytes());
}

#[test]
fn test_job_network_target_for_solo_mining() {
    use btcminer::mining::Job;

    let mut job = Job::new("solo-block".to_string());

    // Initially no network target
    assert!(job.network_target.is_none());
    let hash = [0x00; 32];
    assert!(!job.meets_network_target(&hash));

    // Set network target (for solo mining)
    let network_target = [0x00, 0x00, 0x00, 0x1F, 0xFF, 0xFF, 0xFF, 0xFF,
                          0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                          0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                          0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    job.network_target = Some(network_target);

    // All-zero hash should meet any valid target
    assert!(job.meets_network_target(&hash));

    // High hash should not meet network target
    let high_hash = [0xFF; 32];
    assert!(!job.meets_network_target(&high_hash));
}

#[test]
fn test_difficulty_to_target_function() {
    use btcminer::mining::difficulty_to_target;

    // Difficulty 1 should give a specific target
    let target_1 = difficulty_to_target(1.0);
    assert_eq!(&target_1[0..4], &[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(target_1[4], 0xff);
    assert_eq!(target_1[5], 0xff);

    // Higher difficulty = lower target (more leading zeros)
    let target_high = difficulty_to_target(1000.0);
    // With difficulty 1000, target should be 1/1000 of diff-1 target
    // This means more bytes at the start should be zero or very small
    assert!(target_high < target_1, "Higher difficulty should produce lower target");
}

#[test]
fn test_job_clean_jobs_flag() {
    use btcminer::mining::Job;

    let mut job = Job::new("new-block".to_string());

    // Default should be false
    assert!(!job.clean_jobs);

    // Set it true (indicating workers should abandon current work)
    job.clean_jobs = true;
    assert!(job.clean_jobs);
}
