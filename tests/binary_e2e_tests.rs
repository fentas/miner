//! True end-to-end tests that spawn the actual btcminer binary
//!
//! These tests:
//! 1. Start a mock Bitcoin RPC server
//! 2. Create a test config file
//! 3. Spawn the actual btcminer binary
//! 4. Verify the miner connects, mines, and submits blocks
//!
//! Run with: cargo test --release binary_e2e_ -- --nocapture --ignored
//!
//! Note: Binary must be built first with `cargo build --release`

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Mock Bitcoin RPC server for e2e testing
struct MockBitcoinRpc {
    port: u16,
    running: Arc<AtomicBool>,
    blocks_submitted: Arc<AtomicU64>,
    submitted_blocks: Arc<Mutex<Vec<String>>>,
    getblocktemplate_calls: Arc<AtomicU64>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockBitcoinRpc {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
        let port = listener.local_addr().unwrap().port();

        let running = Arc::new(AtomicBool::new(true));
        let blocks_submitted = Arc::new(AtomicU64::new(0));
        let submitted_blocks = Arc::new(Mutex::new(Vec::new()));
        let getblocktemplate_calls = Arc::new(AtomicU64::new(0));

        let running_clone = Arc::clone(&running);
        let blocks_submitted_clone = Arc::clone(&blocks_submitted);
        let submitted_blocks_clone = Arc::clone(&submitted_blocks);
        let getblocktemplate_calls_clone = Arc::clone(&getblocktemplate_calls);

        let handle = thread::spawn(move || {
            listener.set_nonblocking(true).ok();

            while running_clone.load(Ordering::Relaxed) {
                if let Ok((mut stream, addr)) = listener.accept() {
                    eprintln!("  [MockRPC] Connection from {}", addr);
                    let mut buffer = vec![0u8; 16384];
                    if let Ok(n) = stream.read(&mut buffer) {
                        if n == 0 {
                            continue;
                        }
                        let request = String::from_utf8_lossy(&buffer[..n]);
                        eprintln!("  [MockRPC] Request: {} bytes", n);

                        // Find JSON body after HTTP headers
                        if let Some(body_start) = request.find("\r\n\r\n") {
                            let body = &request[body_start + 4..];
                            if let Ok(json) = serde_json::from_str::<Value>(body) {
                                let method = json["method"].as_str().unwrap_or("");
                                let params = &json["params"];

                                let response = Self::handle_rpc(
                                    method,
                                    params,
                                    &blocks_submitted_clone,
                                    &submitted_blocks_clone,
                                    &getblocktemplate_calls_clone,
                                );

                                let response_json = json!({
                                    "result": response,
                                    "error": null,
                                    "id": json["id"]
                                });

                                let body = serde_json::to_string(&response_json).unwrap();
                                let http = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(http.as_bytes());
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(5));
            }
        });

        MockBitcoinRpc {
            port,
            running,
            blocks_submitted,
            submitted_blocks,
            getblocktemplate_calls,
            handle: Some(handle),
        }
    }

    fn handle_rpc(
        method: &str,
        params: &Value,
        blocks_submitted: &Arc<AtomicU64>,
        submitted_blocks: &Arc<Mutex<Vec<String>>>,
        getblocktemplate_calls: &Arc<AtomicU64>,
    ) -> Value {
        match method {
            "getblockchaininfo" => {
                json!({
                    "initialblockdownload": false,
                    "verificationprogress": 0.9999999,
                    "headers": 800000,
                    "blocks": 800000,
                    "bestblockhash": "0000000000000000000000000000000000000000000000000000000000000001",
                    "difficulty": 1.0
                })
            }
            "getblocktemplate" => {
                getblocktemplate_calls.fetch_add(1, Ordering::Relaxed);
                // Return very easy target so mining finds blocks quickly
                json!({
                    "version": 0x20000000_u64,
                    "previousblockhash": "0000000000000000000000000000000000000000000000000000000000000001",
                    "curtime": 1700000000_u64,
                    "bits": "207fffff",
                    "height": 800001_u64,
                    "coinbasevalue": 312500000_u64,
                    "transactions": [],
                    "target": "7fffff0000000000000000000000000000000000000000000000000000000000"
                })
            }
            "submitblock" => {
                if let Some(block_hex) = params.get(0).and_then(|v| v.as_str()) {
                    let count = blocks_submitted.fetch_add(1, Ordering::Relaxed) + 1;
                    submitted_blocks.lock().unwrap().push(block_hex.to_string());
                    eprintln!("  [MockRPC] Block #{} submitted ({} bytes)", count, block_hex.len() / 2);
                }
                Value::Null
            }
            "getbestblockhash" => {
                json!("0000000000000000000000000000000000000000000000000000000000000001")
            }
            "getnetworkinfo" => {
                json!({
                    "version": 250000,
                    "subversion": "/Satoshi:25.0.0/",
                    "connections": 8
                })
            }
            "getconnectioncount" => json!(8),
            "getdifficulty" => json!(1.0),
            "getnetworkhashps" => json!(1000000.0),
            _ => {
                eprintln!("  [MockRPC] Unknown method: {}", method);
                Value::Null
            }
        }
    }

    fn blocks_submitted(&self) -> u64 {
        self.blocks_submitted.load(Ordering::Relaxed)
    }

    fn get_submitted_blocks(&self) -> Vec<String> {
        self.submitted_blocks.lock().unwrap().clone()
    }

    fn template_requests(&self) -> u64 {
        self.getblocktemplate_calls.load(Ordering::Relaxed)
    }
}

impl Drop for MockBitcoinRpc {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Test context that manages binary, config, and cleanup
struct TestContext {
    config_path: PathBuf,
    binary_path: PathBuf,
    child: Option<Child>,
}

impl TestContext {
    fn new(rpc_port: u16, test_name: &str) -> Self {
        let test_dir = std::env::temp_dir().join(format!("btcminer_e2e_{}", test_name));
        let _ = fs::create_dir_all(&test_dir);

        let config_path = test_dir.join("btcminer.toml");

        // Create test config - use "none" preset for solo mining via external bitcoind
        let config = format!(
            r#"
[pool]
preset = "none"
payout_address = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"

[mining]
threads = 2
batch_size = 65536

[bitcoind]
enabled = true
managed = false
host = "127.0.0.1"
rpc_port = {port}
rpc_user = "test"
rpc_password = "test"

[web]
enabled = false

[idle]
enabled = false
"#,
            port = rpc_port
        );

        fs::write(&config_path, config).expect("Failed to write config");

        // Find binary
        let binary_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("btcminer");

        TestContext {
            config_path,
            binary_path,
            child: None,
        }
    }

    fn start_miner(&mut self) -> Result<(), String> {
        if !self.binary_path.exists() {
            return Err(format!(
                "Binary not found at {:?}. Run `cargo build --release` first.",
                self.binary_path
            ));
        }

        let child = Command::new(&self.binary_path)
            .arg("--config")
            .arg(&self.config_path)
            .arg("--no-hud")
            .arg("--debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn miner: {}", e))?;

        self.child = Some(child);
        Ok(())
    }

    fn stop_miner(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Try graceful shutdown first via kill command on Unix
            #[cfg(unix)]
            {
                let pid = child.id();
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .output();
            }

            // Wait with timeout for graceful shutdown
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(3) {
                if let Ok(Some(_)) = child.try_wait() {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn read_output(&mut self) -> (String, String) {
        let mut stdout = String::new();
        let mut stderr = String::new();

        if let Some(ref mut child) = self.child {
            if let Some(ref mut out) = child.stdout {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(ref mut err) = child.stderr {
                let _ = err.read_to_string(&mut stderr);
            }
        }

        (stdout, stderr)
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        self.stop_miner();
        // Clean up config file
        let _ = fs::remove_file(&self.config_path);
        if let Some(parent) = self.config_path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

/// Binary E2E: Miner starts and connects to RPC
#[test]
#[ignore]
fn binary_e2e_miner_starts_and_connects() {
    println!("\n=== Binary E2E: Miner Starts and Connects ===\n");

    let rpc = MockBitcoinRpc::start();
    println!("Mock RPC started on port {}", rpc.port);

    let mut ctx = TestContext::new(rpc.port, "starts_connects");
    println!("Config path: {:?}", ctx.config_path);

    println!("Starting miner binary...");
    if let Err(e) = ctx.start_miner() {
        println!("SKIP: {}", e);
        return;
    }

    // Wait for miner to connect and request block template
    let start = Instant::now();
    let timeout = Duration::from_secs(15);

    while start.elapsed() < timeout {
        if rpc.template_requests() > 0 {
            println!("Miner connected and requested block template!");
            break;
        }

        // Check if process is still running
        if let Some(ref mut child) = ctx.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    println!("Miner exited early with status: {}", status);
                    let (stdout, stderr) = ctx.read_output();
                    if !stdout.is_empty() {
                        println!("STDOUT:\n{}", stdout);
                    }
                    if !stderr.is_empty() {
                        println!("STDERR:\n{}", stderr);
                    }
                    break;
                }
                Ok(None) => {} // Still running
                Err(e) => println!("Error checking process: {}", e),
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    // Print output on failure
    if rpc.template_requests() == 0 {
        println!("\nMiner output:");
        let (stdout, stderr) = ctx.read_output();
        if !stdout.is_empty() {
            println!("STDOUT:\n{}", stdout);
        }
        if !stderr.is_empty() {
            println!("STDERR:\n{}", stderr);
        }
    }

    ctx.stop_miner();

    assert!(
        rpc.template_requests() > 0,
        "Miner should request at least one block template"
    );

    println!("\nTest passed!\n");
}

/// Binary E2E: Miner finds and submits block
#[test]
#[ignore]
fn binary_e2e_miner_submits_block() {
    println!("\n=== Binary E2E: Miner Submits Block ===\n");

    let rpc = MockBitcoinRpc::start();
    println!("Mock RPC started on port {}", rpc.port);

    let mut ctx = TestContext::new(rpc.port, "submits_block");

    println!("Starting miner binary...");
    if let Err(e) = ctx.start_miner() {
        println!("SKIP: {}", e);
        return;
    }

    // Wait for block submission
    let start = Instant::now();
    let timeout = Duration::from_secs(60);

    println!("Waiting for block submission...");
    while start.elapsed() < timeout {
        let blocks = rpc.blocks_submitted();
        if blocks > 0 {
            println!("Block submitted after {:.1}s!", start.elapsed().as_secs_f64());
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    ctx.stop_miner();

    let blocks = rpc.blocks_submitted();
    println!("Total blocks submitted: {}", blocks);

    assert!(blocks > 0, "Miner should submit at least one block");

    // Verify block structure
    let submitted = rpc.get_submitted_blocks();
    if let Some(block_hex) = submitted.first() {
        let bytes = hex::decode(block_hex).expect("Valid hex");
        println!("Block size: {} bytes", bytes.len());

        assert!(bytes.len() >= 80, "Block should have header (80+ bytes)");

        // Check version
        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        println!("Block version: 0x{:08x}", version);
        assert_eq!(version, 0x20000000, "Version should match template");
    }

    println!("\nTest passed!\n");
}

/// Binary E2E: Miner mines multiple blocks
#[test]
#[ignore]
fn binary_e2e_miner_mines_multiple_blocks() {
    println!("\n=== Binary E2E: Miner Mines Multiple Blocks ===\n");

    let rpc = MockBitcoinRpc::start();
    println!("Mock RPC started on port {}", rpc.port);

    let mut ctx = TestContext::new(rpc.port, "multiple_blocks");

    println!("Starting miner binary...");
    if let Err(e) = ctx.start_miner() {
        println!("SKIP: {}", e);
        return;
    }

    let target_blocks = 3;
    let start = Instant::now();
    let timeout = Duration::from_secs(120);

    println!("Waiting for {} blocks...", target_blocks);
    while start.elapsed() < timeout {
        let blocks = rpc.blocks_submitted();
        println!(
            "  Blocks: {}/{} (elapsed: {:.1}s)",
            blocks,
            target_blocks,
            start.elapsed().as_secs_f64()
        );

        if blocks >= target_blocks {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    ctx.stop_miner();

    let blocks = rpc.blocks_submitted();
    println!("\nFinal block count: {}", blocks);

    assert!(
        blocks >= target_blocks,
        "Should mine at least {} blocks, got {}",
        target_blocks,
        blocks
    );

    println!("\nTest passed!\n");
}

/// Binary E2E: Verify block header fields are correct
#[test]
#[ignore]
fn binary_e2e_verify_block_structure() {
    println!("\n=== Binary E2E: Verify Block Structure ===\n");

    let rpc = MockBitcoinRpc::start();
    println!("Mock RPC started on port {}", rpc.port);

    let mut ctx = TestContext::new(rpc.port, "block_structure");

    println!("Starting miner binary...");
    if let Err(e) = ctx.start_miner() {
        println!("SKIP: {}", e);
        return;
    }

    // Wait for one block
    let start = Instant::now();
    let timeout = Duration::from_secs(60);

    while start.elapsed() < timeout && rpc.blocks_submitted() == 0 {
        thread::sleep(Duration::from_millis(200));
    }

    ctx.stop_miner();

    let submitted = rpc.get_submitted_blocks();
    assert!(!submitted.is_empty(), "Should have at least one block");

    let block_hex = &submitted[0];
    let bytes = hex::decode(block_hex).expect("Valid hex");

    println!("Analyzing block structure...");
    println!("  Total size: {} bytes", bytes.len());

    // Header is 80 bytes
    assert!(bytes.len() >= 80, "Block must have 80-byte header");

    // Parse header fields
    let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let prev_hash = &bytes[4..36];
    let merkle_root = &bytes[36..68];
    let timestamp = u32::from_le_bytes([bytes[68], bytes[69], bytes[70], bytes[71]]);
    let bits = u32::from_le_bytes([bytes[72], bytes[73], bytes[74], bytes[75]]);
    let nonce = u32::from_le_bytes([bytes[76], bytes[77], bytes[78], bytes[79]]);

    println!("  Version: 0x{:08x}", version);
    println!("  Prev hash: {:02x?}...", &prev_hash[0..4]);
    println!("  Merkle root: {:02x?}...", &merkle_root[0..4]);
    println!("  Timestamp: {}", timestamp);
    println!("  Bits: 0x{:08x}", bits);
    println!("  Nonce: 0x{:08x}", nonce);

    // Verify expected values from template
    assert_eq!(version, 0x20000000, "Version should match template");
    assert_eq!(bits, 0x207fffff, "Bits should match template");
    assert!(timestamp > 0, "Timestamp should be set");
    // Note: nonce can be 0 if that's what was found by mining

    // Check transaction count (varint after header)
    let tx_count = bytes[80];
    println!("  Transaction count: {}", tx_count);
    assert!(tx_count >= 1, "Should have at least coinbase transaction");

    println!("\nBlock structure verified!");
    println!("\nTest passed!\n");
}

/// Basic test to verify test infrastructure works
#[test]
fn binary_e2e_mock_rpc_works() {
    let rpc = MockBitcoinRpc::start();
    assert!(rpc.port > 0);
    assert_eq!(rpc.blocks_submitted(), 0);
    println!("Mock RPC infrastructure works on port {}", rpc.port);
}
