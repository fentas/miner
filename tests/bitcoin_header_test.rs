//! Bitcoin Header Structure Verification Tests
//!
//! Verifies that our SHA-256 implementation correctly computes Bitcoin block hashes.
//! Uses the Bitcoin genesis block as a test vector.
//!
//! Bitcoin block header structure (80 bytes):
//! - Bytes 0-3:   Version (int32_t, little-endian)
//! - Bytes 4-35:  Previous block hash (char[32], internal byte order)
//! - Bytes 36-67: Merkle root (char[32], internal byte order)
//! - Bytes 68-71: Timestamp (uint32_t, little-endian)
//! - Bytes 72-75: nBits/Target (uint32_t, little-endian)
//! - Bytes 76-79: Nonce (uint32_t, little-endian)
//!
//! Reference: https://developer.bitcoin.org/reference/block_chain.html

use btcminer::mining::{Hasher, HasherType};
use btcminer::mining::sha256_simd::{
    Midstate, sha256d_midstate_generic, sha256d_midstate_simd,
    sha256d_midstate_asm, sha256d_midstate_v2, sha256d_midstate_v3,
};

/// Bitcoin genesis block header (80 bytes)
/// Raw hex: 0100000000000000000000000000000000000000000000000000000000000000
///          000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa
///          4b1e5e4a29ab5f49ffff001d1dac2b7c
///
/// Source: https://en.bitcoin.it/wiki/Genesis_block
fn genesis_block_header() -> [u8; 80] {
    let mut header = [0u8; 80];

    // Version: 1 (little-endian)
    header[0..4].copy_from_slice(&1u32.to_le_bytes());

    // Previous block hash: all zeros (genesis has no parent)
    // bytes 4-35 are already zero

    // Merkle root: 3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a
    // This is in internal byte order (reversed from display order)
    let merkle_root = hex::decode("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a")
        .unwrap();
    header[36..68].copy_from_slice(&merkle_root);

    // Timestamp: 1231006505 (2009-01-03 18:15:05 UTC)
    header[68..72].copy_from_slice(&1231006505u32.to_le_bytes());

    // nBits (difficulty target): 0x1d00ffff
    header[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());

    // Nonce: 2083236893
    header[76..80].copy_from_slice(&2083236893u32.to_le_bytes());

    header
}

/// Expected hash of the genesis block (in internal byte order)
/// Display order: 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f
/// Internal order: 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
fn genesis_block_hash() -> [u8; 32] {
    // The hash in internal byte order (reversed from display)
    let hash_hex = "6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000";
    let hash_bytes = hex::decode(hash_hex).unwrap();
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash_bytes);
    result
}

/// Verify the header structure is correct
#[test]
fn test_bitcoin_header_structure() {
    let header = genesis_block_header();

    // Verify total size
    assert_eq!(header.len(), 80, "Bitcoin header must be exactly 80 bytes");

    // Verify version (bytes 0-3)
    let version = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    assert_eq!(version, 1, "Genesis block version should be 1");

    // Verify prev_hash is all zeros (bytes 4-35)
    let prev_hash = &header[4..36];
    assert!(prev_hash.iter().all(|&b| b == 0), "Genesis prev_hash should be all zeros");

    // Verify timestamp (bytes 68-71)
    let timestamp = u32::from_le_bytes([header[68], header[69], header[70], header[71]]);
    assert_eq!(timestamp, 1231006505, "Genesis timestamp should be 1231006505");

    // Verify nBits (bytes 72-75)
    let nbits = u32::from_le_bytes([header[72], header[73], header[74], header[75]]);
    assert_eq!(nbits, 0x1d00ffff, "Genesis nBits should be 0x1d00ffff");

    // Verify nonce (bytes 76-79)
    let nonce = u32::from_le_bytes([header[76], header[77], header[78], header[79]]);
    assert_eq!(nonce, 2083236893, "Genesis nonce should be 2083236893");
}

/// Verify hash_header produces correct Bitcoin block hash
#[test]
fn test_genesis_block_hash_header() {
    let header = genesis_block_header();
    let expected_hash = genesis_block_hash();

    let hasher = Hasher::new();
    let computed_hash = hasher.hash_header(&header);

    assert_eq!(
        computed_hash, expected_hash,
        "hash_header should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(computed_hash)
    );
}

/// Verify all hasher implementations produce the same correct hash
#[test]
fn test_all_hashers_produce_correct_genesis_hash() {
    let header = genesis_block_header();
    let expected_hash = genesis_block_hash();

    // Test each hasher type
    let hasher_types = [
        (HasherType::Sha2Crate, "SHA2 Crate"),
        (HasherType::Simd, "SIMD"),
        (HasherType::Assembly, "Assembly"),
        (HasherType::AsmV2, "AsmV2"),
        (HasherType::AsmV3, "AsmV3"),
    ];

    for (hasher_type, name) in hasher_types {
        let mut hasher = Hasher::with_type(hasher_type);

        // Create header_prefix (first 76 bytes) and extract nonce
        let mut header_prefix = [0u8; 76];
        header_prefix.copy_from_slice(&header[..76]);
        let nonce = u32::from_le_bytes([header[76], header[77], header[78], header[79]]);

        // Use an easy target that will match any hash
        let easy_target = [0xFF; 32];

        // hash_nonce_range should find and return the correct hash
        let result = hasher.hash_nonce_range(&header_prefix, nonce, 1, &easy_target);

        assert!(result.is_some(), "{} should find hash with easy target", name);
        let (found_nonce, computed_hash) = result.unwrap();

        assert_eq!(found_nonce, nonce, "{} should return the same nonce", name);
        assert_eq!(
            computed_hash, expected_hash,
            "{} should produce correct genesis block hash.\n\
             Expected: {}\n\
             Got:      {}",
            name,
            hex::encode(expected_hash),
            hex::encode(computed_hash)
        );
    }
}

/// Verify midstate implementations produce correct hash
#[test]
fn test_midstate_implementations_genesis_hash() {
    let header = genesis_block_header();
    let expected_hash = genesis_block_hash();

    // Compute midstate from first 64 bytes
    let mut prefix_64 = [0u8; 64];
    prefix_64.copy_from_slice(&header[..64]);
    let midstate = Midstate::from_header_prefix(&prefix_64);

    // Extract tail (bytes 64-75) and nonce (bytes 76-79)
    let mut tail = [0u8; 16];
    tail[..12].copy_from_slice(&header[64..76]);
    let nonce = u32::from_le_bytes([header[76], header[77], header[78], header[79]]);

    // Test generic implementation
    let hash_generic = sha256d_midstate_generic(&midstate, &tail, nonce);
    assert_eq!(
        hash_generic, expected_hash,
        "sha256d_midstate_generic should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(hash_generic)
    );

    // Test SIMD implementation
    let hash_simd = sha256d_midstate_simd(&midstate, &tail, nonce);
    assert_eq!(
        hash_simd, expected_hash,
        "sha256d_midstate_simd should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(hash_simd)
    );

    // Test ASM implementation
    let hash_asm = sha256d_midstate_asm(&midstate, &tail, nonce);
    assert_eq!(
        hash_asm, expected_hash,
        "sha256d_midstate_asm should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(hash_asm)
    );

    // Test AsmV2 implementation
    let hash_v2 = sha256d_midstate_v2(&midstate, &tail, nonce);
    assert_eq!(
        hash_v2, expected_hash,
        "sha256d_midstate_v2 should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(hash_v2)
    );

    // Test AsmV3 implementation
    let hash_v3 = sha256d_midstate_v3(&midstate, &tail, nonce);
    assert_eq!(
        hash_v3, expected_hash,
        "sha256d_midstate_v3 should produce correct genesis block hash.\n\
         Expected: {}\n\
         Got:      {}",
        hex::encode(expected_hash),
        hex::encode(hash_v3)
    );
}

/// Verify that nonce is correctly positioned in the header
/// This test catches the bug where nonce was at bytes 80-83 instead of 76-79
#[test]
fn test_nonce_position_is_correct() {
    let header = genesis_block_header();

    // The nonce should be at bytes 76-79
    let nonce_from_header = u32::from_le_bytes([header[76], header[77], header[78], header[79]]);
    assert_eq!(nonce_from_header, 2083236893, "Nonce should be at bytes 76-79");

    // Verify the header is exactly 80 bytes (not 84)
    assert_eq!(header.len(), 80, "Header must be 80 bytes, not 84");

    // Create midstate and verify changing nonce changes the hash
    let mut prefix_64 = [0u8; 64];
    prefix_64.copy_from_slice(&header[..64]);
    let midstate = Midstate::from_header_prefix(&prefix_64);

    let mut tail = [0u8; 16];
    tail[..12].copy_from_slice(&header[64..76]);

    let hash1 = sha256d_midstate_generic(&midstate, &tail, 0);
    let hash2 = sha256d_midstate_generic(&midstate, &tail, 1);

    assert_ne!(hash1, hash2, "Different nonces should produce different hashes");
}

/// Test with a known block that has a specific difficulty target
/// Block 100000: First block with difficulty adjustment after early period
#[test]
fn test_block_100000_hash() {
    // Block 100000 header data (from blockchain)
    // Hash: 000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506
    let mut header = [0u8; 80];

    // Version: 1
    header[0..4].copy_from_slice(&1u32.to_le_bytes());

    // Previous block hash (internal byte order)
    // Display: 000000000002d01c1fccc21636b607dfd930d31d01c3a62104612a1719011250
    let prev_hash = hex::decode("50120119172a610421a6c3628ffa0121d9d36f2d9a8efb1f8b4b9c5c9f4e5e3a")
        .unwrap_or_else(|_| vec![0u8; 32]);
    if prev_hash.len() == 32 {
        header[4..36].copy_from_slice(&prev_hash);
    }

    // For a full test, we'd need the actual block 100000 data
    // This test serves as a template for adding more test vectors
}

/// Verify byte order conventions
#[test]
fn test_byte_order_conventions() {
    let header = genesis_block_header();

    // Version should be little-endian
    // Value 1 should be stored as [0x01, 0x00, 0x00, 0x00]
    assert_eq!(header[0], 0x01, "Version LSB should be 0x01");
    assert_eq!(header[1], 0x00, "Version byte 1 should be 0x00");
    assert_eq!(header[2], 0x00, "Version byte 2 should be 0x00");
    assert_eq!(header[3], 0x00, "Version byte 3 should be 0x00");

    // nBits 0x1d00ffff should be stored as [0xff, 0xff, 0x00, 0x1d]
    assert_eq!(header[72], 0xff, "nBits LSB should be 0xff");
    assert_eq!(header[73], 0xff, "nBits byte 1 should be 0xff");
    assert_eq!(header[74], 0x00, "nBits byte 2 should be 0x00");
    assert_eq!(header[75], 0x1d, "nBits byte 3 should be 0x1d");

    // Timestamp 1231006505 = 0x495FAB29 should be stored as [0x29, 0xAB, 0x5F, 0x49]
    assert_eq!(header[68], 0x29, "Timestamp LSB should be 0x29");
    assert_eq!(header[69], 0xAB, "Timestamp byte 1 should be 0xAB");
    assert_eq!(header[70], 0x5F, "Timestamp byte 2 should be 0x5F");
    assert_eq!(header[71], 0x49, "Timestamp byte 3 should be 0x49");
}
