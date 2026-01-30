//! AssumeUTXO snapshot download and loading
//!
//! This module handles downloading and loading UTXO snapshots for fast
//! initial sync using Bitcoin Core's AssumeUTXO feature.

use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use super::{BitcoinError, BitcoinRpc, LoadProgress};

/// AssumeUTXO snapshot height (Bitcoin Core v30+)
pub const ASSUMEUTXO_HEIGHT: u64 = 910000;

/// Default magnet URI for AssumeUTXO snapshot
/// The hash is verified by Bitcoin Core when loading via loadtxoutset RPC.
/// Using torrent for trustless download - verified by infohash + Bitcoin Core hash check.
const ASSUMEUTXO_MAGNET: &str = "magnet:?xt=urn:btih:7019437a2b1530624b100c0795cfc5f90b8322ca&dn=utxo-910000.dat&tr=udp://tracker.openbittorrent.com:80&tr=udp://tracker.bitcoin.sprovoost.nl:6969";

/// Get the snapshot file path for a given data directory
pub fn snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir.join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT))
}

/// Get the completion marker path for a given data directory
fn marker_path(data_dir: &Path) -> PathBuf {
    data_dir.join(format!("utxo-{}.dat.complete", ASSUMEUTXO_HEIGHT))
}

/// Check if AssumeUTXO snapshot exists and download is complete
/// Uses a .complete marker file to track successful torrent completion
pub fn has_snapshot(data_dir: &Path) -> bool {
    let path = snapshot_path(data_dir);
    let marker = marker_path(data_dir);

    // Both the snapshot and the completion marker must exist
    path.exists() && marker.exists()
}

/// Download AssumeUTXO snapshot via BitTorrent with progress callback
/// Callback receives (bytes_downloaded, total_bytes)
pub async fn download_with_progress<F>(
    data_dir: &Path,
    magnet_override: Option<&str>,
    torrent_file: Option<&Path>,
    progress_callback: F,
) -> Result<PathBuf, BitcoinError>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    // Prefer torrent file if provided (enables proper piece verification on resume)
    if let Some(torrent_path) = torrent_file {
        if torrent_path.exists() {
            info!("Using configured torrent file: {:?}", torrent_path);
            return download_from_torrent(data_dir, torrent_path, progress_callback).await;
        } else {
            warn!("Configured torrent file not found: {:?}", torrent_path);
        }
    }

    let magnet = magnet_override.unwrap_or(ASSUMEUTXO_MAGNET);
    download_from_magnet(data_dir, magnet, progress_callback).await
}

/// Download AssumeUTXO snapshot from a torrent file
async fn download_from_torrent<F>(
    data_dir: &Path,
    torrent_path: &Path,
    progress_callback: F,
) -> Result<PathBuf, BitcoinError>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    use gosh_dl::{DownloadEngine, DownloadOptions, EngineConfig};

    let dest_path = snapshot_path(data_dir);

    // Ensure data directory exists
    std::fs::create_dir_all(data_dir)?;

    // Read torrent file
    let torrent_data = std::fs::read(torrent_path)
        .map_err(|e| BitcoinError::DownloadError(format!("Failed to read torrent file: {}", e)))?;

    // Validate it's a real torrent file
    if torrent_data.first() != Some(&b'd') {
        return Err(BitcoinError::DownloadError(
            "Invalid torrent file format".into(),
        ));
    }

    info!(
        "Downloading AssumeUTXO snapshot (height {}) from torrent file...",
        ASSUMEUTXO_HEIGHT
    );

    // Configure download engine
    let config = EngineConfig {
        download_dir: data_dir.to_path_buf(),
        database_path: Some(data_dir.join("gosh-dl.db")),
        ..Default::default()
    };

    let engine = DownloadEngine::new(config)
        .await
        .map_err(|e| BitcoinError::DownloadError(format!("Failed to create download engine: {}", e)))?;

    // Start download with torrent file (enables piece verification)
    let opts = DownloadOptions::default();
    let download_id = engine
        .add_torrent(&torrent_data, opts)
        .await
        .map_err(|e| BitcoinError::DownloadError(format!("Failed to add torrent: {}", e)))?;

    // Monitor download progress
    wait_for_download(&engine, download_id, &progress_callback).await?;

    if !dest_path.exists() {
        return Err(BitcoinError::DownloadError(
            "Download completed but file not found".into(),
        ));
    }

    // Create completion marker
    if let Err(e) = std::fs::write(marker_path(data_dir), "complete") {
        warn!("Failed to create completion marker: {}", e);
    }

    info!("AssumeUTXO snapshot downloaded and verified: {:?}", dest_path);
    Ok(dest_path)
}

/// Download AssumeUTXO snapshot from a magnet URI
async fn download_from_magnet<F>(
    data_dir: &Path,
    magnet_uri: &str,
    progress_callback: F,
) -> Result<PathBuf, BitcoinError>
where
    F: Fn(u64, u64) + Send + Sync + 'static,
{
    use gosh_dl::torrent::MagnetUri;
    use gosh_dl::{DownloadEngine, EngineConfig};

    let dest_path = snapshot_path(data_dir);

    // Ensure data directory exists
    std::fs::create_dir_all(data_dir)?;

    // Parse magnet to extract info hash
    let magnet = MagnetUri::parse(magnet_uri)
        .map_err(|e| BitcoinError::DownloadError(format!("Invalid magnet URI: {}", e)))?;
    let info_hash = magnet.info_hash_hex();

    info!(
        "Downloading AssumeUTXO snapshot (height {}) via BitTorrent...",
        ASSUMEUTXO_HEIGHT
    );
    info!("Info hash: {}", info_hash);

    // Configure download engine with persistent session state
    let config = EngineConfig {
        download_dir: data_dir.to_path_buf(),
        database_path: Some(data_dir.join("gosh-dl.db")),
        ..Default::default()
    };

    let engine = DownloadEngine::new(config)
        .await
        .map_err(|e| BitcoinError::DownloadError(format!("Failed to create download engine: {}", e)))?;

    // Check if we have a cached .torrent file (for proper piece verification on resume)
    let torrent_cache_path = data_dir.join(format!("{}.torrent", info_hash));
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
                std::fs::write(marker_path(data_dir), "complete")?;
                return Ok(dest_path);
            }
            gosh_dl::DownloadState::Downloading => {
                info!("Resuming existing download");
                existing.id
            }
            _ => {
                // Paused, Queued, Failed - cancel and restart with proper verification
                info!("Restarting download with piece verification");
                let _ = engine.cancel(existing.id, false).await;
                start_torrent_download(&engine, torrent_data.as_deref(), magnet_uri).await?
            }
        }
    } else {
        start_torrent_download(&engine, torrent_data.as_deref(), magnet_uri).await?
    };

    // Monitor download progress
    wait_for_download(&engine, download_id, &progress_callback).await?;

    // Verify file exists
    if !dest_path.exists() {
        return Err(BitcoinError::DownloadError(
            "Download completed but file not found".into(),
        ));
    }

    // Create completion marker file
    if let Err(e) = std::fs::write(marker_path(data_dir), "complete") {
        warn!("Failed to create completion marker: {}", e);
    }

    info!("AssumeUTXO snapshot downloaded and verified: {:?}", dest_path);
    Ok(dest_path)
}

/// Start a torrent download from either torrent data or magnet URI
async fn start_torrent_download(
    engine: &std::sync::Arc<gosh_dl::DownloadEngine>,
    torrent_data: Option<&[u8]>,
    magnet_uri: &str,
) -> Result<gosh_dl::DownloadId, BitcoinError> {
    use gosh_dl::DownloadOptions;

    let opts = DownloadOptions::default();

    if let Some(data) = torrent_data {
        info!("Starting download with torrent file");
        engine
            .add_torrent(data, opts)
            .await
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to add torrent: {}", e)))
    } else {
        info!("Starting download with magnet link");
        engine
            .add_magnet(magnet_uri, opts)
            .await
            .map_err(|e| BitcoinError::DownloadError(format!("Failed to add magnet: {}", e)))
    }
}

/// Wait for a download to complete, calling progress callback
async fn wait_for_download<F>(
    engine: &std::sync::Arc<gosh_dl::DownloadEngine>,
    download_id: gosh_dl::DownloadId,
    progress_callback: &F,
) -> Result<(), BitcoinError>
where
    F: Fn(u64, u64),
{
    use gosh_dl::DownloadEvent;

    // Subscribe to events for progress
    let mut events = engine.subscribe();

    // Get current status to initialize progress tracking
    let status = engine.status(download_id);
    let mut last_downloaded: u64 = status.as_ref().map(|s| s.progress.completed_size).unwrap_or(0);
    let mut total_size: u64 = status
        .as_ref()
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
                        return Ok(());
                    }
                    _ => {
                        // Check if all bytes downloaded (gosh-dl may stay in Downloading state)
                        if let Some(total) = status.progress.total_size {
                            if status.progress.completed_size >= total && total > 0 {
                                info!("AssumeUTXO snapshot download complete (100% verified)");
                                return Ok(());
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
                    DownloadEvent::StateChanged {
                        id,
                        new_state:
                            gosh_dl::DownloadState::Seeding | gosh_dl::DownloadState::Completed,
                        ..
                    } if id == download_id => {
                        info!("AssumeUTXO snapshot download complete");
                        return Ok(());
                    }
                    DownloadEvent::StateChanged { .. } => {}
                    DownloadEvent::Completed { id } if id == download_id => {
                        info!("AssumeUTXO snapshot download complete");
                        return Ok(());
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
            Err(_) => {}         // Timeout - continue loop, status check happens at start
        }
    }

    Ok(())
}

/// Load AssumeUTXO snapshot with progress callback
/// Callback receives LoadProgress enum indicating current phase
pub async fn load_with_progress<F>(
    data_dir: &Path,
    rpc: &BitcoinRpc,
    progress_callback: F,
) -> Result<(), BitcoinError>
where
    F: Fn(LoadProgress) + Send + Sync + 'static,
{
    let path = snapshot_path(data_dir);
    if !path.exists() {
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
                    info!(
                        "Already synced to block {} - snapshot not needed",
                        status.blocks
                    );
                    progress_callback(LoadProgress::AlreadySynced);
                    return Ok(());
                }
                if status.headers >= ASSUMEUTXO_HEIGHT {
                    info!(
                        "Headers synced to {} - ready to load snapshot",
                        status.headers
                    );
                    break;
                }
                // Log progress every check
                let progress = (status.headers as f64 / ASSUMEUTXO_HEIGHT as f64) * 100.0;
                info!(
                    "Headers sync: {:.1}% ({}/{})",
                    progress, status.headers, ASSUMEUTXO_HEIGHT
                );
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
    progress_callback(LoadProgress::LoadingSnapshot {
        elapsed_secs: 0,
        progress_pct: None,
    });

    // Open debug.log to monitor progress (Bitcoin Core logs loading percentage)
    let debug_log_path = data_dir.join("debug.log");
    let mut log_position = std::fs::metadata(&debug_log_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Spawn the RPC call in a background task so we can update progress
    let rpc_clone = rpc.clone();
    let path_clone = path.clone();
    let load_task = tokio::spawn(async move { rpc_clone.load_utxo_snapshot(&path_clone).await });

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
                            if let Some(pct) = parse_loading_progress(&line) {
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
pub fn parse_loading_progress(line: &str) -> Option<f64> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_path() {
        let dir = TempDir::new().unwrap();
        let path = snapshot_path(dir.path());
        assert_eq!(
            path,
            dir.path().join(format!("utxo-{}.dat", ASSUMEUTXO_HEIGHT))
        );
    }

    #[test]
    fn test_has_snapshot_false() {
        let dir = TempDir::new().unwrap();
        assert!(!has_snapshot(dir.path()));
    }

    #[test]
    fn test_has_snapshot_incomplete() {
        let dir = TempDir::new().unwrap();

        // Create snapshot file but no marker
        let path = snapshot_path(dir.path());
        std::fs::write(&path, "fake snapshot").unwrap();

        assert!(!has_snapshot(dir.path()));
    }

    #[test]
    fn test_has_snapshot_complete() {
        let dir = TempDir::new().unwrap();

        // Create both snapshot file and marker
        let path = snapshot_path(dir.path());
        let marker = marker_path(dir.path());
        std::fs::write(&path, "fake snapshot").unwrap();
        std::fs::write(&marker, "complete").unwrap();

        assert!(has_snapshot(dir.path()));
    }

    #[test]
    fn test_parse_loading_progress_snapshot_log() {
        let line = "2026-01-29T17:54:45Z [snapshot] 116000000 coins loaded (69.12%, 16711 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, Some(69.12));
    }

    #[test]
    fn test_parse_loading_progress_loadtxoutset_log() {
        let line = "2026-01-29T17:54:45Z loadtxoutset progress (50%, 10000 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, Some(50.0));
    }

    #[test]
    fn test_parse_loading_progress_100_percent() {
        let line = "2026-01-29T17:54:45Z [snapshot] completed (100%, 20000 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, Some(100.0));
    }

    #[test]
    fn test_parse_loading_progress_0_percent() {
        let line = "2026-01-29T17:54:45Z [snapshot] starting (0%, 0 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, Some(0.0));
    }

    #[test]
    fn test_parse_loading_progress_no_match() {
        let line = "2026-01-29T17:54:45Z Some other log message";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, None);
    }

    #[test]
    fn test_parse_loading_progress_invalid_percentage() {
        // Percentage outside valid range should not match
        let line = "2026-01-29T17:54:45Z [snapshot] error (150%, 20000 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, None);
    }

    #[test]
    fn test_parse_loading_progress_negative_percentage() {
        let line = "2026-01-29T17:54:45Z [snapshot] error (-5%, 0 MB)";
        let progress = parse_loading_progress(line);
        assert_eq!(progress, None);
    }
}
