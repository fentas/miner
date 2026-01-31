//! API Hashrate Test
//!
//! Starts the actual btcminer binary and reads hashrate from the web API.
//! This is the most accurate way to measure production performance.
//!
//! Run: cargo test --release --test api_hashrate_test -- --nocapture --ignored
//!
//! Port configuration (in order of priority):
//! 1. BTCMINER_API_PORT environment variable
//! 2. web.port from btcminer.toml config file
//! 3. Default: 9080

use std::process::{Command, Child, Stdio};
use std::time::{Duration, Instant};
use std::thread;

/// Get the API port from config or environment
fn get_api_port() -> u16 {
    // 1. Check environment variable first
    if let Ok(port_str) = std::env::var("BTCMINER_API_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            return port;
        }
    }

    // 2. Try to load from btcminer.toml using the actual Config struct
    if let Ok(config) = btcminer::config::Config::from_file("btcminer.toml") {
        return config.web.port;
    }

    // 3. Default port (matches user's typical config)
    9080
}

fn get_api_url() -> String {
    format!("http://127.0.0.1:{}/api/stats", get_api_port())
}

/// Start btcminer binary and return the child process
fn start_miner() -> Child {
    #[cfg(windows)]
    let binary = "target/release/btcminer.exe";
    #[cfg(not(windows))]
    let binary = "target/release/btcminer";

    Command::new(binary)
        .args(["--no-hud"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start btcminer")
}

/// Wait for API to become available
fn wait_for_api(timeout: Duration) -> bool {
    let api_url = get_api_url();
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(resp) = ureq::get(&api_url).timeout(Duration::from_secs(1)).call() {
            if resp.status() == 200 {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Fetch stats from API
fn fetch_stats() -> Option<serde_json::Value> {
    let api_url = get_api_url();
    match ureq::get(&api_url).timeout(Duration::from_secs(5)).call() {
        Ok(resp) => {
            if resp.status() == 200 {
                resp.into_json().ok()
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

/// Main test: start binary, wait, read API, measure hashrate
#[test]
#[ignore]
fn api_hashrate_measurement() {
    println!("\n=== API Hashrate Test ===\n");

    // Show port configuration
    let api_port = get_api_port();
    println!("API port: {} (set BTCMINER_API_PORT env var to override)", api_port);

    // Note: Assumes binary is already built (run: cargo build --release)
    // Start miner
    println!("Starting btcminer...");
    let mut child = start_miner();

    // Wait for API (60 seconds to allow for Bitcoin Core sync check)
    println!("Waiting for API on port {}...", api_port);
    if !wait_for_api(Duration::from_secs(60)) {
        // Capture stderr before killing
        if let Some(stderr) = child.stderr.take() {
            use std::io::{BufRead, BufReader};
            let reader = BufReader::new(stderr);
            println!("\n=== Miner stderr output ===");
            for line in reader.lines().take(50) {
                if let Ok(line) = line {
                    println!("  {}", line);
                }
            }
        }
        let _ = child.kill();
        panic!("API did not become available within 60 seconds");
    }
    println!("API ready!\n");

    // Collect samples
    let mut samples: Vec<(u64, f64, Instant)> = Vec::new();
    let duration = Duration::from_secs(30);
    let start = Instant::now();

    println!("Collecting samples for {} seconds...", duration.as_secs());
    while start.elapsed() < duration {
        if let Some(stats) = fetch_stats() {
            let hashes = stats["hashes"].as_u64().unwrap_or(0);
            let hashrate = stats["hashrate"].as_f64().unwrap_or(0.0);
            let now = Instant::now();

            println!("  Hashes: {:>15}, Hashrate: {:>10.2} H/s ({:.2} GH/s)",
                hashes, hashrate, hashrate / 1e9);

            samples.push((hashes, hashrate, now));
        }
        thread::sleep(Duration::from_secs(2));
    }

    // Stop miner
    println!("\nStopping miner...");
    let _ = child.kill();
    let _ = child.wait();

    // Analyze results
    println!("\n=== Results ===");
    println!("Collected {} samples\n", samples.len());

    if samples.len() >= 2 {
        // Calculate hashrate from hash count difference
        let first = &samples[0];
        let last = &samples[samples.len() - 1];
        let hash_diff = last.0.saturating_sub(first.0);
        let time_diff = last.2.duration_since(first.2).as_secs_f64();

        if time_diff > 0.0 && hash_diff > 0 {
            let calculated_rate = hash_diff as f64 / time_diff;
            println!("From hash count delta:");
            println!("  Hashes: {} -> {} (delta: {})", first.0, last.0, hash_diff);
            println!("  Time: {:.2}s", time_diff);
            println!("  Calculated rate: {:.2} GH/s", calculated_rate / 1e9);
        }

        // Average reported hashrate
        let avg_reported: f64 = samples.iter().map(|s| s.1).sum::<f64>() / samples.len() as f64;
        let max_reported: f64 = samples.iter().map(|s| s.1).fold(0.0, f64::max);

        println!("\nFrom API hashrate field:");
        println!("  Average: {:.2} GH/s", avg_reported / 1e9);
        println!("  Max: {:.2} GH/s", max_reported / 1e9);
    }

    // Compare with test baseline
    println!("\n=== Comparison ===");
    println!("Test suite (cargo test): ~23 MH/s single thread, ~230 MH/s 24 threads");
    println!("Production binary should show significantly higher rates.");
}

/// Quick API connectivity test
#[test]
#[ignore]
fn api_connectivity() {
    println!("\n=== API Connectivity Test ===\n");
    println!("Using port: {} (set BTCMINER_API_PORT to override)\n", get_api_port());

    // Check if miner is already running
    if let Some(stats) = fetch_stats() {
        println!("Miner already running!");
        println!("Stats: {}", serde_json::to_string_pretty(&stats).unwrap_or_default());
        return;
    }

    println!("Miner not running. Start it manually with:");
    println!("  cargo run --release --bin btcminer");
    println!("\nThen run this test again.");
}
