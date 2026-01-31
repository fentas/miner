//! Block Finding Integration Tests
//!
//! These tests verify that the miner correctly handles block/share finding scenarios.
//! Critical for ensuring that when a valid hash is found, the entire submission
//! pipeline works correctly.

use btcminer::mining::{
    Coordinator, Job, Share, Stats, hash_meets_target, difficulty_to_target,
    Hasher, HasherType,
};
use std::sync::atomic::Ordering;

/// Test that Share struct contains all fields needed for pool submission
#[test]
fn test_share_has_required_fields() {
    let share = Share {
        job_id: "test-job-1".to_string(),
        nonce: 0xDEADBEEF,
        hash: [0u8; 32],
        worker_id: 0,
        ntime: 0x12345678,
        extranonce2: vec![0x01, 0x02, 0x03, 0x04],
    };

    // Verify all pool submission fields are present
    assert_eq!(share.job_id, "test-job-1");
    assert_eq!(share.nonce, 0xDEADBEEF);
    assert_eq!(share.ntime, 0x12345678);
    assert_eq!(share.extranonce2, vec![0x01, 0x02, 0x03, 0x04]);
}

/// Test that ntime encoding for pool submission is correct
#[test]
fn test_share_ntime_encoding_for_pool() {
    let ntime: u32 = 0x6700f86b;
    let share = Share {
        job_id: "job".to_string(),
        nonce: 0,
        hash: [0u8; 32],
        worker_id: 0,
        ntime,
        extranonce2: vec![],
    };

    // Pool expects ntime as big-endian hex
    let ntime_hex = hex::encode(share.ntime.to_be_bytes());
    assert_eq!(ntime_hex, "6700f86b");
}

/// Test that nonce encoding for pool submission is correct
#[test]
fn test_share_nonce_encoding_for_pool() {
    let nonce: u32 = 0xDEADBEEF;
    let share = Share {
        job_id: "job".to_string(),
        nonce,
        hash: [0u8; 32],
        worker_id: 0,
        ntime: 0,
        extranonce2: vec![],
    };

    // Pool expects nonce as little-endian hex
    let nonce_hex = hex::encode(share.nonce.to_le_bytes());
    assert_eq!(nonce_hex, "efbeadde");
}

/// Test that extranonce2 encoding is correct
#[test]
fn test_share_extranonce2_encoding_for_pool() {
    let share = Share {
        job_id: "job".to_string(),
        nonce: 0,
        hash: [0u8; 32],
        worker_id: 5,
        ntime: 0,
        extranonce2: vec![0x05, 0x00, 0x00, 0x00],  // Worker ID 5 in little-endian
    };

    let extranonce2_hex = hex::encode(&share.extranonce2);
    assert_eq!(extranonce2_hex, "05000000");
}

/// Test job building with all required solo mining fields
#[test]
fn test_job_has_solo_mining_fields() {
    let mut job = Job::new("solo-job".to_string());

    // Set up a solo mining job
    job.version = 0x20000000;
    job.prev_hash = [0x11; 32];
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    job.network_target = Some(job.target);
    job.merkle_root_override = Some([0x22; 32]);
    job.coinbase_tx = Some(vec![0x01, 0x00, 0x00, 0x00, 0x01]);
    job.transactions = Some(vec![]);

    // Verify block can be built
    let block_hex = job.build_block(0xDEADBEEF);
    assert!(block_hex.is_some(), "Job should be able to build block for solo mining");
}

/// Test that build_block fails gracefully with missing data
#[test]
fn test_job_build_block_missing_coinbase() {
    let mut job = Job::new("test".to_string());
    job.transactions = Some(vec![]);
    // Missing coinbase_tx

    let block = job.build_block(0);
    assert!(block.is_none(), "build_block should return None without coinbase_tx");
}

/// Test that build_block fails gracefully with missing transactions
#[test]
fn test_job_build_block_missing_transactions() {
    let mut job = Job::new("test".to_string());
    job.coinbase_tx = Some(vec![0x01]);
    // Missing transactions

    let block = job.build_block(0);
    assert!(block.is_none(), "build_block should return None without transactions");
}

/// Test that meets_network_target correctly identifies blocks
#[test]
fn test_job_meets_network_target() {
    let mut job = Job::new("test".to_string());

    // Set a target that requires 4 leading zero bytes
    let network_target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
                         0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                         0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                         0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    job.network_target = Some(network_target);

    // Hash that meets target (has 4 leading zero bytes)
    let good_hash = [0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x56, 0x78,
                     0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44,
                     0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                     0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44];
    assert!(job.meets_network_target(&good_hash), "Hash with 4 leading zeros should meet target");

    // Hash that doesn't meet target
    let bad_hash = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    assert!(!job.meets_network_target(&bad_hash), "Hash with byte 3 = 0x01 should not meet target");
}

/// Test that meets_target returns false when no network_target is set
#[test]
fn test_job_meets_network_target_not_set() {
    let job = Job::new("test".to_string());
    // network_target is None by default

    let hash = [0x00; 32]; // All zeros should meet any target
    assert!(!job.meets_network_target(&hash), "Should return false when network_target is None");
}

/// Test hash_meets_target comparison logic
#[test]
fn test_hash_meets_target_comparison() {
    let target = [0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    // Test exact match
    assert!(hash_meets_target(&target, &target), "Equal hash should meet target");

    // Test less than
    let less_hash = [0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(hash_meets_target(&less_hash, &target), "Smaller hash should meet target");

    // Test greater than
    let greater_hash = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(!hash_meets_target(&greater_hash, &target), "Larger hash should not meet target");
}

/// Test difficulty_to_target conversion
#[test]
fn test_difficulty_to_target_values() {
    // Difficulty 1 should produce the standard diff1 target
    let target1 = difficulty_to_target(1.0);
    assert_eq!(target1[0..4], [0x00, 0x00, 0x00, 0x00]);
    assert_eq!(target1[4], 0xff);
    assert_eq!(target1[5], 0xff);

    // Difficulty 2 should produce half the target
    let target2 = difficulty_to_target(2.0);
    assert_eq!(target2[0..4], [0x00, 0x00, 0x00, 0x00]);
    // 0xFFFF / 2 = 0x7FFF
    assert_eq!(target2[4], 0x7f);
    assert_eq!(target2[5], 0xff);

    // Very high difficulty should produce minimal target
    let target_high = difficulty_to_target(1e30);
    assert_eq!(target_high, [0x00; 32]);

    // Zero difficulty should produce max target
    let target_zero = difficulty_to_target(0.0);
    assert_eq!(target_zero, [0xff; 32]);
}

/// Test block header structure is correct
#[test]
fn test_job_build_header_structure() {
    let mut job = Job::new("test".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0x11; 32];
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.merkle_root_override = Some([0x22; 32]);

    let header = job.build_header(&[], &[], 0xDEADBEEF);

    // Verify header is 80 bytes
    assert_eq!(header.len(), 80);

    // Verify version at offset 0 (little-endian)
    assert_eq!(&header[0..4], &0x20000000u32.to_le_bytes());

    // Verify prev_hash at offset 4
    assert_eq!(&header[4..36], &[0x11; 32]);

    // Verify merkle_root at offset 36
    assert_eq!(&header[36..68], &[0x22; 32]);

    // Verify ntime at offset 68 (little-endian)
    assert_eq!(&header[68..72], &0x12345678u32.to_le_bytes());

    // Verify nbits at offset 72 (little-endian)
    assert_eq!(&header[72..76], &0x1d00ffffu32.to_le_bytes());

    // Verify nonce at offset 76 (little-endian)
    assert_eq!(&header[76..80], &0xDEADBEEFu32.to_le_bytes());
}

/// Test that changing nonce in header produces different hash
#[test]
fn test_different_nonces_produce_different_hashes() {
    let mut job = Job::new("test".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0x11; 32];
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.merkle_root_override = Some([0x22; 32]);

    let header1 = job.build_header(&[], &[], 0);
    let header2 = job.build_header(&[], &[], 1);

    // Headers should differ only in nonce
    assert_eq!(&header1[0..76], &header2[0..76]);
    assert_ne!(&header1[76..80], &header2[76..80]);

    // Hash the headers
    let hasher = Hasher::new();
    let hash1 = hasher.hash_header(&header1);
    let hash2 = hasher.hash_header(&header2);

    assert_ne!(hash1, hash2, "Different nonces should produce different hashes");
}

/// Test stats tracking for block finding
#[test]
fn test_stats_block_found_tracking() {
    let stats = Stats::new();

    assert_eq!(stats.blocks_found.load(Ordering::Relaxed), 0);

    stats.found_block();
    assert_eq!(stats.blocks_found.load(Ordering::Relaxed), 1);

    stats.found_block();
    stats.found_block();
    assert_eq!(stats.blocks_found.load(Ordering::Relaxed), 3);
}

/// Test share acceptance/rejection tracking
#[test]
fn test_stats_share_tracking() {
    let stats = Stats::new();

    assert_eq!(stats.shares_accepted.load(Ordering::Relaxed), 0);
    assert_eq!(stats.shares_rejected.load(Ordering::Relaxed), 0);

    stats.accept_share();
    stats.accept_share();
    stats.reject_share();

    assert_eq!(stats.shares_accepted.load(Ordering::Relaxed), 2);
    assert_eq!(stats.shares_rejected.load(Ordering::Relaxed), 1);
}

/// Test coordinator can receive shares
#[test]
fn test_coordinator_share_channel() {
    let mut coord = Coordinator::new(1);

    // No shares yet
    assert!(coord.try_recv_share().is_none());
}

/// Test varint encoding used in block construction
#[test]
fn test_block_varint_encoding() {
    let mut job = Job::new("test".to_string());
    job.merkle_root_override = Some([0x00; 32]);
    job.coinbase_tx = Some(vec![0xAA; 100]);

    // Test with 1 transaction (just coinbase)
    job.transactions = Some(vec![]);
    let block1 = job.build_block(0).unwrap();
    let bytes1 = hex::decode(&block1).unwrap();
    assert_eq!(bytes1[80], 1, "Varint for 1 tx should be 0x01");

    // Test with 10 transactions
    let mut txs = Vec::new();
    for _ in 0..9 {
        txs.push(vec![0xBB; 10]);
    }
    job.transactions = Some(txs);
    let block10 = job.build_block(0).unwrap();
    let bytes10 = hex::decode(&block10).unwrap();
    assert_eq!(bytes10[80], 10, "Varint for 10 txs should be 0x0a");

    // Test with 253 transactions (requires 3-byte varint)
    let mut txs = Vec::new();
    for _ in 0..252 {
        txs.push(vec![0xCC; 5]);
    }
    job.transactions = Some(txs);
    let block253 = job.build_block(0).unwrap();
    let bytes253 = hex::decode(&block253).unwrap();
    assert_eq!(bytes253[80], 0xfd, "Varint marker for 253 should be 0xfd");
    assert_eq!(bytes253[81], 253, "Varint low byte for 253");
    assert_eq!(bytes253[82], 0, "Varint high byte for 253");
}

/// Test that all hasher implementations produce consistent results for share detection
#[test]
fn test_all_hashers_share_detection() {
    let hasher_types = [
        HasherType::Sha2Crate,
        HasherType::Simd,
        HasherType::Assembly,
        HasherType::AsmV2,
        HasherType::AsmV3,
    ];

    // Create a test header
    let mut job = Job::new("test".to_string());
    job.version = 0x20000000;
    job.prev_hash = [0x00; 32];
    job.ntime = 0x12345678;
    job.nbits = 0x1d00ffff;
    job.merkle_root_override = Some([0x00; 32]);

    let header = job.build_header(&[], &[], 0);
    let mut header_prefix = [0u8; 76];
    header_prefix.copy_from_slice(&header[..76]);

    // All hashers should produce the same hash for the same input
    let base_hasher = Hasher::with_type(HasherType::Sha2Crate);
    let base_hash = base_hasher.hash_header(&header);

    for hasher_type in hasher_types {
        let hasher = Hasher::with_type(hasher_type);
        let hash = hasher.hash_header(&header);
        assert_eq!(
            hash, base_hash,
            "Hasher {:?} should produce same hash as Sha2Crate",
            hasher_type
        );
    }
}

/// Test that hash_nonce_range correctly finds shares when target is easy
#[test]
fn test_hash_nonce_range_finds_share() {
    let mut hasher = Hasher::new();

    // Create a header prefix
    let mut header_prefix = [0u8; 76];
    header_prefix[0..4].copy_from_slice(&0x20000000u32.to_le_bytes()); // version

    // Easy target (all 0xFF)
    let easy_target = [0xFF; 32];

    // Should find a share immediately with easy target
    let result = hasher.hash_nonce_range(&header_prefix, 0, 10, &easy_target);
    assert!(result.is_some(), "Should find share with easy target");

    let (nonce, _hash) = result.unwrap();
    assert!(nonce < 10, "Found nonce should be in searched range");
}

/// Test that hash_nonce_range correctly rejects when target is impossible
#[test]
fn test_hash_nonce_range_no_share_with_impossible_target() {
    let mut hasher = Hasher::new();

    // Create a header prefix
    let mut header_prefix = [0u8; 76];
    header_prefix[0..4].copy_from_slice(&0x20000000u32.to_le_bytes());

    // Impossible target (all zeros)
    let impossible_target = [0x00; 32];

    // Should not find any share
    let result = hasher.hash_nonce_range(&header_prefix, 0, 1000, &impossible_target);
    assert!(result.is_none(), "Should not find share with impossible target");
}

/// Test share clone preserves all fields
#[test]
fn test_share_clone_all_fields() {
    let original = Share {
        job_id: "test-job".to_string(),
        nonce: 0xABCD1234,
        hash: [0x55; 32],
        worker_id: 42,
        ntime: 0x67890ABC,
        extranonce2: vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
    };

    let cloned = original.clone();

    assert_eq!(cloned.job_id, original.job_id);
    assert_eq!(cloned.nonce, original.nonce);
    assert_eq!(cloned.hash, original.hash);
    assert_eq!(cloned.worker_id, original.worker_id);
    assert_eq!(cloned.ntime, original.ntime);
    assert_eq!(cloned.extranonce2, original.extranonce2);
}

/// Test that block building includes nonce in header
#[test]
fn test_block_includes_correct_nonce() {
    let mut job = Job::new("test".to_string());
    job.merkle_root_override = Some([0x00; 32]);
    job.coinbase_tx = Some(vec![0x01, 0x00]);
    job.transactions = Some(vec![]);

    let test_nonce: u32 = 0x12345678;
    let block_hex = job.build_block(test_nonce).unwrap();
    let block_bytes = hex::decode(&block_hex).unwrap();

    // Nonce should be at offset 76 in the header (little-endian)
    let nonce_from_block = u32::from_le_bytes([
        block_bytes[76],
        block_bytes[77],
        block_bytes[78],
        block_bytes[79],
    ]);

    assert_eq!(nonce_from_block, test_nonce, "Block should contain correct nonce");
}

/// Test genesis block nonce is at correct position
#[test]
fn test_genesis_nonce_position() {
    // Bitcoin genesis block nonce is 2083236893
    let genesis_nonce: u32 = 2083236893;

    // Create genesis-like job
    let mut job = Job::new("genesis".to_string());
    job.version = 1;
    job.prev_hash = [0u8; 32];
    job.ntime = 1231006505;
    job.nbits = 0x1d00ffff;

    // Merkle root from genesis block
    let merkle_root = hex::decode("3ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a")
        .unwrap();
    let mut merkle_root_arr = [0u8; 32];
    merkle_root_arr.copy_from_slice(&merkle_root);
    job.merkle_root_override = Some(merkle_root_arr);

    let header = job.build_header(&[], &[], genesis_nonce);

    // Verify nonce is at bytes 76-79
    let nonce_from_header = u32::from_le_bytes([
        header[76], header[77], header[78], header[79]
    ]);
    assert_eq!(nonce_from_header, genesis_nonce);

    // Verify header is exactly 80 bytes
    assert_eq!(header.len(), 80);
}
