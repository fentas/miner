use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::{BitcoinError, BitcoinRpc, LoadProgress, SyncStatus};

const BITCOIN_CORE_VERSION: &str = "30.2";
const DOWNLOAD_BASE_URL: &str = "https://bitcoincore.org/bin";

// AssumeUTXO snapshot info (Bitcoin Core v30+)
// The hash is verified by Bitcoin Core when loading via loadtxoutset RPC.
// Using torrent for trustless download - verified by infohash + Bitcoin Core hash check.
// Alternative: generate with bitcoin-cli dumptxoutset utxo-910000.dat rollback=910000
// PR: https://github.com/bitcoin/bitcoin/pull/33274
const ASSUMEUTXO_HEIGHT: u64 = 910000;
const ASSUMEUTXO_MAGNET: &str = "magnet:?xt=urn:btih:7019437a2b1530624b100c0795cfc5f90b8322ca&dn=utxo-910000.dat&tr=udp://tracker.openbittorrent.com:80&tr=udp://tracker.bitcoin.sprovoost.nl:6969";

/// Manages Bitcoin Core installation and lifecycle
pub struct BitcoindManager {
    data_dir: PathBuf,
    install_dir: PathBuf,
    rpc: Option<BitcoinRpc>,
    process: Option<Child>,
    prune_size: u32,
    dbcache: u32,
    assumeutxo: bool,
    assumeutxo_magnet: Option<String>,
    assumeutxo_torrent: Option<PathBuf>,
    rpc_user: String,
    rpc_password: String,
    rpc_port: u16,
}

impl BitcoindManager {
    /// Create a new manager with default directories
    pub fn new() -> Self {
        // Auto-calculate dbcache based on available RAM
        // Use ~50% of free memory, capped between 450MB and 16GB
        let dbcache = Self::calculate_dbcache();

        // Platform-specific default directories
        #[cfg(target_os = "windows")]
        let (data_dir, install_dir) = {
            let appdata = std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
            // On Windows, Bitcoin Core uses "Bitcoin Core" as default data directory
            let bitcoin_core_dir = appdata.join("Bitcoin Core");
            (
                bitcoin_core_dir.clone(),
                bitcoin_core_dir,
            )
        };

        #[cfg(not(target_os = "windows"))]
        let (data_dir, install_dir) = {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            (
                home.join(".bitcoin"),
                home.join(".local/share/bitcoin-core"),
            )
        };

        Self {
            data_dir,
            install_dir,
            rpc: None,
            process: None,
            prune_size: 10000,
            dbcache,
            assumeutxo: false,
            assumeutxo_magnet: None,
            assumeutxo_torrent: None,
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

    /// Set custom magnet URI for AssumeUTXO snapshot
    pub fn with_assumeutxo_magnet(mut self, magnet: Option<String>) -> Self {
        self.assumeutxo_magnet = magnet;
        self
    }

    /// Set torrent file path for AssumeUTXO snapshot (alternative to magnet)
    pub fn with_assumeutxo_torrent(mut self, path: Option<PathBuf>) -> Self {
        self.assumeutxo_torrent = path;
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
        #[cfg(target_os = "windows")]
        return self.install_dir.join("bin").join("bitcoind.exe");

        #[cfg(not(target_os = "windows"))]
        return self.install_dir.join("bin").join("bitcoind");
    }

    /// Download and install Bitcoin Core
    pub async fn download_and_install(&self) -> Result<(), BitcoinError> {
        info!("Downloading Bitcoin Core v{}...", BITCOIN_CORE_VERSION);

        let platform = get_platform();

        // Windows uses .zip, others use .tar.gz
        #[cfg(target_os = "windows")]
        let filename = format!("bitcoin-{}-{}.zip", BITCOIN_CORE_VERSION, platform);
        #[cfg(not(target_os = "windows"))]
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

        // Extract - different method per platform
        info!("Extracting Bitcoin Core...");

        #[cfg(target_os = "windows")]
        {
            // Use PowerShell to extract zip on Windows
            let status = Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &format!(
                        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force; \
                         Get-ChildItem -Path '{}' -Directory | ForEach-Object {{ \
                             Get-ChildItem -Path $_.FullName | Move-Item -Destination '{}' -Force \
                         }}",
                        temp_path.display(),
                        self.install_dir.display(),
                        self.install_dir.display(),
                        self.install_dir.display()
                    )
                ])
                .status()?;

            if !status.success() {
                return Err(BitcoinError::DownloadError("Failed to extract archive".into()));
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let status = Command::new("tar")
                .args(["-xzf", &filename, "--strip-components=1"])
                .current_dir(&self.install_dir)
                .status()?;

            if !status.success() {
                return Err(BitcoinError::DownloadError("Failed to extract archive".into()));
            }
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

    /// Get data directory path
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    /// Get debug.log path
    pub fn debug_log_path(&self) -> PathBuf {
        self.data_dir.join("debug.log")
    }

    /// Check if AssumeUTXO snapshot exists and download is complete
    /// Uses a .complete marker file to track successful torrent completion
    /// Note: We don't use size-based detection as truncated files would pass but fail to load
    pub fn has_assumeutxo_snapshot(&self) -> bool {
        let path = self.assumeutxo_snapshot_path();
        let marker = self.data_dir.join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));

        // Both the snapshot and the completion marker must exist
        // The marker is only created after torrent piece verification completes
        path.exists() && marker.exists()
    }

    /// Get AssumeUTXO snapshot path
    pub fn assumeutxo_snapshot_path(&self) -> PathBuf {
        self.data_dir.join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT))
    }

    /// Download AssumeUTXO snapshot via BitTorrent with progress callback
    /// Callback receives (bytes_downloaded, total_bytes)
    /// Uses torrent for trustless download - data is verified by infohash
    /// If a torrent file path is configured, uses it directly (enables piece verification)
    pub async fn download_assumeutxo_with_progress<F>(
        &self,
        progress_callback: F,
    ) -> Result<PathBuf, BitcoinError>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        // Prefer torrent file if configured (enables proper piece verification on resume)
        if let Some(ref torrent_path) = self.assumeutxo_torrent {
            if torrent_path.exists() {
                info!("Using configured torrent file: {:?}", torrent_path);
                return self.download_assumeutxo_torrent_with_progress(torrent_path, progress_callback).await;
            } else {
                warn!("Configured torrent file not found: {:?}", torrent_path);
            }
        }

        let magnet = self.assumeutxo_magnet.as_deref().unwrap_or(ASSUMEUTXO_MAGNET);
        self.download_assumeutxo_magnet_with_progress(magnet, progress_callback).await
    }

    /// Download AssumeUTXO snapshot from a torrent file
    /// This method provides proper piece verification for existing partial files
    pub async fn download_assumeutxo_torrent_with_progress<F>(
        &self,
        torrent_path: &std::path::Path,
        progress_callback: F,
    ) -> Result<PathBuf, BitcoinError>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        use gosh_dl::{DownloadEngine, EngineConfig, DownloadOptions, DownloadEvent};

        let snapshot_path = self.assumeutxo_snapshot_path();

        // Ensure data directory exists
        std::fs::create_dir_all(&self.data_dir)?;

        // Read torrent file
        let torrent_data = std::fs::read(torrent_path)
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to read torrent file: {}", e)))?;

        // Validate it's a real torrent file
        if torrent_data.first() != Some(&b'd') {
            return Err(BitcoinError::DownloadError("Invalid torrent file format".into()));
        }

        info!("Downloading AssumeUTXO snapshot (height {}) from torrent file...", ASSUMEUTXO_HEIGHT);

        // Configure download engine
        let config = EngineConfig {
            download_dir: self.data_dir.clone(),
            database_path: Some(self.data_dir.join("gosh-dl.db")),
            ..Default::default()
        };

        let engine = DownloadEngine::new(config)
            .await
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to create download engine: {}", e)))?;

        // Start download with torrent file (enables piece verification)
        let opts = DownloadOptions::default();
        let download_id = engine.add_torrent(&torrent_data, opts)
            .await
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to add torrent: {}", e)))?;

        // Subscribe to events for progress
        let mut events = engine.subscribe();

        // Get current status
        let status = engine.status(download_id);
        let mut last_downloaded: u64 = status.as_ref().map(|s| s.progress.completed_size).unwrap_or(0);
        let mut total_size: u64 = status.as_ref()
            .and_then(|s| s.progress.total_size)
            .unwrap_or(9_772_098_907);

        progress_callback(last_downloaded, total_size);

        let mut last_status_check = std::time::Instant::now();
        loop {
            // Check status periodically (every 1s) regardless of events
            if last_status_check.elapsed() >= Duration::from_secs(1) {
                last_status_check = std::time::Instant::now();
                if let Some(status) = engine.status(download_id) {
                    match status.state {
                        gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed => {
                            info!("AssumeUTXO snapshot download complete");
                            break;
                        }
                        _ => {
                            // Check if all bytes downloaded (gosh-dl may stay in Downloading state)
                            if let Some(total) = status.progress.total_size {
                                if status.progress.completed_size >= total && total > 0 {
                                    info!("AssumeUTXO snapshot download complete (100% verified)");
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            match tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
                Ok(Ok(event)) => {
                    match event {
                        DownloadEvent::Progress { id, progress } if id == download_id => {
                            if let Some(total) = progress.total_size {
                                if total != total_size {
                                    total_size = total;
                                }
                            }
                            if progress.completed_size != last_downloaded {
                                last_downloaded = progress.completed_size;
                                progress_callback(last_downloaded, total_size);
                            }
                        }
                        DownloadEvent::StateChanged { id, new_state, .. } if id == download_id => {
                            match new_state {
                                gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed => {
                                    info!("AssumeUTXO snapshot download complete");
                                    break;
                                }
                                _ => {}
                            }
                        }
                        DownloadEvent::Completed { id } if id == download_id => {
                            info!("AssumeUTXO snapshot download complete");
                            break;
                        }
                        DownloadEvent::Failed { id, error, .. } if id == download_id => {
                            return Err(BitcoinError::DownloadError(format!("Torrent download failed: {}", error)));
                        }
                        _ => {}
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => {} // Timeout - continue loop, status check happens at start
            }
        }

        if !snapshot_path.exists() {
            return Err(BitcoinError::DownloadError("Download completed but file not found".into()));
        }

        // Create completion marker
        let marker = self.data_dir.join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));
        if let Err(e) = std::fs::write(&marker, "complete") {
            warn!("Failed to create completion marker: {}", e);
        }

        info!("AssumeUTXO snapshot downloaded and verified: {:?}", snapshot_path);
        Ok(snapshot_path)
    }

    /// Download AssumeUTXO snapshot from custom magnet URI
    /// Uses .torrent file for proper piece verification on resume
    pub async fn download_assumeutxo_magnet_with_progress<F>(
        &self,
        magnet_uri: &str,
        progress_callback: F,
    ) -> Result<PathBuf, BitcoinError>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        use gosh_dl::{DownloadEngine, EngineConfig, DownloadEvent};
        use gosh_dl::torrent::MagnetUri;

        let snapshot_path = self.assumeutxo_snapshot_path();

        // Ensure data directory exists
        std::fs::create_dir_all(&self.data_dir)?;

        // Parse magnet to extract info hash
        let magnet = MagnetUri::parse(magnet_uri)
            .map_err(|e| BitcoinError::DownloadError(format!("Invalid magnet URI: {}", e)))?;
        let info_hash = magnet.info_hash_hex();

        info!("Downloading AssumeUTXO snapshot (height {}) via BitTorrent...", ASSUMEUTXO_HEIGHT);
        info!("Info hash: {}", info_hash);

        // Configure download engine with persistent session state
        let config = EngineConfig {
            download_dir: self.data_dir.clone(),
            database_path: Some(self.data_dir.join("gosh-dl.db")),
            ..Default::default()
        };

        let engine = DownloadEngine::new(config)
            .await
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to create download engine: {}", e)))?;

        // Check if we have a cached .torrent file (for proper piece verification on resume)
        let torrent_cache_path = self.data_dir.join(format!("{}.torrent", info_hash));
        let torrent_data = if torrent_cache_path.exists() {
            let data = std::fs::read(&torrent_cache_path)?;
            // Validate it's a real torrent file (starts with 'd' for bencode dict)
            if data.first() == Some(&b'd') {
                info!("Using cached torrent file for piece verification");
                Some(data)
            } else {
                warn!("Cached torrent file is invalid, removing");
                let _ = std::fs::remove_file(&torrent_cache_path);
                None
            }
        } else {
            None
        };

        // Note: itorrents.org fallback removed - magnet links now verify existing files
        // thanks to https://github.com/goshitsarch-eng/gosh-dl/pull/9

        // Check if a download already exists for this torrent
        let existing_download = engine.list().into_iter().find(|d| {
            if let Some(ref ih) = d.metadata.info_hash {
                return ih.to_lowercase() == info_hash.to_lowercase();
            }
            false
        });

        let download_id = if let Some(existing) = existing_download {
            match existing.state {
                gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed => {
                    info!("Download already complete");
                    let marker_path = self.data_dir.join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));
                    std::fs::write(&marker_path, "complete")?;
                    return Ok(self.assumeutxo_snapshot_path());
                }
                gosh_dl::DownloadState::Downloading => {
                    info!("Resuming existing download");
                    existing.id
                }
                _ => {
                    // Paused, Queued, Failed - cancel and restart with proper verification
                    info!("Restarting download with piece verification");
                    let _ = engine.cancel(existing.id, false).await;
                    self.start_torrent_download(&engine, torrent_data.as_deref(), magnet_uri).await?
                }
            }
        } else {
            self.start_torrent_download(&engine, torrent_data.as_deref(), magnet_uri).await?
        };

        // Subscribe to events for progress
        let mut events = engine.subscribe();

        // Get current status to initialize progress tracking
        let status = engine.status(download_id);
        let mut last_downloaded: u64 = status.as_ref().map(|s| s.progress.completed_size).unwrap_or(0);
        let mut total_size: u64 = status.as_ref()
            .and_then(|s| s.progress.total_size)
            .unwrap_or(9_772_098_907); // Fallback to known size estimate (~9.1 GB)

        // Send initial progress callback with current state
        progress_callback(last_downloaded, total_size);

        let mut last_status_check = std::time::Instant::now();
        loop {
            // Check status periodically (every 1s) regardless of events
            if last_status_check.elapsed() >= Duration::from_secs(1) {
                last_status_check = std::time::Instant::now();
                if let Some(status) = engine.status(download_id) {
                    match status.state {
                        gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed => {
                            info!("AssumeUTXO snapshot download complete");
                            break;
                        }
                        _ => {
                            // Check if all bytes downloaded (gosh-dl may stay in Downloading state)
                            if let Some(total) = status.progress.total_size {
                                if status.progress.completed_size >= total && total > 0 {
                                    info!("AssumeUTXO snapshot download complete (100% verified)");
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            match tokio::time::timeout(Duration::from_millis(100), events.recv()).await {
                Ok(Ok(event)) => {
                    match event {
                        DownloadEvent::Progress { id, progress } if id == download_id => {
                            // Update total size when we get metadata
                            if let Some(total) = progress.total_size {
                                if total != total_size {
                                    total_size = total;
                                    progress_callback(last_downloaded, total_size);
                                }
                            }
                            // Update progress when downloaded bytes change
                            if progress.completed_size != last_downloaded {
                                last_downloaded = progress.completed_size;
                                progress_callback(last_downloaded, total_size);
                            }
                        }
                        // Torrent transitions to Seeding/Completed state when download finishes
                        DownloadEvent::StateChanged { id, new_state, .. } if id == download_id => {
                            match new_state {
                                gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed => {
                                    info!("AssumeUTXO snapshot download complete");
                                    break;
                                }
                                _ => {}
                            }
                        }
                        DownloadEvent::Completed { id } if id == download_id => {
                            info!("AssumeUTXO snapshot download complete");
                            break;
                        }
                        DownloadEvent::Failed { id, error, .. } if id == download_id => {
                            return Err(BitcoinError::DownloadError(format!(
                                "Torrent download failed: {}. You can download manually from: \
                                https://blog.lopp.net/bitcoin-node-sync-with-utxo-snapshots/ \
                                or generate with: bitcoin-cli dumptxoutset utxo-840000.dat",
                                error
                            )));
                        }
                        _ => {} // Ignore other events
                    }
                }
                Ok(Err(_)) => break, // Channel closed
                Err(_) => {} // Timeout - continue loop, status check happens at start
            }
        }

        // Verify file exists
        if !snapshot_path.exists() {
            return Err(BitcoinError::DownloadError(
                "Download completed but file not found".into()
            ));
        }

        // Create completion marker file
        let marker = self.data_dir.join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));
        if let Err(e) = std::fs::write(&marker, "complete") {
            warn!("Failed to create completion marker: {}", e);
        }

        info!("AssumeUTXO snapshot downloaded and verified: {:?}", snapshot_path);
        Ok(snapshot_path)
    }

    /// Helper to start torrent download with piece verification
    /// Uses add_torrent() when .torrent file provided via config, otherwise add_magnet()
    /// Both paths verify existing files and resume downloads properly
    async fn start_torrent_download(
        &self,
        engine: &std::sync::Arc<gosh_dl::DownloadEngine>,
        torrent_data: Option<&[u8]>,
        magnet_uri: &str,
    ) -> Result<gosh_dl::DownloadId, BitcoinError> {
        use gosh_dl::DownloadOptions;

        let opts = DownloadOptions::default();

        if let Some(data) = torrent_data {
            info!("Starting download with torrent file");
            engine.add_torrent(data, opts)
                .await
                .map_err(|e| BitcoinError::DownloadError(format!("Failed to add torrent: {}", e)))
        } else {
            info!("Starting download with magnet link");
            engine.add_magnet(magnet_uri, opts)
                .await
                .map_err(|e| BitcoinError::DownloadError(format!("Failed to add magnet: {}", e)))
        }
    }

    /// Download AssumeUTXO snapshot if not already present (simple version)
    pub async fn download_assumeutxo(&self) -> Result<PathBuf, BitcoinError> {
        self.download_assumeutxo_with_progress(|_, _| {}).await
    }

    /// Load AssumeUTXO snapshot via RPC (requires Bitcoin Core to be running)
    /// Waits for headers to sync to snapshot height before loading
    pub async fn load_assumeutxo(&self) -> Result<(), BitcoinError> {
        self.load_assumeutxo_with_progress(|_| {}).await
    }

    /// Load AssumeUTXO snapshot with progress callback
    /// Callback receives LoadProgress enum indicating current phase
    pub async fn load_assumeutxo_with_progress<F>(&self, progress_callback: F) -> Result<(), BitcoinError>
    where
        F: Fn(LoadProgress) + Send + Sync + 'static,
    {
        let rpc = self.rpc.as_ref()
            .ok_or_else(|| BitcoinError::ConnectionError("Bitcoin Core not running".into()))?;

        let snapshot_path = self.assumeutxo_snapshot_path();
        if !snapshot_path.exists() {
            return Err(BitcoinError::NotSynced { progress: 0.0 });
        }

        // Wait for headers to sync to at least the snapshot height
        info!("Waiting for headers to sync to height {}...", ASSUMEUTXO_HEIGHT);
        loop {
            match rpc.get_blockchain_info().await {
                Ok(status) => {
                    // Report progress to callback
                    progress_callback(LoadProgress::SyncingHeaders {
                        current: status.headers,
                        target: ASSUMEUTXO_HEIGHT,
                    });

                    // If already synced past snapshot height, no need to load it
                    if status.blocks >= ASSUMEUTXO_HEIGHT {
                        info!("Already synced to block {} - snapshot not needed", status.blocks);
                        progress_callback(LoadProgress::AlreadySynced);
                        return Ok(());
                    }
                    if status.headers >= ASSUMEUTXO_HEIGHT {
                        info!("Headers synced to {} - ready to load snapshot", status.headers);
                        break;
                    }
                    // Log progress every check
                    let progress = (status.headers as f64 / ASSUMEUTXO_HEIGHT as f64) * 100.0;
                    info!("Headers sync: {:.1}% ({}/{})", progress, status.headers, ASSUMEUTXO_HEIGHT);
                }
                Err(e) => {
                    // Warmup errors are expected, keep waiting
                    if !e.to_string().contains("warming up") {
                        warn!("Error checking headers: {}", e);
                    }
                }
            }
            sleep(Duration::from_secs(2)).await;
        }

        info!("Loading AssumeUTXO snapshot (height {})...", ASSUMEUTXO_HEIGHT);

        // Signal that we're now loading the snapshot
        progress_callback(LoadProgress::LoadingSnapshot { elapsed_secs: 0, progress_pct: None });

        // Open debug.log to monitor progress (Bitcoin Core logs loading percentage)
        let debug_log_path = self.data_dir.join("debug.log");
        let mut log_position = std::fs::metadata(&debug_log_path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Spawn the RPC call in a background task so we can update progress
        let rpc_clone = rpc.clone();
        let snapshot_path_clone = snapshot_path.clone();
        let load_task = tokio::spawn(async move {
            rpc_clone.load_utxo_snapshot(&snapshot_path_clone).await
        });

        // Poll for completion while updating elapsed time and parsing debug.log
        let start = std::time::Instant::now();
        let mut last_progress_pct: Option<f64> = None;

        loop {
            // Check if the task is done
            if load_task.is_finished() {
                break;
            }

            // Try to parse progress from debug.log
            if let Ok(file) = std::fs::File::open(&debug_log_path) {
                use std::io::{BufRead, BufReader, Seek, SeekFrom};
                let mut reader = BufReader::new(file);
                if reader.seek(SeekFrom::Start(log_position)).is_ok() {
                    loop {
                        let mut line = String::new();
                        match reader.read_line(&mut line) {
                            Ok(0) => break, // EOF
                            Ok(_) => {
                                // Look for patterns like "loading coins...71.2%" or "PopulateAndValidateSnapshot...50%"
                                if let Some(pct) = Self::parse_loading_progress(&line) {
                                    last_progress_pct = Some(pct);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    // Update position to current end
                    log_position = reader.stream_position().unwrap_or(log_position);
                }
            }

            // Update progress with elapsed time and any parsed percentage
            let elapsed = start.elapsed().as_secs();
            progress_callback(LoadProgress::LoadingSnapshot {
                elapsed_secs: elapsed,
                progress_pct: last_progress_pct,
            });

            sleep(Duration::from_millis(500)).await;
        }

        // Get the result
        match load_task.await {
            Ok(Ok(())) => {
                info!("AssumeUTXO snapshot loaded successfully!");
                info!("Node will sync remaining blocks in background");
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(e) => Err(BitcoinError::RpcError(format!("Task panicked: {}", e))),
        }
    }

    /// Parse loading progress percentage from Bitcoin Core debug.log line
    fn parse_loading_progress(line: &str) -> Option<f64> {
        // Bitcoin Core logs patterns like:
        // "2026-01-29T17:54:45Z [snapshot] 116000000 coins loaded (69.12%, 16711 MB)"
        // Look for percentage pattern near snapshot keywords
        if line.contains("[snapshot]") || line.contains("loadtxoutset") {
            // Find percentage pattern like "(69.12%," or "(50%)"
            if let Some(start) = line.find('(') {
                if let Some(pct_pos) = line[start..].find('%') {
                    let num_str = &line[start + 1..start + pct_pos];
                    if let Ok(pct) = num_str.trim().parse::<f64>() {
                        if (0.0..=100.0).contains(&pct) {
                            return Some(pct);
                        }
                    }
                }
            }
        }
        None
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
    return "win64";

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper to create manager with temp directories
    fn manager_with_temp_dirs() -> (BitcoindManager, TempDir, TempDir) {
        let data_dir = TempDir::new().unwrap();
        let install_dir = TempDir::new().unwrap();

        let manager = BitcoindManager::new()
            .with_data_dir(data_dir.path().to_path_buf());

        // Manually set install_dir since there's no builder for it
        let mut manager = manager;
        manager.install_dir = install_dir.path().to_path_buf();

        (manager, data_dir, install_dir)
    }

    #[test]
    fn test_manager_new() {
        let manager = BitcoindManager::new();
        assert!(!manager.assumeutxo);
        assert!(manager.rpc.is_none());
        assert!(manager.process.is_none());
        assert_eq!(manager.prune_size, 10000);
        assert_eq!(manager.rpc_port, 8332);
    }

    #[test]
    fn test_manager_default() {
        let manager = BitcoindManager::default();
        assert!(!manager.assumeutxo);
        assert!(manager.rpc.is_none());
    }

    #[test]
    fn test_with_data_dir() {
        let manager = BitcoindManager::new()
            .with_data_dir(PathBuf::from("/custom/path"));
        assert_eq!(manager.data_dir, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_with_prune() {
        let manager = BitcoindManager::new().with_prune(5000);
        assert_eq!(manager.prune_size, 5000);
    }

    #[test]
    fn test_with_dbcache_manual() {
        let manager = BitcoindManager::new().with_dbcache(1024);
        assert_eq!(manager.dbcache, 1024);
    }

    #[test]
    fn test_with_dbcache_auto() {
        let manager = BitcoindManager::new().with_dbcache(0);
        // Auto-calculated, should be between 450 and 16384
        assert!(manager.dbcache >= 450);
        assert!(manager.dbcache <= 16384);
    }

    #[test]
    fn test_with_dbcache_clamped_low() {
        let manager = BitcoindManager::new().with_dbcache(100);
        assert_eq!(manager.dbcache, 450); // Clamped to minimum
    }

    #[test]
    fn test_with_dbcache_clamped_high() {
        let manager = BitcoindManager::new().with_dbcache(50000);
        assert_eq!(manager.dbcache, 16384); // Clamped to maximum
    }

    #[test]
    fn test_with_assumeutxo() {
        let manager = BitcoindManager::new().with_assumeutxo(true);
        assert!(manager.assumeutxo);
        assert!(manager.assumeutxo_enabled());
    }

    #[test]
    fn test_with_assumeutxo_magnet() {
        let magnet = "magnet:?xt=urn:btih:test";
        let manager = BitcoindManager::new()
            .with_assumeutxo_magnet(Some(magnet.to_string()));
        assert_eq!(manager.assumeutxo_magnet, Some(magnet.to_string()));
    }

    #[test]
    fn test_with_rpc_auth() {
        let manager = BitcoindManager::new()
            .with_rpc_auth("myuser", "mypassword");
        assert_eq!(manager.rpc_user, "myuser");
        assert_eq!(manager.rpc_password, "mypassword");
    }

    #[test]
    fn test_with_rpc_port() {
        let manager = BitcoindManager::new().with_rpc_port(18332);
        assert_eq!(manager.rpc_port, 18332);
    }

    #[test]
    fn test_is_installed_false() {
        let (manager, _data, _install) = manager_with_temp_dirs();
        assert!(!manager.is_installed());
    }

    #[test]
    fn test_is_installed_true() {
        let (manager, _data, install) = manager_with_temp_dirs();

        // Create fake bitcoind binary
        let bin_dir = install.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(target_os = "windows")]
        fs::write(bin_dir.join("bitcoind.exe"), "fake").unwrap();
        #[cfg(not(target_os = "windows"))]
        fs::write(bin_dir.join("bitcoind"), "fake").unwrap();

        assert!(manager.is_installed());
    }

    #[test]
    fn test_installed_version_not_installed() {
        let (manager, _data, _install) = manager_with_temp_dirs();
        assert!(manager.installed_version().is_none());
    }

    #[test]
    fn test_needs_upgrade_not_installed() {
        let (manager, _data, _install) = manager_with_temp_dirs();
        assert!(!manager.needs_upgrade()); // Can't upgrade what's not installed
    }

    #[test]
    fn test_bitcoind_path() {
        let (manager, _data, install) = manager_with_temp_dirs();

        #[cfg(target_os = "windows")]
        assert_eq!(
            manager.bitcoind_path(),
            install.path().join("bin").join("bitcoind.exe")
        );

        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            manager.bitcoind_path(),
            install.path().join("bin").join("bitcoind")
        );
    }

    #[test]
    fn test_assumeutxo_snapshot_path() {
        let (manager, data, _install) = manager_with_temp_dirs();
        let expected = data.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT));
        assert_eq!(manager.assumeutxo_snapshot_path(), expected);
    }

    #[test]
    fn test_has_assumeutxo_snapshot_false() {
        let (manager, _data, _install) = manager_with_temp_dirs();
        assert!(!manager.has_assumeutxo_snapshot());
    }

    #[test]
    fn test_has_assumeutxo_snapshot_incomplete() {
        let (manager, data, _install) = manager_with_temp_dirs();

        // Create snapshot file but no marker
        let snapshot_path = data.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT));
        fs::write(&snapshot_path, "fake snapshot").unwrap();

        assert!(!manager.has_assumeutxo_snapshot());
    }

    #[test]
    fn test_has_assumeutxo_snapshot_complete() {
        let (manager, data, _install) = manager_with_temp_dirs();

        // Create both snapshot file and marker
        let snapshot_path = data.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT));
        let marker_path = data.path().join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));
        fs::write(&snapshot_path, "fake snapshot").unwrap();
        fs::write(&marker_path, "complete").unwrap();

        assert!(manager.has_assumeutxo_snapshot());
    }

    #[test]
    fn test_rpc_none_initially() {
        let manager = BitcoindManager::new();
        assert!(manager.rpc().is_none());
    }

    #[test]
    fn test_write_config_creates_file() {
        let (mut manager, data, _install) = manager_with_temp_dirs();
        manager.rpc_user = "testuser".to_string();
        manager.rpc_password = "testpass".to_string();

        manager.write_config().unwrap();

        let config_path = data.path().join("bitcoin.conf");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("rpcuser=testuser"));
        assert!(content.contains("rpcpassword=testpass"));
        assert!(content.contains("server=1"));
        assert!(content.contains(&format!("prune={}", manager.prune_size)));
    }

    #[test]
    fn test_write_config_respects_existing() {
        let (mut manager, data, _install) = manager_with_temp_dirs();

        // Write existing config with different credentials
        let config_path = data.path().join("bitcoin.conf");
        fs::write(&config_path, "rpcuser=existinguser\nrpcpassword=existingpass\n").unwrap();

        // Write config should read existing
        manager.write_config().unwrap();

        // Manager should use existing credentials
        assert_eq!(manager.rpc_user, "existinguser");
        assert_eq!(manager.rpc_password, "existingpass");
    }

    #[test]
    fn test_read_existing_config_with_port() {
        let (mut manager, data, _install) = manager_with_temp_dirs();

        let config_path = data.path().join("bitcoin.conf");
        fs::write(&config_path, "rpcuser=u\nrpcpassword=p\nrpcport=18332\n").unwrap();

        assert!(manager.read_existing_config());
        assert_eq!(manager.rpc_user, "u");
        assert_eq!(manager.rpc_password, "p");
        assert_eq!(manager.rpc_port, 18332);
    }

    #[test]
    fn test_read_existing_config_partial() {
        let (mut manager, data, _install) = manager_with_temp_dirs();

        // Only user, no password
        let config_path = data.path().join("bitcoin.conf");
        fs::write(&config_path, "rpcuser=u\n").unwrap();

        // Should return false - incomplete credentials
        assert!(!manager.read_existing_config());
    }

    #[test]
    fn test_read_existing_config_missing() {
        let (mut manager, _data, _install) = manager_with_temp_dirs();
        assert!(!manager.read_existing_config());
    }

    #[test]
    fn test_start_not_installed() {
        let (mut manager, _data, _install) = manager_with_temp_dirs();
        let result = manager.start();
        assert!(result.is_err());
    }

    #[test]
    fn test_stop_when_not_running() {
        let (mut manager, _data, _install) = manager_with_temp_dirs();
        // Should not panic
        let result = manager.stop();
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_random_string_length() {
        let s = generate_random_string(16);
        assert_eq!(s.len(), 16);

        let s = generate_random_string(32);
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn test_generate_random_string_alphanumeric() {
        let s = generate_random_string(100);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_random_string_different_each_time() {
        // Small chance of collision, but very unlikely
        let s1 = generate_random_string(32);
        std::thread::sleep(std::time::Duration::from_millis(1));
        let s2 = generate_random_string(32);
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_get_platform() {
        let platform = get_platform();
        assert!(!platform.is_empty());

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(platform, "x86_64-linux-gnu");

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        assert_eq!(platform, "aarch64-linux-gnu");

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        assert_eq!(platform, "x86_64-apple-darwin");

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert_eq!(platform, "arm64-apple-darwin");

        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(platform, "win64");
    }

    #[test]
    fn test_calculate_dbcache_in_range() {
        let dbcache = BitcoindManager::calculate_dbcache();
        assert!(dbcache >= 450, "dbcache {} should be >= 450", dbcache);
        assert!(dbcache <= 16384, "dbcache {} should be <= 16384", dbcache);
    }

    #[test]
    fn test_builder_chaining() {
        let manager = BitcoindManager::new()
            .with_data_dir(PathBuf::from("/data"))
            .with_prune(5000)
            .with_dbcache(2048)
            .with_assumeutxo(true)
            .with_rpc_auth("user", "pass")
            .with_rpc_port(18332);

        assert_eq!(manager.data_dir, PathBuf::from("/data"));
        assert_eq!(manager.prune_size, 5000);
        assert_eq!(manager.dbcache, 2048);
        assert!(manager.assumeutxo);
        assert_eq!(manager.rpc_user, "user");
        assert_eq!(manager.rpc_password, "pass");
        assert_eq!(manager.rpc_port, 18332);
    }

    #[tokio::test]
    async fn test_get_sync_status_not_started() {
        let manager = BitcoindManager::new();
        let result = manager.get_sync_status().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wait_ready_not_started() {
        let manager = BitcoindManager::new();
        let result = manager.wait_ready(1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_assumeutxo_not_started() {
        let manager = BitcoindManager::new();
        let result = manager.load_assumeutxo().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_loading_progress_snapshot_log() {
        let line = "2026-01-29T17:54:45Z [snapshot] 116000000 coins loaded (69.12%, 16711 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, Some(69.12));
    }

    #[test]
    fn test_parse_loading_progress_loadtxoutset_log() {
        let line = "2026-01-29T17:54:45Z loadtxoutset progress (50%, 10000 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, Some(50.0));
    }

    #[test]
    fn test_parse_loading_progress_100_percent() {
        let line = "2026-01-29T17:54:45Z [snapshot] completed (100%, 20000 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, Some(100.0));
    }

    #[test]
    fn test_parse_loading_progress_0_percent() {
        let line = "2026-01-29T17:54:45Z [snapshot] starting (0%, 0 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, Some(0.0));
    }

    #[test]
    fn test_parse_loading_progress_no_match() {
        let line = "2026-01-29T17:54:45Z Some other log message";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, None);
    }

    #[test]
    fn test_parse_loading_progress_invalid_percentage() {
        // Percentage outside valid range should not match
        let line = "2026-01-29T17:54:45Z [snapshot] error (150%, 20000 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, None);
    }

    #[test]
    fn test_parse_loading_progress_negative_percentage() {
        let line = "2026-01-29T17:54:45Z [snapshot] error (-5%, 0 MB)";
        let progress = BitcoindManager::parse_loading_progress(line);
        assert_eq!(progress, None);
    }

    #[test]
    fn test_debug_log_path() {
        let (manager, data, _install) = manager_with_temp_dirs();
        let expected = data.path().join("debug.log");
        assert_eq!(manager.debug_log_path(), expected);
    }

    #[test]
    fn test_data_dir() {
        let (manager, data, _install) = manager_with_temp_dirs();
        assert_eq!(manager.data_dir(), data.path());
    }

    #[test]
    fn test_with_assumeutxo_torrent() {
        let manager = BitcoindManager::new()
            .with_assumeutxo_torrent(Some(PathBuf::from("/path/to/snapshot.torrent")));
        assert_eq!(manager.assumeutxo_torrent, Some(PathBuf::from("/path/to/snapshot.torrent")));
    }

    #[test]
    fn test_with_assumeutxo_torrent_none() {
        let manager = BitcoindManager::new()
            .with_assumeutxo_torrent(None);
        assert!(manager.assumeutxo_torrent.is_none());
    }
}
