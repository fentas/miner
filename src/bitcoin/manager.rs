//! Bitcoin Core lifecycle management
//!
//! This module provides the `BitcoindManager` struct for managing Bitcoin Core
//! installation, configuration, and process lifecycle.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::config_writer::{self, BitcoinConfig};
use super::downloader::{self, BITCOIN_CORE_VERSION};
use super::snapshot;
use super::{BitcoinError, BitcoinRpc, LoadProgress, SyncStatus};

/// Manages Bitcoin Core installation and lifecycle
pub struct BitcoindManager {
    data_dir: PathBuf,
    install_dir: PathBuf,
    rpc: Option<BitcoinRpc>,
    process: Option<Child>,
    config: BitcoinConfig,
    assumeutxo: bool,
    assumeutxo_magnet: Option<String>,
    assumeutxo_torrent: Option<PathBuf>,
}

impl BitcoindManager {
    /// Create a new manager with default directories
    pub fn new() -> Self {
        // Platform-specific default directories
        #[cfg(target_os = "windows")]
        let (data_dir, install_dir) = {
            let appdata = std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
            // On Windows, Bitcoin Core uses "Bitcoin Core" as default data directory
            let bitcoin_core_dir = appdata.join("Bitcoin Core");
            (bitcoin_core_dir.clone(), bitcoin_core_dir)
        };

        #[cfg(not(target_os = "windows"))]
        let (data_dir, install_dir) = {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            (home.join(".bitcoin"), home.join(".local/share/bitcoin-core"))
        };

        Self {
            data_dir,
            install_dir,
            rpc: None,
            process: None,
            config: BitcoinConfig::default(),
            assumeutxo: false,
            assumeutxo_magnet: None,
            assumeutxo_torrent: None,
        }
    }

    /// Set custom data directory
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }

    /// Set prune size in MB
    pub fn with_prune(mut self, size: u32) -> Self {
        self.config.prune_size = size;
        self
    }

    /// Set dbcache size in MB (0 = auto-calculate)
    pub fn with_dbcache(mut self, size: u32) -> Self {
        if size == 0 {
            self.config.dbcache = config_writer::calculate_dbcache();
        } else {
            self.config.dbcache = size.clamp(450, 16384);
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
        self.config.rpc_user = user.to_string();
        self.config.rpc_password = password.to_string();
        self
    }

    /// Set RPC port
    pub fn with_rpc_port(mut self, port: u16) -> Self {
        self.config.rpc_port = port;
        self
    }

    /// Connect to an external Bitcoin Core RPC (without managing the process)
    pub fn connect(&mut self, host: &str, port: u16, user: &str, password: &str) {
        self.config.rpc_port = port;
        self.config.rpc_user = user.to_string();
        self.config.rpc_password = password.to_string();
        self.rpc = Some(BitcoinRpc::new(host, port).with_auth(user, password));
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

        let output = Command::new(self.bitcoind_path())
            .arg("-version")
            .output()
            .ok()?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        for line in version_str.lines() {
            if line.contains("Bitcoin Core version") {
                if let Some(v) = line.split('v').nth(1) {
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
        downloader::download_and_install(&self.install_dir).await
    }

    /// Write bitcoin.conf with optimal settings (only if needed)
    pub fn write_config(&mut self) -> Result<(), BitcoinError> {
        // Check if config already exists with valid credentials
        if let Some(existing) = config_writer::read_existing_config(&self.data_dir) {
            self.config.rpc_user = existing.rpc_user;
            self.config.rpc_password = existing.rpc_password;
            self.config.rpc_port = existing.rpc_port;
            return Ok(());
        }

        config_writer::write_config(&self.data_dir, &self.config)
    }

    /// Start Bitcoin Core daemon
    pub fn start(&mut self) -> Result<(), BitcoinError> {
        if self.process.is_some() {
            warn!("Bitcoin Core is already running");
            return Ok(());
        }

        let bitcoind = self.bitcoind_path();
        if !bitcoind.exists() {
            return Err(BitcoinError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("bitcoind not found at {:?}", bitcoind),
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
                    format!("Failed to spawn bitcoind: {}", e),
                ))
            })?;

        self.process = Some(child);

        // Initialize RPC client
        self.rpc = Some(
            BitcoinRpc::new("127.0.0.1", self.config.rpc_port)
                .with_auth(&self.config.rpc_user, &self.config.rpc_password),
        );

        Ok(())
    }

    /// Stop Bitcoin Core daemon gracefully (async version)
    pub async fn stop_async(&mut self) -> Result<(), BitcoinError> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping Bitcoin Core...");

            // Try graceful shutdown via RPC first
            if let Some(ref rpc) = self.rpc {
                match rpc.stop().await {
                    Ok(_) => {
                        info!("Sent stop command to Bitcoin Core");
                        let start = std::time::Instant::now();
                        while start.elapsed() < Duration::from_secs(30) {
                            match process.try_wait() {
                                Ok(Some(_)) => {
                                    info!("Bitcoin Core stopped gracefully");
                                    self.rpc = None;
                                    return Ok(());
                                }
                                Ok(None) => {
                                    sleep(Duration::from_millis(500)).await;
                                }
                                Err(_) => break,
                            }
                        }
                        warn!("Graceful shutdown timed out, force killing...");
                    }
                    Err(e) => {
                        warn!("RPC stop failed: {}, force killing...", e);
                    }
                }
            }

            let _ = process.kill();
            let _ = process.wait();

            info!("Bitcoin Core stopped");
        }

        self.rpc = None;
        Ok(())
    }

    /// Stop Bitcoin Core daemon (sync version, used for Drop)
    pub fn stop(&mut self) -> Result<(), BitcoinError> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping Bitcoin Core (force)...");
            let _ = process.kill();
            let _ = process.wait();
            info!("Bitcoin Core stopped");
        }

        self.rpc = None;
        Ok(())
    }

    /// Wait for Bitcoin Core to be ready and synced
    pub async fn wait_ready(&self, timeout_secs: u64) -> Result<(), BitcoinError> {
        let rpc = self
            .rpc
            .as_ref()
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

                    if last_progress_time.elapsed() >= Duration::from_secs(5) {
                        info!(
                            "Syncing: {:.2}% ({}/{} blocks)",
                            status.progress, status.blocks, status.headers
                        );
                        last_progress_time = std::time::Instant::now();
                    }
                }
                Err(_) if !connected => {}
                Err(e) => {
                    warn!("RPC error during sync: {}", e);
                }
            }
            sleep(Duration::from_secs(1)).await;
        }

        if !connected {
            Err(BitcoinError::ConnectionError(
                "Timeout waiting for Bitcoin Core to start".into(),
            ))
        } else if let Ok(status) = rpc.get_blockchain_info().await {
            Err(BitcoinError::NotSynced {
                progress: status.progress,
            })
        } else {
            Err(BitcoinError::ConnectionError(
                "Lost connection to Bitcoin Core".into(),
            ))
        }
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> Result<SyncStatus, BitcoinError> {
        let rpc = self
            .rpc
            .as_ref()
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
    pub fn has_assumeutxo_snapshot(&self) -> bool {
        snapshot::has_snapshot(&self.data_dir)
    }

    /// Get AssumeUTXO snapshot path
    pub fn assumeutxo_snapshot_path(&self) -> PathBuf {
        snapshot::snapshot_path(&self.data_dir)
    }

    /// Download AssumeUTXO snapshot via BitTorrent with progress callback
    pub async fn download_assumeutxo_with_progress<F>(
        &self,
        progress_callback: F,
    ) -> Result<PathBuf, BitcoinError>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        snapshot::download_with_progress(
            &self.data_dir,
            self.assumeutxo_magnet.as_deref(),
            self.assumeutxo_torrent.as_deref(),
            progress_callback,
        )
        .await
    }

    /// Download AssumeUTXO snapshot if not already present (simple version)
    pub async fn download_assumeutxo(&self) -> Result<PathBuf, BitcoinError> {
        self.download_assumeutxo_with_progress(|_, _| {}).await
    }

    /// Load AssumeUTXO snapshot via RPC (requires Bitcoin Core to be running)
    pub async fn load_assumeutxo(&self) -> Result<(), BitcoinError> {
        self.load_assumeutxo_with_progress(|_| {}).await
    }

    /// Load AssumeUTXO snapshot with progress callback
    pub async fn load_assumeutxo_with_progress<F>(
        &self,
        progress_callback: F,
    ) -> Result<(), BitcoinError>
    where
        F: Fn(LoadProgress) + Send + Sync + 'static,
    {
        let rpc = self
            .rpc
            .as_ref()
            .ok_or_else(|| BitcoinError::ConnectionError("Bitcoin Core not running".into()))?;

        snapshot::load_with_progress(&self.data_dir, rpc, progress_callback).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin::snapshot::ASSUMEUTXO_HEIGHT;
    use std::fs;
    use tempfile::TempDir;

    fn manager_with_temp_dirs() -> (BitcoindManager, TempDir, TempDir) {
        let data_dir = TempDir::new().unwrap();
        let install_dir = TempDir::new().unwrap();

        let manager = BitcoindManager::new().with_data_dir(data_dir.path().to_path_buf());

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
        assert_eq!(manager.config.prune_size, 10000);
        assert_eq!(manager.config.rpc_port, 8332);
    }

    #[test]
    fn test_manager_default() {
        let manager = BitcoindManager::default();
        assert!(!manager.assumeutxo);
        assert!(manager.rpc.is_none());
    }

    #[test]
    fn test_with_data_dir() {
        let manager = BitcoindManager::new().with_data_dir(PathBuf::from("/custom/path"));
        assert_eq!(manager.data_dir, PathBuf::from("/custom/path"));
    }

    #[test]
    fn test_with_prune() {
        let manager = BitcoindManager::new().with_prune(5000);
        assert_eq!(manager.config.prune_size, 5000);
    }

    #[test]
    fn test_with_dbcache_manual() {
        let manager = BitcoindManager::new().with_dbcache(1024);
        assert_eq!(manager.config.dbcache, 1024);
    }

    #[test]
    fn test_with_dbcache_auto() {
        let manager = BitcoindManager::new().with_dbcache(0);
        assert!(manager.config.dbcache >= 450);
        assert!(manager.config.dbcache <= 16384);
    }

    #[test]
    fn test_with_dbcache_clamped_low() {
        let manager = BitcoindManager::new().with_dbcache(100);
        assert_eq!(manager.config.dbcache, 450);
    }

    #[test]
    fn test_with_dbcache_clamped_high() {
        let manager = BitcoindManager::new().with_dbcache(50000);
        assert_eq!(manager.config.dbcache, 16384);
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
        let manager = BitcoindManager::new().with_assumeutxo_magnet(Some(magnet.to_string()));
        assert_eq!(manager.assumeutxo_magnet, Some(magnet.to_string()));
    }

    #[test]
    fn test_with_rpc_auth() {
        let manager = BitcoindManager::new().with_rpc_auth("myuser", "mypassword");
        assert_eq!(manager.config.rpc_user, "myuser");
        assert_eq!(manager.config.rpc_password, "mypassword");
    }

    #[test]
    fn test_with_rpc_port() {
        let manager = BitcoindManager::new().with_rpc_port(18332);
        assert_eq!(manager.config.rpc_port, 18332);
    }

    #[test]
    fn test_is_installed_false() {
        let (manager, _data, _install) = manager_with_temp_dirs();
        assert!(!manager.is_installed());
    }

    #[test]
    fn test_is_installed_true() {
        let (manager, _data, install) = manager_with_temp_dirs();

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
        assert!(!manager.needs_upgrade());
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

        let snapshot_path = data.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT));
        fs::write(&snapshot_path, "fake snapshot").unwrap();

        assert!(!manager.has_assumeutxo_snapshot());
    }

    #[test]
    fn test_has_assumeutxo_snapshot_complete() {
        let (manager, data, _install) = manager_with_temp_dirs();

        let snapshot_path = data.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT));
        let marker_path = data
            .path()
            .join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT));
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
        manager.config.rpc_user = "testuser".to_string();
        manager.config.rpc_password = "testpass".to_string();

        manager.write_config().unwrap();

        let config_path = data.path().join("bitcoin.conf");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("rpcuser=testuser"));
        assert!(content.contains("rpcpassword=testpass"));
        assert!(content.contains("server=1"));
        assert!(content.contains(&format!("prune={}", manager.config.prune_size)));
    }

    #[test]
    fn test_write_config_respects_existing() {
        let (mut manager, data, _install) = manager_with_temp_dirs();

        let config_path = data.path().join("bitcoin.conf");
        fs::write(
            &config_path,
            "rpcuser=existinguser\nrpcpassword=existingpass\n",
        )
        .unwrap();

        manager.write_config().unwrap();

        assert_eq!(manager.config.rpc_user, "existinguser");
        assert_eq!(manager.config.rpc_password, "existingpass");
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
        let result = manager.stop();
        assert!(result.is_ok());
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
        assert_eq!(manager.config.prune_size, 5000);
        assert_eq!(manager.config.dbcache, 2048);
        assert!(manager.assumeutxo);
        assert_eq!(manager.config.rpc_user, "user");
        assert_eq!(manager.config.rpc_password, "pass");
        assert_eq!(manager.config.rpc_port, 18332);
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
        assert_eq!(
            manager.assumeutxo_torrent,
            Some(PathBuf::from("/path/to/snapshot.torrent"))
        );
    }

    #[test]
    fn test_with_assumeutxo_torrent_none() {
        let manager = BitcoindManager::new().with_assumeutxo_torrent(None);
        assert!(manager.assumeutxo_torrent.is_none());
    }
}
