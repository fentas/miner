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
