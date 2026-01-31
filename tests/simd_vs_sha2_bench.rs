//! Test 7: SIMD vs SHA2 Crate Direct Comparison
//!
//! Isolates where the 303x slowdown is coming from:
//! 1. Is our transform function slow?
//! 2. Is it the surrounding code (block building, memory copies)?
//!
//! Run with:
//!   cargo test --release --test simd_vs_sha2_bench -- --nocapture --ignored

use std::time::{Duration, Instant};
use sha2::{Sha256, Digest};
use btcminer::mining::sha256_simd::{Midstate, H_INIT, state_to_bytes};

fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e12 { format!("{:.2} TH/s", hashrate / 1e12) }
    else if hashrate >= 1e9 { format!("{:.2} GH/s", hashrate / 1e9) }
    else if hashrate >= 1e6 { format!("{:.2} MH/s", hashrate / 1e6) }
    else if hashrate >= 1e3 { format!("{:.2} KH/s", hashrate / 1e3) }
    else { format!("{:.2} H/s", hashrate) }
}

/// Test 1: sha2 crate full header hash (no midstate)
#[test]
#[ignore]
fn bench_sha2_crate_full_hash() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   SHA2 Crate: Full 80-byte Header Hash                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let mut header = [0u8; 80];
    let mut count = 0u64;
    let mut nonce = 0u32;

    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());

            // Double SHA256 using sha2 crate
            let first = Sha256::digest(&header);
            let _hash = Sha256::digest(&first);

            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = count as f64 / elapsed;
    println!("  Hashes:  {}", count);
    println!("  Rate:    {} ({:.2} per thread)", format_hashrate(rate), rate);
    println!();
}

/// Test 2: Our SIMD with midstate (should be faster due to midstate)
#[test]
#[ignore]
fn bench_simd_with_midstate() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Our SIMD: Midstate-Optimized Hash                           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0u8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0u8; 16];
    let target = [0x00u8; 32]; // Hard target

    let mut count = 0u64;
    let mut nonce = 0u32;

    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..10000 {
            // Use our SIMD mining function
            let _ = btcminer::mining::sha256_simd::mine_with_midstate(
                &midstate, &tail, nonce, 1, &target
            );
            nonce = nonce.wrapping_add(1);
            count += 1;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = count as f64 / elapsed;
    println!("  Hashes:  {}", count);
    println!("  Rate:    {} ({:.2} per thread)", format_hashrate(rate), rate);
    println!();
}

/// Test 3: Our SIMD batch (how it's actually called)
#[test]
#[ignore]
fn bench_simd_batch() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   Our SIMD: Batch Mining (262144 nonces per call)             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let header_prefix = [0u8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0u8; 16];
    let target = [0x00u8; 32]; // Hard target
    let batch_size = 262144u32;

    let mut count = 0u64;
    let mut nonce = 0u32;

    let start = Instant::now();
    while start.elapsed() < duration {
        let _ = btcminer::mining::sha256_simd::mine_with_midstate(
            &midstate, &tail, nonce, batch_size, &target
        );
        nonce = nonce.wrapping_add(batch_size);
        count += batch_size as u64;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = count as f64 / elapsed;
    println!("  Hashes:  {}", count);
    println!("  Rate:    {} ({:.2} per thread)", format_hashrate(rate), rate);
    println!();
}

/// Test 4: sha2 crate batch (matching batch size)
#[test]
#[ignore]
fn bench_sha2_crate_batch() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   SHA2 Crate: Batch (262144 nonces per iteration)             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let mut header = [0u8; 80];
    let batch_size = 262144u32;

    let mut count = 0u64;
    let mut nonce = 0u32;

    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..batch_size {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());

            // Double SHA256 using sha2 crate
            let first = Sha256::digest(&header);
            let _hash = Sha256::digest(&first);

            nonce = nonce.wrapping_add(1);
        }
        count += batch_size as u64;
    }

    let elapsed = start.elapsed().as_secs_f64();
    let rate = count as f64 / elapsed;
    println!("  Hashes:  {}", count);
    println!("  Rate:    {} ({:.2} per thread)", format_hashrate(rate), rate);
    println!();
}

/// Run all benchmarks and compare
#[test]
#[ignore]
fn bench_comparison() {
    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║   SIMD vs SHA2 Crate Direct Comparison                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let duration = Duration::from_secs(5);
    let batch_size = 262144u32;

    // Test 1: SHA2 crate (full 80-byte hash)
    println!("Testing SHA2 crate (full 80-byte hash)...");
    let mut header = [0u8; 80];
    let mut count1 = 0u64;
    let mut nonce = 0u32;
    let start = Instant::now();
    while start.elapsed() < duration {
        for _ in 0..batch_size {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());
            let first = Sha256::digest(&header);
            let _hash = Sha256::digest(&first);
            nonce = nonce.wrapping_add(1);
        }
        count1 += batch_size as u64;
    }
    let rate1 = count1 as f64 / start.elapsed().as_secs_f64();

    // Test 2: Our SIMD (midstate optimized)
    println!("Testing Our SIMD (midstate optimized)...");
    let header_prefix = [0u8; 64];
    let midstate = Midstate::from_header_prefix(&header_prefix);
    let tail = [0u8; 16];
    let target = [0x00u8; 32];
    let mut count2 = 0u64;
    nonce = 0;
    let start = Instant::now();
    while start.elapsed() < duration {
        let _ = btcminer::mining::sha256_simd::mine_with_midstate(
            &midstate, &tail, nonce, batch_size, &target
        );
        nonce = nonce.wrapping_add(batch_size);
        count2 += batch_size as u64;
    }
    let rate2 = count2 as f64 / start.elapsed().as_secs_f64();

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESULTS (Single Thread)");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  {:30} {:>15}", "Implementation", "Hashrate");
    println!("  {:30} {:>15}", "──────────────", "────────");
    println!("  {:30} {:>15}", "SHA2 Crate (full hash)", format_hashrate(rate1));
    println!("  {:30} {:>15}", "Our SIMD (midstate)", format_hashrate(rate2));
    println!();

    let ratio = rate1 / rate2;
    if ratio > 1.0 {
        println!("  SHA2 Crate is {:.1}x faster than Our SIMD", ratio);
    } else {
        println!("  Our SIMD is {:.1}x faster than SHA2 Crate", 1.0 / ratio);
    }

    println!();
    println!("  Note: SHA2 crate does full 80-byte hash (no midstate)");
    println!("  Our SIMD uses midstate optimization but is still slower.");
    println!("  This indicates a problem in our SIMD transform or loop code.");
    println!();
}
