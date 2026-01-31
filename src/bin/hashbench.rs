//! Standalone hasher benchmark binary
//!
//! Run: cargo run --release --bin hashbench

use std::time::Instant;
use btcminer::mining::{Hasher, HasherType};

fn main() {
    println!("=== Standalone Hasher Benchmark ===\n");

    let header = [0u8; 76];
    let target = [0x00; 32]; // Hard target
    let batch_size = 65536u32;
    let duration_secs = 10;

    // SIMD benchmark
    {
        let mut hasher = Hasher::with_type(HasherType::Simd);
        println!("SIMD Hasher ({}s):", duration_secs);

        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed().as_secs() < duration_secs {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let elapsed = start.elapsed().as_secs_f64();
        let hashrate = total_hashes as f64 / elapsed;

        println!("  Total hashes: {}", total_hashes);
        println!("  Elapsed: {:.2}s", elapsed);
        println!("  Hashrate: {:.2} MH/s ({:.2} GH/s)\n",
            hashrate / 1_000_000.0, hashrate / 1_000_000_000.0);
    }

    // SHA2 benchmark
    {
        let mut hasher = Hasher::with_type(HasherType::Sha2Crate);
        println!("SHA2 Hasher ({}s):", duration_secs);

        let start = Instant::now();
        let mut total_hashes = 0u64;
        let mut nonce = 0u32;

        while start.elapsed().as_secs() < duration_secs {
            let _ = hasher.hash_nonce_range(&header, nonce, batch_size, &target);
            total_hashes += batch_size as u64;
            nonce = nonce.wrapping_add(batch_size);
        }

        let elapsed = start.elapsed().as_secs_f64();
        let hashrate = total_hashes as f64 / elapsed;

        println!("  Total hashes: {}", total_hashes);
        println!("  Elapsed: {:.2}s", elapsed);
        println!("  Hashrate: {:.2} MH/s ({:.2} GH/s)\n",
            hashrate / 1_000_000.0, hashrate / 1_000_000_000.0);
    }

    println!("Done!");
}
