//! Verify hash computation isn't optimized away
use std::hint::black_box;
use std::time::Instant;
use btcminer::mining::sha256_simd::{Midstate, sha256d_midstate_v3, sha256d_midstate_simd, sha256d_midstate_generic};
use sha2::{Sha256, Digest};

fn main() {
    let header_prefix = [0xABu8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0x12u8; 16];
    let count = 1_000_000u64;
    
    println!("Single-thread SHA-256d benchmark (with black_box to prevent optimization)\n");
    
    // Test AsmV3
    let start = Instant::now();
    let mut hash = [0u8; 32];
    for nonce in 0..count as u32 {
        hash = sha256d_midstate_v3(&midstate, &tail, nonce);
        black_box(&hash);
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("AsmV3:    {:.2} MH/s (sample: {:02x}{:02x}{:02x}{:02x}...)", 
             count as f64 / elapsed / 1e6, hash[0], hash[1], hash[2], hash[3]);
    
    // Test SIMD
    let start = Instant::now();
    for nonce in 0..count as u32 {
        hash = sha256d_midstate_simd(&midstate, &tail, nonce);
        black_box(&hash);
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("SIMD:     {:.2} MH/s (sample: {:02x}{:02x}{:02x}{:02x}...)", 
             count as f64 / elapsed / 1e6, hash[0], hash[1], hash[2], hash[3]);
    
    // Test Generic
    let start = Instant::now();
    for nonce in 0..count as u32 {
        hash = sha256d_midstate_generic(&midstate, &tail, nonce);
        black_box(&hash);
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("Generic:  {:.2} MH/s (sample: {:02x}{:02x}{:02x}{:02x}...)", 
             count as f64 / elapsed / 1e6, hash[0], hash[1], hash[2], hash[3]);
    
    // Test SHA2 crate (full hash, no midstate)
    let mut header = [0u8; 80];
    header[..64].copy_from_slice(&header_prefix);
    header[64..76].copy_from_slice(&tail[..12]);
    let start = Instant::now();
    for nonce in 0..count as u32 {
        header[76..80].copy_from_slice(&nonce.to_le_bytes());
        let mut hasher = Sha256::new();
        hasher.update(&header);
        let first = hasher.finalize();
        let mut hasher = Sha256::new();
        hasher.update(&first);
        hash = hasher.finalize().into();
        black_box(&hash);
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("SHA2 crate: {:.2} MH/s (sample: {:02x}{:02x}{:02x}{:02x}...)", 
             count as f64 / elapsed / 1e6, hash[0], hash[1], hash[2], hash[3]);
    
    println!("\nNote: SHA2 crate does full 80-byte hash (no midstate), so it's doing MORE work.");
}
