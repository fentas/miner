//! Bitcoin helper functions for solo mining
//!
//! This module provides utilities for:
//! - Difficulty conversion from compact bits format
//! - Coinbase transaction building
//! - BIP34 height encoding
//! - Bech32 address decoding
//! - Merkle root computation

use sha2::{Sha256, Digest};

/// Convert compact bits format to difficulty
///
/// The bits field in a block header encodes the target in a compact form.
/// This function converts it to a floating-point difficulty value.
pub fn bits_to_difficulty(bits: u32) -> f64 {
    let exp = (bits >> 24) as i32;
    let mant = (bits & 0x007fffff) as f64;
    if mant == 0.0 {
        return 0.0;
    }
    // Target = mant * 256^(exp-3)
    // Difficulty = diff1_target / target
    // diff1_target = 0xFFFF * 256^26 (when exp=29, mant=0xFFFF for difficulty 1)
    // Simplified: difficulty = 0xFFFF * 256^26 / (mant * 256^(exp-3))
    //           = 0xFFFF * 256^(26 - exp + 3) / mant
    //           = 0xFFFF * 256^(29 - exp) / mant
    let diff1_mant: f64 = 0xFFFF as f64;
    let pow_diff = (29 - exp) as f64;
    (diff1_mant / mant) * (256.0_f64).powf(pow_diff)
}

/// Build a coinbase transaction for solo mining
///
/// Creates a valid coinbase transaction with:
/// - BIP34 encoded block height
/// - Miner tag ("/btcminer/")
/// - Output to the specified payout address
///
/// Returns the raw transaction bytes.
pub fn build_coinbase_tx(height: u64, coinbase_value: u64, payout_address: &str) -> Vec<u8> {
    let mut tx = Vec::new();

    // Version (4 bytes, little-endian)
    tx.extend_from_slice(&1u32.to_le_bytes());

    // Input count (varint)
    tx.push(1);

    // Coinbase input:
    // - Previous output hash (32 zero bytes)
    tx.extend_from_slice(&[0u8; 32]);
    // - Previous output index (0xffffffff)
    tx.extend_from_slice(&0xffffffffu32.to_le_bytes());

    // Script sig: BIP34 height encoding + arbitrary data
    let mut scriptsig = Vec::new();
    // Encode height (BIP34)
    let height_bytes = encode_bip34_height(height);
    scriptsig.extend_from_slice(&height_bytes);
    // Add some arbitrary data (miner tag)
    scriptsig.extend_from_slice(b"/btcminer/");

    // Script length (varint)
    tx.push(scriptsig.len() as u8);
    tx.extend_from_slice(&scriptsig);

    // Sequence (0xffffffff)
    tx.extend_from_slice(&0xffffffffu32.to_le_bytes());

    // Output count (varint)
    tx.push(1);

    // Output value (8 bytes, little-endian)
    tx.extend_from_slice(&coinbase_value.to_le_bytes());

    // Output script (P2PKH or P2WPKH based on address)
    let script = address_to_script(payout_address);
    tx.push(script.len() as u8);
    tx.extend_from_slice(&script);

    // Locktime (4 bytes)
    tx.extend_from_slice(&0u32.to_le_bytes());

    tx
}

/// Encode block height per BIP34
///
/// BIP34 requires the block height to be encoded in the coinbase scriptSig.
pub fn encode_bip34_height(height: u64) -> Vec<u8> {
    let mut result = Vec::new();
    if height < 17 {
        result.push(0x50 + height as u8);
    } else if height <= 0x7f {
        result.push(1);
        result.push(height as u8);
    } else if height <= 0x7fff {
        result.push(2);
        result.extend_from_slice(&(height as u16).to_le_bytes());
    } else if height <= 0x7fffff {
        result.push(3);
        result.push(height as u8);
        result.push((height >> 8) as u8);
        result.push((height >> 16) as u8);
    } else {
        result.push(4);
        result.extend_from_slice(&(height as u32).to_le_bytes());
    }
    result
}

/// Convert a Bitcoin address to output script
///
/// Supports bech32 P2WPKH addresses (bc1q... and tb1q...).
/// Falls back to OP_RETURN for unsupported formats.
pub fn address_to_script(address: &str) -> Vec<u8> {
    // For simplicity, support common formats
    if address.starts_with("bc1q") || address.starts_with("tb1q") {
        // P2WPKH (bech32)
        if let Some(script) = decode_bech32_p2wpkh(address) {
            return script;
        }
    }
    // Fallback: OP_RETURN with address as data (won't be spendable but allows mining)
    let mut script = vec![0x6a]; // OP_RETURN
    let addr_bytes = address.as_bytes();
    script.push(addr_bytes.len() as u8);
    script.extend_from_slice(addr_bytes);
    script
}

/// Decode bech32 P2WPKH address to script with checksum validation (BIP-173)
///
/// Returns the witness program script if valid, None otherwise.
pub fn decode_bech32_p2wpkh(address: &str) -> Option<Vec<u8>> {
    // Bech32 charset
    const CHARSET: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

    // BIP-173 generator polynomial coefficients
    const GEN: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

    // Polymod function for checksum verification
    fn polymod(values: &[u8]) -> u32 {
        let mut chk: u32 = 1;
        for &v in values {
            let top = chk >> 25;
            chk = ((chk & 0x1ffffff) << 5) ^ (v as u32);
            for (i, &g) in GEN.iter().enumerate() {
                if (top >> i) & 1 == 1 {
                    chk ^= g;
                }
            }
        }
        chk
    }

    // Expand HRP for checksum calculation
    fn hrp_expand(hrp: &str) -> Vec<u8> {
        let mut ret: Vec<u8> = hrp.chars().map(|c| (c as u8) >> 5).collect();
        ret.push(0);
        ret.extend(hrp.chars().map(|c| (c as u8) & 31));
        ret
    }

    // Find separator
    let hrp_end = address.rfind('1')?; // Use rfind per BIP-173
    if hrp_end < 1 {
        return None;
    }

    let hrp = &address[..hrp_end];
    let data_part = &address[hrp_end + 1..];

    // Validate HRP (must be bc or tb for Bitcoin)
    if hrp != "bc" && hrp != "tb" {
        return None;
    }

    // Convert characters to 5-bit values
    let mut values: Vec<u8> = Vec::new();
    for c in data_part.chars() {
        let idx = CHARSET.find(c.to_ascii_lowercase())? as u8;
        values.push(idx);
    }

    // Minimum length check (version + data + 6 checksum)
    if values.len() < 8 {
        return None;
    }

    // Verify checksum (polymod should be 1 for valid bech32)
    let mut checksum_data = hrp_expand(hrp);
    checksum_data.extend_from_slice(&values);
    if polymod(&checksum_data) != 1 {
        return None; // Invalid checksum
    }

    // Extract witness version and data (excluding 6-byte checksum)
    let version = values[0];
    if version != 0 {
        return None; // Only support witness version 0 (P2WPKH/P2WSH)
    }

    let data = &values[1..values.len() - 6];

    // Convert from 5-bit to 8-bit
    let mut result = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &val in data {
        acc = (acc << 5) | (val as u32);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            result.push((acc >> bits) as u8);
        }
    }

    // Verify it's 20 bytes (P2WPKH)
    if result.len() != 20 {
        return None;
    }

    // Build witness program: OP_0 <push 20 bytes> <20 bytes pubkey hash>
    let mut script = vec![0x00, 0x14]; // OP_0, push 20 bytes
    script.extend_from_slice(&result);
    Some(script)
}

/// Compute merkle root from coinbase and transaction hashes
///
/// The coinbase transaction is hashed first, then combined with
/// the transaction data from the block template.
pub fn compute_merkle_root(coinbase_tx: &[u8], tx_hashes: &[String]) -> [u8; 32] {
    // Double SHA256 of coinbase tx
    let mut hasher = Sha256::new();
    hasher.update(coinbase_tx);
    let first = hasher.finalize();
    let mut hasher = Sha256::new();
    hasher.update(first);
    let coinbase_hash: [u8; 32] = hasher.finalize().into();

    // Collect all txids (coinbase first, then transactions)
    let mut hashes: Vec<[u8; 32]> = vec![coinbase_hash];

    // Parse transaction hashes from template (they're txids in hex)
    for tx_hex in tx_hashes {
        if let Ok(tx_bytes) = hex::decode(tx_hex) {
            // Transaction data from template - compute txid (double SHA256)
            let mut hasher = Sha256::new();
            hasher.update(&tx_bytes);
            let first = hasher.finalize();
            let mut hasher = Sha256::new();
            hasher.update(first);
            let txid: [u8; 32] = hasher.finalize().into();
            hashes.push(txid);
        }
    }

    // Build merkle tree
    while hashes.len() > 1 {
        // If odd number, duplicate the last hash
        if hashes.len() % 2 == 1 {
            hashes.push(*hashes.last().unwrap());
        }

        let mut new_level = Vec::new();
        for chunk in hashes.chunks(2) {
            let mut combined = [0u8; 64];
            combined[0..32].copy_from_slice(&chunk[0]);
            combined[32..64].copy_from_slice(&chunk[1]);

            let mut hasher = Sha256::new();
            hasher.update(combined);
            let first = hasher.finalize();
            let mut hasher = Sha256::new();
            hasher.update(first);
            let hash: [u8; 32] = hasher.finalize().into();
            new_level.push(hash);
        }
        hashes = new_level;
    }

    hashes.first().copied().unwrap_or([0u8; 32])
}

/// Parse a hex-encoded previous block hash into bytes (reversed for Bitcoin's little-endian format)
///
/// Bitcoin block headers store the previous block hash in little-endian order.
/// This function handles the hex decoding and byte reversal.
pub fn parse_prev_hash(hex_str: &str) -> Option<[u8; 32]> {
    let hash_bytes = hex::decode(hex_str).ok()?;
    if hash_bytes.len() != 32 {
        return None;
    }
    let mut prev_hash = [0u8; 32];
    prev_hash.copy_from_slice(&hash_bytes);
    prev_hash.reverse();
    Some(prev_hash)
}

/// Parse a hex-encoded nbits (compact difficulty target) value
pub fn parse_nbits(bits_hex: &str) -> Option<u32> {
    u32::from_str_radix(bits_hex, 16).ok()
}

/// Parse a hex-encoded target into bytes
pub fn parse_target(hex_str: &str) -> Option<[u8; 32]> {
    let target_bytes = hex::decode(hex_str).ok()?;
    if target_bytes.len() != 32 {
        return None;
    }
    let mut target = [0u8; 32];
    target.copy_from_slice(&target_bytes);
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_bip34_height_small() {
        // Heights 1-16 use OP_1 through OP_16 (0x51-0x60)
        let result = encode_bip34_height(1);
        assert_eq!(result, vec![0x51]);

        let result = encode_bip34_height(16);
        assert_eq!(result, vec![0x60]);
    }

    #[test]
    fn test_encode_bip34_height_one_byte() {
        // Heights 17-127 use push 1 byte
        let result = encode_bip34_height(17);
        assert_eq!(result, vec![1, 17]);

        let result = encode_bip34_height(127);
        assert_eq!(result, vec![1, 127]);
    }

    #[test]
    fn test_encode_bip34_height_two_bytes() {
        // Heights 128-32767 use push 2 bytes
        let result = encode_bip34_height(128);
        assert_eq!(result, vec![2, 128, 0]);

        let result = encode_bip34_height(1000);
        assert_eq!(result, vec![2, 0xe8, 0x03]); // 1000 in little-endian
    }

    #[test]
    fn test_encode_bip34_height_three_bytes() {
        // Heights 32768-8388607 use push 3 bytes
        let result = encode_bip34_height(100000);
        assert_eq!(result, vec![3, 0xa0, 0x86, 0x01]); // 100000 = 0x186A0 in little-endian

        let result = encode_bip34_height(840000);
        assert_eq!(result, vec![3, 0x40, 0xd1, 0x0c]); // 840000 = 0x0CD140 in little-endian
    }

    #[test]
    fn test_encode_bip34_height_four_bytes() {
        // Heights >= 8388608 use push 4 bytes
        let result = encode_bip34_height(10000000);
        assert_eq!(result, vec![4, 0x80, 0x96, 0x98, 0x00]); // 10000000 in little-endian
    }

    #[test]
    fn test_decode_bech32_p2wpkh_valid_mainnet() {
        // Valid mainnet P2WPKH address
        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        let result = decode_bech32_p2wpkh(address);
        assert!(result.is_some());
        let script = result.unwrap();
        // P2WPKH: OP_0 (0x00) + push 20 bytes (0x14) + 20-byte pubkey hash
        assert_eq!(script.len(), 22);
        assert_eq!(script[0], 0x00); // OP_0
        assert_eq!(script[1], 0x14); // Push 20 bytes
    }

    #[test]
    fn test_decode_bech32_p2wpkh_valid_testnet() {
        // Valid testnet P2WPKH address
        let address = "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx";
        let result = decode_bech32_p2wpkh(address);
        assert!(result.is_some());
        let script = result.unwrap();
        assert_eq!(script.len(), 22);
    }

    #[test]
    fn test_decode_bech32_p2wpkh_invalid_too_short() {
        let address = "bc1qshort";
        let result = decode_bech32_p2wpkh(address);
        // Should fail - too short to be valid
        assert!(result.is_none() || result.unwrap().len() != 22);
    }

    #[test]
    fn test_decode_bech32_p2wpkh_no_separator() {
        let address = "invalidaddresswithnoseparator";
        let result = decode_bech32_p2wpkh(address);
        assert!(result.is_none());
    }

    #[test]
    fn test_address_to_script_bech32() {
        let address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
        let script = address_to_script(address);
        // Should be P2WPKH script
        assert_eq!(script.len(), 22);
        assert_eq!(script[0], 0x00); // OP_0
        assert_eq!(script[1], 0x14); // Push 20 bytes
    }

    #[test]
    fn test_address_to_script_fallback() {
        // Non-bech32 address falls back to OP_RETURN
        let address = "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2";
        let script = address_to_script(address);
        assert_eq!(script[0], 0x6a); // OP_RETURN
    }

    #[test]
    fn test_build_coinbase_tx_basic() {
        let tx = build_coinbase_tx(840000, 312500000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        // Check version (first 4 bytes, little-endian)
        assert_eq!(&tx[0..4], &1u32.to_le_bytes());

        // Check input count
        assert_eq!(tx[4], 1);

        // Check previous output hash (32 zero bytes)
        assert_eq!(&tx[5..37], &[0u8; 32]);

        // Check previous output index (0xffffffff)
        assert_eq!(&tx[37..41], &0xffffffffu32.to_le_bytes());

        // Transaction should be non-empty and reasonable size
        assert!(tx.len() > 80);
        assert!(tx.len() < 500);
    }

    #[test]
    fn test_build_coinbase_tx_contains_height() {
        let tx = build_coinbase_tx(840000, 312500000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        // The scriptsig should contain the BIP34 encoded height
        // Height 840000 = 0x0CD140 in little-endian: [0x40, 0xd1, 0x0c]
        let height_bytes = [0x40u8, 0xd1, 0x0c];

        // Find the height in the transaction
        let tx_str = hex::encode(&tx);
        let height_hex = hex::encode(height_bytes);
        assert!(tx_str.contains(&height_hex), "Transaction should contain BIP34 height");
    }

    #[test]
    fn test_build_coinbase_tx_contains_miner_tag() {
        let tx = build_coinbase_tx(100000, 5000000000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        // Check that miner tag is present
        let miner_tag = b"/btcminer/";
        let tx_contains_tag = tx.windows(miner_tag.len()).any(|w| w == miner_tag);
        assert!(tx_contains_tag, "Transaction should contain miner tag");
    }

    #[test]
    fn test_build_coinbase_tx_output_value() {
        let coinbase_value: u64 = 312500000; // 3.125 BTC
        let tx = build_coinbase_tx(840000, coinbase_value, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        // Find output value in transaction (8 bytes after output count)
        // The output value should be in the transaction
        let value_bytes = coinbase_value.to_le_bytes();
        let tx_contains_value = tx.windows(8).any(|w| w == value_bytes);
        assert!(tx_contains_value, "Transaction should contain output value");
    }

    #[test]
    fn test_compute_merkle_root_single_tx() {
        // With just coinbase, merkle root = double SHA256 of coinbase
        let coinbase = vec![0x01, 0x02, 0x03, 0x04];
        let merkle_root = compute_merkle_root(&coinbase, &[]);

        // Verify it's 32 bytes
        assert_eq!(merkle_root.len(), 32);

        // Compute expected value manually
        let mut hasher = Sha256::new();
        hasher.update(&coinbase);
        let first = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(first);
        let expected: [u8; 32] = hasher.finalize().into();

        assert_eq!(merkle_root, expected);
    }

    #[test]
    fn test_compute_merkle_root_deterministic() {
        let coinbase = build_coinbase_tx(840000, 312500000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        // Same inputs should produce same output
        let root1 = compute_merkle_root(&coinbase, &[]);
        let root2 = compute_merkle_root(&coinbase, &[]);

        assert_eq!(root1, root2);
    }

    #[test]
    fn test_compute_merkle_root_different_inputs() {
        let coinbase1 = build_coinbase_tx(840000, 312500000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
        let coinbase2 = build_coinbase_tx(840001, 312500000, "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");

        let root1 = compute_merkle_root(&coinbase1, &[]);
        let root2 = compute_merkle_root(&coinbase2, &[]);

        assert_ne!(root1, root2, "Different coinbases should produce different merkle roots");
    }

    #[test]
    fn test_bits_to_difficulty_genesis() {
        // Genesis block bits: 0x1d00ffff
        // This should give difficulty ~1
        let diff = bits_to_difficulty(0x1d00ffff);
        assert!((diff - 1.0).abs() < 0.01, "Genesis difficulty should be ~1, got {}", diff);
    }

    #[test]
    fn test_bits_to_difficulty_higher() {
        // Higher difficulty (more leading zeros in target)
        // bits = 0x170c69e6 is approximately difficulty 84,254,428,880,224
        let bits: u32 = 0x170c69e6;
        let diff = bits_to_difficulty(bits);
        assert!(diff > 1_000_000_000_000.0, "High bits should give high difficulty, got {}", diff);
    }

    #[test]
    fn test_bits_to_difficulty_increases_with_harder_target() {
        // Lower exponent = harder target = higher difficulty
        let easy_bits: u32 = 0x1d00ffff; // difficulty ~1
        let hard_bits: u32 = 0x170c69e6; // very high difficulty

        let easy_diff = bits_to_difficulty(easy_bits);
        let hard_diff = bits_to_difficulty(hard_bits);

        assert!(hard_diff > easy_diff, "Harder target should have higher difficulty");
    }

    #[test]
    fn test_parse_prev_hash_valid() {
        // 32-byte hash in hex (64 characters)
        let hex = "0000000000000000000000000000000000000000000000000000000000001234";
        let result = parse_prev_hash(hex);
        assert!(result.is_some());
        let hash = result.unwrap();
        // Should be reversed (little-endian)
        assert_eq!(hash[0], 0x34);
        assert_eq!(hash[1], 0x12);
    }

    #[test]
    fn test_parse_prev_hash_invalid_length() {
        // Too short
        let hex = "0000000000001234";
        assert!(parse_prev_hash(hex).is_none());
    }

    #[test]
    fn test_parse_prev_hash_invalid_hex() {
        // Invalid hex characters
        let hex = "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg";
        assert!(parse_prev_hash(hex).is_none());
    }

    #[test]
    fn test_parse_nbits_valid() {
        assert_eq!(parse_nbits("1d00ffff"), Some(0x1d00ffff));
        assert_eq!(parse_nbits("170c69e6"), Some(0x170c69e6));
    }

    #[test]
    fn test_parse_nbits_invalid() {
        assert!(parse_nbits("invalid").is_none());
        assert!(parse_nbits("gggg").is_none());
    }

    #[test]
    fn test_parse_target_valid() {
        // 32-byte target in hex
        let hex = "00000000ffff0000000000000000000000000000000000000000000000000000";
        let result = parse_target(hex);
        assert!(result.is_some());
        let target = result.unwrap();
        assert_eq!(target[0], 0x00);
        assert_eq!(target[4], 0xff);
        assert_eq!(target[5], 0xff);
    }

    #[test]
    fn test_parse_target_invalid_length() {
        let hex = "00000000ffff";
        assert!(parse_target(hex).is_none());
    }
}
