//! End-to-end mining tests with mock RPC server
//!
//! These tests run the actual mining code against a mock Bitcoin RPC server
//! to verify the complete mining and block submission flow.
//!
//! Run with: cargo test --release e2e_ -- --nocapture --ignored

use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::runtime::Runtime;

use btcminer::bitcoin::BitcoinRpc;
use btcminer::mining::{Coordinator, Job, Hasher, difficulty_to_target};

/// Mock RPC server that simulates Bitcoin Core
struct MockRpcServer {
    port: u16,
    running: Arc<AtomicBool>,
    blocks_submitted: Arc<AtomicU64>,
    submitted_blocks: Arc<Mutex<Vec<String>>>,
    #[allow(dead_code)] // Reserved for future tests
    block_height: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockRpcServer {
    /// Start a mock RPC server on a random available port
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let port = listener.local_addr().unwrap().port();

        let running = Arc::new(AtomicBool::new(true));
        let blocks_submitted = Arc::new(AtomicU64::new(0));
        let submitted_blocks = Arc::new(Mutex::new(Vec::new()));
        let block_height = Arc::new(AtomicU64::new(100));

        let running_clone = Arc::clone(&running);
        let blocks_submitted_clone = Arc::clone(&blocks_submitted);
        let submitted_blocks_clone = Arc::clone(&submitted_blocks);
        let block_height_clone = Arc::clone(&block_height);

        let handle = std::thread::spawn(move || {
            listener.set_nonblocking(true).ok();

            while running_clone.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = listener.accept() {
                    use std::io::{Read, Write};

                    let mut buffer = [0u8; 8192];
                    if let Ok(n) = stream.read(&mut buffer) {
                        let request = String::from_utf8_lossy(&buffer[..n]);

                        // Parse JSON-RPC request from HTTP body
                        if let Some(body_start) = request.find("\r\n\r\n") {
                            let body = &request[body_start + 4..];
                            if let Ok(json) = serde_json::from_str::<Value>(body) {
                                let method = json["method"].as_str().unwrap_or("");
                                let response = Self::handle_method(
                                    method,
                                    &json["params"],
                                    &blocks_submitted_clone,
                                    &submitted_blocks_clone,
                                    &block_height_clone,
                                );

                                let response_body = serde_json::to_string(&json!({
                                    "result": response,
                                    "error": null,
                                    "id": json["id"]
                                })).unwrap();

                                let http_response = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                    response_body.len(),
                                    response_body
                                );

                                let _ = stream.write_all(http_response.as_bytes());
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        MockRpcServer {
            port,
            running,
            blocks_submitted,
            submitted_blocks,
            block_height,
            handle: Some(handle),
        }
    }

    fn handle_method(
        method: &str,
        params: &Value,
        blocks_submitted: &Arc<AtomicU64>,
        submitted_blocks: &Arc<Mutex<Vec<String>>>,
        block_height: &Arc<AtomicU64>,
    ) -> Value {
        match method {
            "getblockchaininfo" => {
                json!({
                    "initialblockdownload": false,
                    "verificationprogress": 0.9999999,
                    "headers": block_height.load(Ordering::Relaxed),
                    "blocks": block_height.load(Ordering::Relaxed),
                    "bestblockhash": "0000000000000000000abc123",
                    "difficulty": 1.0
                })
            }
            "getblocktemplate" => {
                let height = block_height.load(Ordering::Relaxed);
                // Return a very easy target (low difficulty)
                json!({
                    "version": 0x20000000_u64,
                    "previousblockhash": "0000000000000000000000000000000000000000000000000000000000000000",
                    "curtime": 1700000000_u64,
                    "bits": "207fffff", // Very easy difficulty
                    "height": height,
                    "coinbasevalue": 312500000_u64,
                    "transactions": [],
                    // Easy target - lots of leading 0x7F
                    "target": "7fffff0000000000000000000000000000000000000000000000000000000000"
                })
            }
            "submitblock" => {
                if let Some(block_hex) = params.get(0).and_then(|v| v.as_str()) {
                    blocks_submitted.fetch_add(1, Ordering::Relaxed);
                    block_height.fetch_add(1, Ordering::Relaxed);
                    submitted_blocks.lock().unwrap().push(block_hex.to_string());
                    println!("  [MockRPC] Block submitted! ({} bytes)", block_hex.len() / 2);
                }
                Value::Null // Success
            }
            "getbestblockhash" => {
                json!("0000000000000000000abc123")
            }
            _ => {
                eprintln!("  [MockRPC] Unknown method: {}", method);
                Value::Null
            }
        }
    }

    #[allow(dead_code)] // Useful for debugging
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn get_blocks_submitted(&self) -> u64 {
        self.blocks_submitted.load(Ordering::Relaxed)
    }

    fn get_submitted_blocks(&self) -> Vec<String> {
        self.submitted_blocks.lock().unwrap().clone()
    }
}

impl Drop for MockRpcServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Test end-to-end: RPC connection and block template fetching
#[test]
#[ignore]
fn e2e_rpc_get_block_template() {
    println!("\n=== E2E Test: RPC Get Block Template ===\n");

    let server = MockRpcServer::start();
    println!("Mock RPC server started on port {}", server.port);

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let rpc = BitcoinRpc::new("127.0.0.1", server.port);

        // Test getblockchaininfo
        let status = rpc.get_blockchain_info().await;
        assert!(status.is_ok(), "Should get blockchain info");
        let status = status.unwrap();
        println!("Blockchain synced: {}", status.synced);
        assert!(status.synced, "Should report as synced");

        // Test getblocktemplate
        let template = rpc.get_block_template("bc1qtest").await;
        assert!(template.is_ok(), "Should get block template");
        let template = template.unwrap();
        println!("Block template height: {}", template.height);
        println!("Block template target: {}", template.target);
        assert!(template.height > 0, "Height should be positive");
    });

    println!("\nTest passed!\n");
}

/// Test end-to-end: Mining and block submission
#[test]
#[ignore]
fn e2e_mine_and_submit_block() {
    println!("\n=== E2E Test: Mine and Submit Block ===\n");

    let server = MockRpcServer::start();
    println!("Mock RPC server started on port {}", server.port);

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let rpc = BitcoinRpc::new("127.0.0.1", server.port);

        // Get block template
        let template = rpc.get_block_template("bc1qtest").await.expect("Should get template");
        println!("Got block template for height {}", template.height);

        // Create mining job from template
        let mut job = Job::new(format!("block-{}", template.height));
        job.version = template.version;

        // Parse prev_hash from hex
        let prev_hash_bytes = hex::decode(&template.prev_block_hash).unwrap_or_else(|_| vec![0; 32]);
        let mut prev_hash = [0u8; 32];
        prev_hash.copy_from_slice(&prev_hash_bytes[..32.min(prev_hash_bytes.len())]);
        job.prev_hash = prev_hash;

        job.ntime = template.timestamp as u32;
        job.nbits = u32::from_str_radix(&template.bits, 16).unwrap_or(0x207fffff);

        // Parse target from hex
        let target_bytes = hex::decode(&template.target).unwrap_or_else(|_| vec![0x7f; 32]);
        let mut target = [0u8; 32];
        for (i, b) in target_bytes.iter().take(32).enumerate() {
            target[i] = *b;
        }
        job.target = target;
        job.network_target = Some(target);

        // Set up coinbase for block construction
        job.merkle_root_override = Some([0x22; 32]);
        job.coinbase_tx = Some(create_minimal_coinbase(template.coinbase_value, template.height));
        job.transactions = Some(vec![]);

        println!("Target (first 8 bytes): {:02x?}", &target[0..8]);

        // Create coordinator and start mining
        let mut coord = Coordinator::new(4);
        coord.update_job(job.clone());
        coord.start();

        let start = Instant::now();
        let timeout = Duration::from_secs(30);
        let mut block_found = false;

        while start.elapsed() < timeout && !block_found {
            if let Some(share) = coord.try_recv_share() {
                println!("\nShare found: nonce=0x{:08x}", share.nonce);
                println!("Hash: {:02x?}", &share.hash[0..8]);

                if job.meets_network_target(&share.hash) {
                    println!("This is a valid BLOCK!");

                    // Build and submit block
                    if let Some(block_hex) = job.build_block(share.nonce) {
                        println!("Block hex length: {} bytes", block_hex.len() / 2);

                        let submit_result = rpc.submit_block(&block_hex).await;
                        match submit_result {
                            Ok(()) => {
                                println!("Block submitted successfully!");
                                block_found = true;
                            }
                            Err(e) => {
                                println!("Block submission failed: {:?}", e);
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        coord.stop();

        assert!(block_found, "Should find and submit a block within timeout");
        assert_eq!(server.get_blocks_submitted(), 1, "Should have submitted exactly 1 block");

        // Verify submitted block data
        let submitted = server.get_submitted_blocks();
        assert_eq!(submitted.len(), 1);
        let block_hex = &submitted[0];
        println!("\nSubmitted block size: {} bytes", block_hex.len() / 2);

        // Verify block header structure
        let block_bytes = hex::decode(block_hex).expect("Valid hex");
        assert!(block_bytes.len() >= 80, "Block should have at least 80 byte header");

        // Check version
        let version = u32::from_le_bytes([block_bytes[0], block_bytes[1], block_bytes[2], block_bytes[3]]);
        assert_eq!(version, 0x20000000, "Version should match");

        println!("Block structure verified!");
    });

    println!("\nTest passed!\n");
}

/// Test end-to-end: Multiple block submissions
#[test]
#[ignore]
fn e2e_mine_multiple_blocks() {
    println!("\n=== E2E Test: Mine Multiple Blocks ===\n");

    let server = MockRpcServer::start();
    println!("Mock RPC server started on port {}", server.port);

    let blocks_to_mine = 3;
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let rpc = BitcoinRpc::new("127.0.0.1", server.port);

        for block_num in 1..=blocks_to_mine {
            println!("\n--- Mining block {} of {} ---", block_num, blocks_to_mine);

            // Get fresh block template
            let template = rpc.get_block_template("bc1qtest").await.expect("Should get template");
            println!("Template height: {}", template.height);

            // Create job
            let mut job = Job::new(format!("block-{}", template.height));
            job.version = template.version;
            job.prev_hash = [0u8; 32];
            job.ntime = template.timestamp as u32;
            job.nbits = u32::from_str_radix(&template.bits, 16).unwrap_or(0x207fffff);

            // Easy target
            let mut target = [0x7f; 32];
            target[0] = 0x7f;
            target[1..32].fill(0xff);
            job.target = target;
            job.network_target = Some(target);
            job.merkle_root_override = Some([0x22; 32]);
            job.coinbase_tx = Some(create_minimal_coinbase(template.coinbase_value, template.height));
            job.transactions = Some(vec![]);

            // Mine
            let mut coord = Coordinator::new(4);
            coord.update_job(job.clone());
            coord.start();

            let start = Instant::now();
            let timeout = Duration::from_secs(30);

            while start.elapsed() < timeout {
                if let Some(share) = coord.try_recv_share() {
                    if job.meets_network_target(&share.hash) {
                        if let Some(block_hex) = job.build_block(share.nonce) {
                            let _ = rpc.submit_block(&block_hex).await;
                            println!("Block {} submitted!", block_num);
                            break;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            coord.stop();
        }

        let submitted = server.get_blocks_submitted();
        println!("\nTotal blocks submitted: {}", submitted);
        assert_eq!(submitted, blocks_to_mine as u64, "Should have mined all blocks");
    });

    println!("\nTest passed!\n");
}

/// Test the hasher directly - verifies hash computation
#[test]
fn e2e_hasher_computes_valid_hash() {
    println!("\n=== E2E Test: Hasher Computes Valid Hash ===\n");

    let mut hasher = Hasher::new();

    // Create a deterministic header
    let mut header = [0u8; 80];
    header[0..4].copy_from_slice(&0x20000000u32.to_le_bytes());
    header[4..36].copy_from_slice(&[0x00; 32]);
    header[36..68].copy_from_slice(&[0x22; 32]);
    header[68..72].copy_from_slice(&1700000000u32.to_le_bytes());
    header[72..76].copy_from_slice(&0x207fffffu32.to_le_bytes());
    header[76..80].copy_from_slice(&0u32.to_le_bytes());

    let hash = hasher.hash_header(&header);
    println!("Hash of test header: {:02x?}", &hash);

    // Same input should give same output
    let hash2 = hasher.hash_header(&header);
    assert_eq!(hash, hash2, "Hashing should be deterministic");

    // Different nonce = different hash
    header[76..80].copy_from_slice(&1u32.to_le_bytes());
    let hash3 = hasher.hash_header(&header);
    assert_ne!(hash, hash3, "Different nonce should give different hash");

    println!("Hasher verification passed!");
}

/// Test difficulty_to_target conversion
#[test]
fn e2e_difficulty_conversion() {
    println!("\n=== E2E Test: Difficulty Conversion ===\n");

    // Difficulty 1
    let t1 = difficulty_to_target(1.0);
    println!("Diff 1 target: {:02x?}", &t1[0..8]);
    assert_eq!(t1[0..4], [0x00, 0x00, 0x00, 0x00], "Should have 4 leading zeros");
    assert_eq!(t1[4], 0xff, "Byte 4 should be 0xFF");
    assert_eq!(t1[5], 0xff, "Byte 5 should be 0xFF");

    // Difficulty 2 (harder)
    let t2 = difficulty_to_target(2.0);
    println!("Diff 2 target: {:02x?}", &t2[0..8]);
    assert!(t2[4] < t1[4] || (t2[4] == t1[4] && t2[5] < t1[5]),
            "Diff 2 target should be lower than diff 1");

    // Difficulty 0.5 (easier)
    let t_half = difficulty_to_target(0.5);
    println!("Diff 0.5 target: {:02x?}", &t_half[0..8]);
    // At half difficulty, target should be larger

    println!("Difficulty conversion verified!");
}

/// Helper: Create minimal valid coinbase transaction
fn create_minimal_coinbase(value: u64, height: u64) -> Vec<u8> {
    let mut coinbase = Vec::new();

    // Version (4 bytes)
    coinbase.extend_from_slice(&1u32.to_le_bytes());

    // Input count
    coinbase.push(1);

    // Previous output (null for coinbase)
    coinbase.extend_from_slice(&[0u8; 32]); // txid
    coinbase.extend_from_slice(&0xffffffffu32.to_le_bytes()); // vout

    // Script sig (includes block height per BIP34)
    let height_bytes = height.to_le_bytes();
    let height_len = if height < 0x100 { 1 } else if height < 0x10000 { 2 } else { 3 };
    let script_len = 1 + height_len + 4; // push op + height + arbitrary data
    coinbase.push(script_len as u8);
    coinbase.push(height_len as u8); // OP_PUSH
    coinbase.extend_from_slice(&height_bytes[..height_len]);
    coinbase.extend_from_slice(b"test"); // arbitrary

    // Sequence
    coinbase.extend_from_slice(&0xffffffffu32.to_le_bytes());

    // Output count
    coinbase.push(1);

    // Output value
    coinbase.extend_from_slice(&value.to_le_bytes());

    // Output script (P2PKH dummy)
    let script = [
        0x76, 0xa9, 0x14, // OP_DUP OP_HASH160 PUSH20
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, // pubkey hash (zeros)
        0x88, 0xac, // OP_EQUALVERIFY OP_CHECKSIG
    ];
    coinbase.push(script.len() as u8);
    coinbase.extend_from_slice(&script);

    // Locktime
    coinbase.extend_from_slice(&0u32.to_le_bytes());

    coinbase
}

/// Basic test (not ignored) to verify module loads
#[test]
fn test_e2e_module_loads() {
    let job = Job::new("test".to_string());
    assert!(!job.id.is_empty());
}

// ============================================================================
// Integration Tests (Direct coordinator tests without RPC)
// ============================================================================

/// Helper to create a job with easy target for integration tests
fn create_easy_job(job_id: &str) -> Job {
    let mut job = Job::new(job_id.to_string());
    job.version = 0x20000000;
    job.prev_hash = [0x11; 32];
    job.ntime = 1700000000;
    job.nbits = 0x207fffff;

    // Very easy target - first byte 0x7F, rest 0xFF
    let mut target = [0xFF; 32];
    target[0] = 0x7F;
    job.target = target;
    job.network_target = Some(target);
    job.merkle_root_override = Some([0x22; 32]);
    job.coinbase_tx = Some(create_minimal_coinbase(312500000, 100));
    job.transactions = Some(vec![]);

    job
}

/// Integration test: Mining coordinator finds share
#[test]
#[ignore]
fn integration_coordinator_finds_share() {
    println!("\n=== Integration Test: Coordinator Finds Share ===\n");

    let mut coord = Coordinator::new(2);
    let job = create_easy_job("integration-share");

    println!("Job ID: {}", job.id);
    println!("Target (first 4 bytes): {:02x?}", &job.target[0..4]);

    coord.update_job(job.clone());
    coord.start();

    let start = Instant::now();
    let timeout = Duration::from_secs(30);
    let mut found = false;

    while start.elapsed() < timeout {
        if let Some(share) = coord.try_recv_share() {
            println!("\nShare found!");
            println!("  Nonce: 0x{:08x}", share.nonce);
            println!("  Hash: {:02x?}", &share.hash[0..4]);
            println!("  Time: {:.2}s", start.elapsed().as_secs_f64());

            assert!(job.meets_target(&share.hash), "Share should meet target");
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    coord.stop();
    assert!(found, "Should find share within timeout");
    println!("\nTest passed!\n");
}

/// Integration test: Mining coordinator can build valid block
#[test]
#[ignore]
fn integration_coordinator_builds_block() {
    println!("\n=== Integration Test: Build Valid Block ===\n");

    let mut coord = Coordinator::new(4);
    let job = create_easy_job("integration-block");

    coord.update_job(job.clone());
    coord.start();

    let start = Instant::now();
    let timeout = Duration::from_secs(30);
    let mut block_built = false;

    while start.elapsed() < timeout {
        if let Some(share) = coord.try_recv_share() {
            if job.meets_network_target(&share.hash) {
                println!("Block found! Nonce: 0x{:08x}", share.nonce);

                let block_hex = job.build_block(share.nonce);
                assert!(block_hex.is_some(), "Should build block");

                let hex = block_hex.unwrap();
                let bytes = hex::decode(&hex).expect("Valid hex");

                println!("Block size: {} bytes", bytes.len());
                assert!(bytes.len() >= 80, "Block should have header");

                // Verify header fields
                let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                assert_eq!(version, 0x20000000, "Version should match");

                let nonce = u32::from_le_bytes([bytes[76], bytes[77], bytes[78], bytes[79]]);
                assert_eq!(nonce, share.nonce, "Nonce should match");

                block_built = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    coord.stop();
    assert!(block_built, "Should build block within timeout");
    println!("\nTest passed!\n");
}

/// Integration test: Job update during mining
#[test]
#[ignore]
fn integration_job_update() {
    println!("\n=== Integration Test: Job Update ===\n");

    let mut coord = Coordinator::new(2);

    // Start with one job
    let job1 = create_easy_job("job-1");
    coord.update_job(job1);
    coord.start();

    std::thread::sleep(Duration::from_millis(200));

    // Update to new job
    let job2 = create_easy_job("job-2");
    println!("Updating to job: {}", job2.id);
    coord.update_job(job2.clone());

    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    let mut found_job2 = false;

    while start.elapsed() < timeout {
        if let Some(share) = coord.try_recv_share() {
            if share.job_id == job2.id {
                println!("Found share for job-2!");
                found_job2 = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    coord.stop();
    assert!(found_job2, "Should find share for updated job");
    println!("\nTest passed!\n");
}

/// Integration test: Pause and resume mining
#[test]
#[ignore]
fn integration_pause_resume() {
    println!("\n=== Integration Test: Pause/Resume ===\n");

    let coord = Coordinator::new(2);
    let job = create_easy_job("pause-test");

    coord.update_job(job);
    coord.start();
    assert!(coord.is_running());

    std::thread::sleep(Duration::from_millis(200));
    let hashes_before = coord.stats().get_hashes();
    println!("Hashes before pause: {}", hashes_before);

    coord.stop();
    assert!(!coord.is_running());

    std::thread::sleep(Duration::from_millis(200));
    let hashes_during = coord.stats().get_hashes();
    println!("Hashes during pause: {}", hashes_during);

    coord.start();
    std::thread::sleep(Duration::from_millis(200));
    let hashes_after = coord.stats().get_hashes();
    println!("Hashes after resume: {}", hashes_after);

    coord.stop();

    assert!(hashes_before > 0, "Should hash before pause");
    assert!(hashes_after > hashes_during, "Should hash after resume");
    println!("\nTest passed!\n");
}

// ============================================================================
// Strict Job Switch Tests (New Block Height Handling)
// ============================================================================

/// Helper to create a job simulating a specific block height
fn create_job_at_height(job_id: &str, height: u64) -> Job {
    let mut job = Job::new(job_id.to_string());
    job.version = 0x20000000;
    // Use height as part of prev_hash to make jobs distinguishable
    let mut prev_hash = [0x00; 32];
    prev_hash[0..8].copy_from_slice(&height.to_le_bytes());
    job.prev_hash = prev_hash;
    job.ntime = 1700000000 + height as u32;
    job.nbits = 0x207fffff;

    // Very easy target for fast share finding
    let mut target = [0xFF; 32];
    target[0] = 0x7F;
    job.target = target;
    job.network_target = Some(target);
    job.merkle_root_override = Some([height as u8; 32]);
    job.coinbase_tx = Some(create_minimal_coinbase(312500000, height));
    job.transactions = Some(vec![]);

    job
}

/// Integration test: Job switch latency - workers must switch within bounded time
/// This tests that when a new block height arrives, workers respond quickly
#[test]
#[ignore]
fn integration_job_switch_latency() {
    println!("\n=== Integration Test: Job Switch Latency ===\n");

    let mut coord = Coordinator::new(4);

    // Start mining at height 100
    let job1 = create_job_at_height("height-100", 100);
    println!("Starting mining at height 100");
    coord.update_job(job1.clone());
    coord.start();

    // Wait for first share to ensure mining is active
    let start = Instant::now();
    let mut shares_job1 = 0;
    while start.elapsed() < Duration::from_secs(5) {
        if let Some(share) = coord.try_recv_share() {
            if share.job_id == job1.id {
                shares_job1 += 1;
                println!("Share from job1 (height 100)");
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(shares_job1 > 0, "Should find at least one share on job1");

    // Now simulate new block at height 101
    let job2 = create_job_at_height("height-101", 101);
    println!("\n--- New block! Switching to height 101 ---");
    let switch_time = Instant::now();
    coord.update_job(job2.clone());

    // Measure time until first share from new job
    let max_switch_latency = Duration::from_millis(500); // 500ms max latency
    let mut first_job2_share_time: Option<Duration> = None;
    let mut stale_shares_after_switch = 0;

    while switch_time.elapsed() < Duration::from_secs(10) {
        if let Some(share) = coord.try_recv_share() {
            if share.job_id == job2.id {
                if first_job2_share_time.is_none() {
                    first_job2_share_time = Some(switch_time.elapsed());
                    println!(
                        "First share from job2 after {:.2}ms",
                        switch_time.elapsed().as_secs_f64() * 1000.0
                    );
                }
                break;
            } else if share.job_id == job1.id {
                // Stale share from old job
                stale_shares_after_switch += 1;
                println!(
                    "  WARNING: Stale share from job1 at {:.2}ms after switch",
                    switch_time.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    coord.stop();

    // Assertions
    let latency = first_job2_share_time.expect("Should find share on job2");
    println!("\nSwitch latency: {:.2}ms", latency.as_secs_f64() * 1000.0);
    println!("Stale shares after switch: {}", stale_shares_after_switch);

    assert!(
        latency < max_switch_latency,
        "Switch latency {:.2}ms should be < {:.2}ms",
        latency.as_secs_f64() * 1000.0,
        max_switch_latency.as_secs_f64() * 1000.0
    );

    // Allow at most 1-2 stale shares (in-flight from batch processing)
    assert!(
        stale_shares_after_switch <= 2,
        "Should have at most 2 stale shares, got {}",
        stale_shares_after_switch
    );

    println!("\nTest passed!\n");
}

/// Integration test: No stale shares after job update
/// Verifies that once workers acknowledge new job, no old job shares arrive
#[test]
#[ignore]
fn integration_no_stale_shares() {
    println!("\n=== Integration Test: No Stale Shares ===\n");

    let mut coord = Coordinator::new(4);

    // Start with job1
    let job1 = create_job_at_height("old-block", 100);
    coord.update_job(job1.clone());
    coord.start();

    // Let it mine for a bit
    std::thread::sleep(Duration::from_millis(300));

    // Drain any pending shares from job1
    while coord.try_recv_share().is_some() {}

    // Switch to job2
    let job2 = create_job_at_height("new-block", 101);
    println!("Switching to new job...");
    coord.update_job(job2.clone());

    // Small grace period for workers to receive the update
    std::thread::sleep(Duration::from_millis(50));

    // Now collect shares and verify none are from job1
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let mut job2_shares = 0;
    let mut job1_stale = 0;

    while start.elapsed() < timeout && job2_shares < 10 {
        if let Some(share) = coord.try_recv_share() {
            if share.job_id == job2.id {
                job2_shares += 1;
            } else if share.job_id == job1.id {
                job1_stale += 1;
                println!(
                    "  Stale share from job1 at {:.2}ms",
                    start.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    coord.stop();

    println!("\nJob2 shares: {}", job2_shares);
    println!("Job1 stale shares (after grace period): {}", job1_stale);

    assert!(job2_shares >= 5, "Should find multiple shares on new job");
    assert_eq!(
        job1_stale, 0,
        "Should have no stale shares after grace period"
    );

    println!("\nTest passed!\n");
}

/// Integration test: Block height change simulation
/// Simulates realistic scenario of multiple consecutive blocks
#[test]
#[ignore]
fn integration_block_height_sequence() {
    println!("\n=== Integration Test: Block Height Sequence ===\n");

    let mut coord = Coordinator::new(4);
    let start_height = 840000u64;

    // Simulate mining through multiple blocks
    let num_blocks = 5;
    let mut shares_per_height: Vec<(u64, u32)> = Vec::new();

    for i in 0..num_blocks {
        let height = start_height + i;
        let job = create_job_at_height(&format!("block-{}", height), height);

        println!("Mining at height {} (job: {})", height, job.id);
        coord.update_job(job.clone());

        if i == 0 {
            coord.start();
        }

        // Mine until we find at least one share, or timeout
        let block_start = Instant::now();
        let mut shares_found = 0;

        while block_start.elapsed() < Duration::from_secs(3) && shares_found < 2 {
            if let Some(share) = coord.try_recv_share() {
                if share.job_id == job.id {
                    shares_found += 1;
                    println!("  Share found at height {}", height);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        shares_per_height.push((height, shares_found));

        // Brief pause before next block (like real network)
        std::thread::sleep(Duration::from_millis(100));
    }

    coord.stop();

    // Print summary
    println!("\n--- Summary ---");
    let mut total_shares = 0;
    for (height, shares) in &shares_per_height {
        println!("Height {}: {} shares", height, shares);
        total_shares += shares;
    }

    // Should have found shares at most heights
    let heights_with_shares = shares_per_height.iter().filter(|(_, s)| *s > 0).count();
    println!(
        "\nFound shares at {}/{} heights",
        heights_with_shares, num_blocks
    );

    assert!(
        heights_with_shares >= (num_blocks - 1) as usize,
        "Should find shares at most block heights"
    );
    assert!(total_shares >= 5, "Should find multiple shares total");

    println!("\nTest passed!\n");
}
