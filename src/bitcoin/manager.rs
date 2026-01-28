use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::{BitcoinError, BitcoinRpc, SyncStatus};

const BITCOIN_CORE_VERSION: &str = "30.2";
const DOWNLOAD_BASE_URL: &str = "https://bitcoincore.org/bin";

// AssumeUTXO snapshot info (Bitcoin Core v28+)
const ASSUMEUTXO_HEIGHT: u64 = 840000;
const ASSUMEUTXO_HASH: &str = "0000000000000000000320283a032748cef8227873ff4872689bf23f1cda83a5";
const ASSUMEUTXO_URL: &str = "https://bitcoincore.org/bin/utxo-snapshots/utxo-840000.dat";

/// Manages Bitcoin Core installation and lifecycle
pub struct BitcoindManager {
    data_dir: PathBuf,
    install_dir: PathBuf,
    rpc: Option<BitcoinRpc>,
    process: Option<Child>,
    prune_size: u32,
    dbcache: u32,
    assumeutxo: bool,
    rpc_user: String,
    rpc_password: String,
    rpc_port: u16,
}

impl BitcoindManager {
    /// Create a new manager with default directories
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        // Auto-calculate dbcache based on available RAM
        // Use ~50% of free memory, capped between 450MB and 16GB
        let dbcache = Self::calculate_dbcache();

        Self {
            data_dir: home.join(".bitcoin"),
            install_dir: home.join(".local/share/bitcoin-core"),
            rpc: None,
            process: None,
            prune_size: 10000,
            dbcache,
            assumeutxo: false,
            rpc_user: generate_random_string(16),
            rpc_password: generate_random_string(32),
            rpc_port: 8332,
        }
    }

    /// Calculate optimal dbcache size based on available system memory
    fn calculate_dbcache() -> u32 {
        use sysinfo::System;

        let mut sys = System::new_all();
        sys.refresh_memory();

        let total_mem_mb = sys.total_memory() / (1024 * 1024);
        let available_mem_mb = sys.available_memory() / (1024 * 1024);

        // Use 50% of available memory or 25% of total, whichever is smaller
        // This leaves room for the OS and mining
        let target = ((available_mem_mb / 2) as u32).min((total_mem_mb / 4) as u32);

        // Clamp between 450 MB (default) and 16384 MB (16 GB)
        target.clamp(450, 16384)
    }

    /// Set custom data directory
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }

    /// Set prune size in MB
    pub fn with_prune(mut self, size: u32) -> Self {
        self.prune_size = size;
        self
    }

    /// Set dbcache size in MB (0 = auto-calculate)
    pub fn with_dbcache(mut self, size: u32) -> Self {
        if size == 0 {
            self.dbcache = Self::calculate_dbcache();
        } else {
            self.dbcache = size.clamp(450, 16384);
        }
        self
    }

    /// Enable AssumeUTXO for fast initial sync
    pub fn with_assumeutxo(mut self, enabled: bool) -> Self {
        self.assumeutxo = enabled;
        self
    }

    /// Set RPC credentials
    pub fn with_rpc_auth(mut self, user: &str, password: &str) -> Self {
        self.rpc_user = user.to_string();
        self.rpc_password = password.to_string();
        self
    }

    /// Set RPC port
    pub fn with_rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = port;
        self
    }

    /// Check if Bitcoin Core is installed
    pub fn is_installed(&self) -> bool {
        self.bitcoind_path().exists()
    }

    /// Get installed Bitcoin Core version (if any)
    pub fn installed_version(&self) -> Option<String> {
        if !self.is_installed() {
            return None;
        }

        // Try to get version from bitcoind -version
        let output = Command::new(self.bitcoind_path())
            .arg("-version")
            .output()
            .ok()?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        // Parse "Bitcoin Core version v30.2.0" -> "30.2"
        for line in version_str.lines() {
            if line.contains("Bitcoin Core version") {
                if let Some(v) = line.split('v').nth(1) {
                    // Take major.minor, ignore patch
                    let parts: Vec<&str> = v.trim().split('.').collect();
                    if parts.len() >= 2 {
                        return Some(format!("{}.{}", parts[0], parts[1]));
                    }
                }
            }
        }
        None
    }

    /// Check if upgrade is needed
    pub fn needs_upgrade(&self) -> bool {
        if let Some(installed) = self.installed_version() {
            installed != BITCOIN_CORE_VERSION
        } else {
            false
        }
    }

    /// Get path to bitcoind binary
    fn bitcoind_path(&self) -> PathBuf {
        self.install_dir.join("bin/bitcoind")
    }

    /// Download and install Bitcoin Core
    pub async fn download_and_install(&self) -> Result<(), BitcoinError> {
        info!("Downloading Bitcoin Core v{}...", BITCOIN_CORE_VERSION);

        let platform = get_platform();
        let filename = format!("bitcoin-{}-{}.tar.gz", BITCOIN_CORE_VERSION, platform);
        let url = format!("{}/bitcoin-core-{}/{}", DOWNLOAD_BASE_URL, BITCOIN_CORE_VERSION, filename);

        // Create install directory
        std::fs::create_dir_all(&self.install_dir)?;

        // Download
        let response = reqwest::get(&url)
            .await
            .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(BitcoinError::DownloadError(format!(
                "HTTP {}: {}",
                response.status(),
                url
            )));
        }

        let bytes = response.bytes()
            .await
            .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

        // Save to temp file
        let temp_path = self.install_dir.join(&filename);
        std::fs::write(&temp_path, &bytes)?;

        // Extract
        info!("Extracting Bitcoin Core...");
        let status = Command::new("tar")
            .args(["-xzf", &filename, "--strip-components=1"])
            .current_dir(&self.install_dir)
            .status()?;

        if !status.success() {
            return Err(BitcoinError::DownloadError("Failed to extract archive".into()));
        }

        // Cleanup
        std::fs::remove_file(&temp_path)?;

        info!("Bitcoin Core v{} installed successfully", BITCOIN_CORE_VERSION);
        Ok(())
    }

    /// Read existing bitcoin.conf credentials if present
    fn read_existing_config(&mut self) -> bool {
        let config_path = self.data_dir.join("bitcoin.conf");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut found_user = false;
            let mut found_pass = false;

            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("rpcuser=") {
                    self.rpc_user = line.trim_start_matches("rpcuser=").to_string();
                    found_user = true;
                } else if line.starts_with("rpcpassword=") {
                    self.rpc_password = line.trim_start_matches("rpcpassword=").to_string();
                    found_pass = true;
                } else if line.starts_with("rpcport=") {
                    if let Ok(port) = line.trim_start_matches("rpcport=").parse() {
                        self.rpc_port = port;
                    }
                }
            }

            if found_user && found_pass {
                info!("Using existing bitcoin.conf credentials");
                return true;
            }
        }
        false
    }

    /// Write bitcoin.conf with optimal settings (only if needed)
    pub fn write_config(&mut self) -> Result<(), BitcoinError> {
        std::fs::create_dir_all(&self.data_dir)?;

        let config_path = self.data_dir.join("bitcoin.conf");

        // Check if config already exists with valid credentials
        if self.read_existing_config() {
            // Config exists, just ensure RPC client uses correct credentials
            return Ok(());
        }

        // Generate new config
        let config = format!(
r#"# Bitcoin Core configuration
# Generated by btcminer

# Network
server=1
txindex=0
prune={}

# RPC
rpcuser={}
rpcpassword={}
rpcport={}
rpcallowip=127.0.0.1
rpcbind=127.0.0.1

# Performance (dbcache auto-calculated based on available RAM)
dbcache={}
maxmempool=300
mempoolexpiry=72

# Mining
blockmaxweight=4000000
"#,
            self.prune_size,
            self.rpc_user,
            self.rpc_password,
            self.rpc_port,
            self.dbcache
        );

        std::fs::write(&config_path, config)?;

        info!("Wrote bitcoin.conf to {:?}", config_path);
        Ok(())
    }

    /// Start Bitcoin Core daemon
    pub fn start(&mut self) -> Result<(), BitcoinError> {
        if self.process.is_some() {
            warn!("Bitcoin Core is already running");
            return Ok(());
        }

        // Check if bitcoind binary exists
        let bitcoind = self.bitcoind_path();
        if !bitcoind.exists() {
            return Err(BitcoinError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("bitcoind not found at {:?}", bitcoind)
            )));
        }

        info!("Starting Bitcoin Core from {:?}...", bitcoind);

        let child = Command::new(&bitcoind)
            .arg(format!("-datadir={}", self.data_dir.display()))
            .arg("-daemon=0")
            .arg("-printtoconsole=0")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                BitcoinError::IoError(std::io::Error::new(
                    e.kind(),
                    format!("Failed to spawn bitcoind: {}", e)
                ))
            })?;

        self.process = Some(child);

        // Initialize RPC client
        self.rpc = Some(
            BitcoinRpc::new("127.0.0.1", self.rpc_port)
                .with_auth(&self.rpc_user, &self.rpc_password)
        );

        Ok(())
    }

    /// Stop Bitcoin Core daemon
    pub fn stop(&mut self) -> Result<(), BitcoinError> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping Bitcoin Core...");

            // Try graceful shutdown via RPC first
            if self.rpc.is_some() {
                // Note: Graceful shutdown would use RPC stop command
                // For now, just kill the process
            }

            // Kill process
            let _ = process.kill();
            let _ = process.wait();

            info!("Bitcoin Core stopped");
        }

        self.rpc = None;
        Ok(())
    }

    /// Wait for Bitcoin Core to be ready and synced
    /// Shows sync progress while waiting
    pub async fn wait_ready(&self, timeout_secs: u64) -> Result<(), BitcoinError> {
        let rpc = self.rpc.as_ref()
            .ok_or_else(|| BitcoinError::ConnectionError("Not started".into()))?;

        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
        let mut connected = false;
        let mut last_progress_time = std::time::Instant::now();

        while std::time::Instant::now() < deadline {
            match rpc.get_blockchain_info().await {
                Ok(status) => {
                    if !connected {
                        info!("Bitcoin Core responding, waiting for sync...");
                        connected = true;
                    }

                    if status.synced {
                        info!("Bitcoin Core synced at height {}", status.blocks);
                        return Ok(());
                    }

                    // Show progress every 5 seconds
                    if last_progress_time.elapsed() >= Duration::from_secs(5) {
                        info!(
                            "Syncing: {:.2}% ({}/{} blocks)",
                            status.progress,
                            status.blocks,
                            status.headers
                        );
                        last_progress_time = std::time::Instant::now();
                    }
                }
                Err(_) if !connected => {
                    // Still waiting for Bitcoin Core to start accepting connections
                }
                Err(e) => {
                    warn!("RPC error during sync: {}", e);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }

        if !connected {
            Err(BitcoinError::ConnectionError("Timeout waiting for Bitcoin Core to start".into()))
        } else {
            // Connected but not synced - return NotSynced error instead of connection error
            if let Ok(status) = rpc.get_blockchain_info().await {
                Err(BitcoinError::NotSynced { progress: status.progress })
            } else {
                Err(BitcoinError::ConnectionError("Lost connection to Bitcoin Core".into()))
            }
        }
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> Result<SyncStatus, BitcoinError> {
        let rpc = self.rpc.as_ref()
            .ok_or_else(|| BitcoinError::ConnectionError("Not started".into()))?;

        rpc.get_blockchain_info().await
    }

    /// Get RPC client reference
    pub fn rpc(&self) -> Option<&BitcoinRpc> {
        self.rpc.as_ref()
    }

    /// Check if AssumeUTXO is enabled
    pub fn assumeutxo_enabled(&self) -> bool {
        self.assumeutxo
    }

    /// Download AssumeUTXO snapshot if not already present
    pub async fn download_assumeutxo(&self) -> Result<PathBuf, BitcoinError> {
        let snapshot_path = self.data_dir.join("utxo-snapshot.dat");

        if snapshot_path.exists() {
            info!("AssumeUTXO snapshot already downloaded");
            return Ok(snapshot_path);
        }

        info!("Downloading AssumeUTXO snapshot from Bitcoin Core...");
        info!("This is a ~7GB file and may take a while");

        let client = reqwest::Client::new();
        let response = client.get(ASSUMEUTXO_URL)
            .send()
            .await
            .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(BitcoinError::DownloadError(
                format!("HTTP {}: Failed to download snapshot", response.status())
            ));
        }

        let bytes = response.bytes()
            .await
            .map_err(|e| BitcoinError::DownloadError(e.to_string()))?;

        std::fs::write(&snapshot_path, &bytes)?;
        info!("AssumeUTXO snapshot downloaded to {:?}", snapshot_path);

        Ok(snapshot_path)
    }

    /// Load AssumeUTXO snapshot via RPC (requires Bitcoin Core to be running)
    pub async fn load_assumeutxo(&self) -> Result<(), BitcoinError> {
        let rpc = self.rpc.as_ref()
            .ok_or_else(|| BitcoinError::ConnectionError("Bitcoin Core not running".into()))?;

        let snapshot_path = self.data_dir.join("utxo-snapshot.dat");
        if !snapshot_path.exists() {
            return Err(BitcoinError::NotSynced { progress: 0.0 });
        }

        info!("Loading AssumeUTXO snapshot (height {})...", ASSUMEUTXO_HEIGHT);

        // Call loadtxoutset RPC
        rpc.load_utxo_snapshot(&snapshot_path).await?;

        info!("AssumeUTXO snapshot loaded successfully!");
        info!("Node will sync remaining blocks in background");

        Ok(())
    }
}

impl Default for BitcoindManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BitcoindManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Get platform string for Bitcoin Core download
fn get_platform() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "x86_64-linux-gnu";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "aarch64-linux-gnu";

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "x86_64-apple-darwin";

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "arm64-apple-darwin";

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "x86_64-w64-mingw32";

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    compile_error!("Unsupported platform for Bitcoin Core");
}

/// Generate a random alphanumeric string
fn generate_random_string(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
        .chars()
        .collect();

    let mut result = String::with_capacity(len);
    let mut state = seed;

    for _ in 0..len {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let idx = ((state >> 33) as usize) % chars.len();
        result.push(chars[idx]);
    }

    result
}
