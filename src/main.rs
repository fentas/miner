#![allow(dead_code)] // Allow unused code during development

mod config;
mod mining;
mod stratum;
mod bitcoin;
mod web;
mod stats;
mod gpu;
mod idle;
mod tui;

use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::Parser;
use colored::Colorize;
use tracing::{info, warn, error, Level};
use tracing_subscriber::FmtSubscriber;

use config::{Config, LogFormat};
use mining::{Coordinator, SimdLevel};
use idle::IdleMonitor;
use stats::{HashrateCalculator, format_hashrate, format_duration};
use tui::{TuiState, TuiRunner, TuiCommand};

/// BTC Miner - Bitcoin solo mining with Stratum V1/V2 support
#[derive(Parser, Debug)]
#[command(name = "btcminer", version, about)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "btcminer.toml")]
    config: String,

    /// Override number of mining threads
    #[arg(short, long)]
    threads: Option<usize>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// Disable TUI dashboard (use continuous logging instead)
    #[arg(long)]
    no_hud: bool,

    /// Log output format (text or json)
    #[arg(long, value_parser = ["text", "json"])]
    log_format: Option<String>,
}

/// Convert compact bits format to difficulty
fn bits_to_difficulty(bits: u32) -> f64 {
    let exp = (bits >> 24) as i32;
    let mant = (bits & 0x007fffff) as f64;
    if mant == 0.0 {
        return 0.0;
    }
    // Target = mant * 256^(exp-3)
    // Difficulty = diff1_target / target
    // diff1_target = 0xFFFF * 256^26 (when exp=29, mant=0xFFFF for difficulty 1)
    // Simplified: difficulty = 0xFFFF * 256^26 / (mant * 256^(exp-3))
    //           = 0xFFFF * 256^(26 - exp + 3) / mant
    //           = 0xFFFF * 256^(29 - exp) / mant
    let diff1_mant: f64 = 0xFFFF as f64;
    let pow_diff = (29 - exp) as f64;
    (diff1_mant / mant) * (256.0_f64).powf(pow_diff)
}

// Main entry point is excluded from coverage as it requires
// full program execution which is not suitable for unit testing
#[tokio::main]
#[cfg(not(tarpaulin_include))]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load configuration
    let mut cfg = Config::from_file(&args.config)?;

    // Override settings from CLI
    if let Some(threads) = args.threads {
        cfg.mining.threads = threads;
    }
    if args.no_hud {
        cfg.logging.hud = false;
    }
    if let Some(ref format) = args.log_format {
        cfg.logging.format = match format.as_str() {
            "json" => LogFormat::Json,
            _ => LogFormat::Text,
        };
    }

    // Configure colored output
    if args.no_color || !cfg.logging.colors {
        colored::control::set_override(false);
    }

    // Detect SIMD capabilities
    let simd_level = SimdLevel::detect();

    // Check if TUI should be used
    let use_tui = cfg.logging.hud && !args.debug && cfg.logging.format == LogFormat::Text;

    if use_tui {
        run_with_tui(cfg, simd_level).await
    } else {
        run_with_logging(cfg, args.debug, simd_level).await
    }
}

/// Run with TUI dashboard
async fn run_with_tui(cfg: Config, simd_level: SimdLevel) -> anyhow::Result<()> {
    // Create shared TUI state
    let tui_state = Arc::new(TuiState::new());

    // Set initial state
    tui_state.set_simd_level(simd_level.name());
    tui_state.set_worker_count(cfg.effective_threads());

    let mode_str = if cfg.is_solo_mode() {
        "True Solo (direct)"
    } else if cfg.is_sv2_mode() {
        "Stratum V2 (SRI)"
    } else {
        "Stratum V1 (pool)"
    };
    tui_state.set_mode(mode_str);

    if let Some(addr) = cfg.pool.effective_address() {
        tui_state.set_pool_address(&addr);
    }

    // Set GPU info - track configured vs available separately
    tui_state.set_gpu_configured(cfg.mining.enable_gpu);
    if cfg.mining.enable_gpu {
        let gpu_manager = gpu::GpuManager::new();
        let devices: Vec<String> = gpu_manager.devices()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        if !devices.is_empty() {
            tui_state.set_gpu_available(true);
            tui_state.log_info(format!("GPU: {} device(s) detected", devices.len()));
            for (i, name) in devices.iter().enumerate() {
                tui_state.log_info(format!("  GPU {}: {}", i, name));
            }
            tui_state.set_gpu_devices(devices);
        } else {
            tui_state.set_gpu_available(false);
            tui_state.log_warn("GPU enabled but no OpenCL devices found - using CPU only");
        }
    } else {
        tui_state.set_gpu_available(false);
    }

    tui_state.log_info(format!("Loading configuration from btcminer.toml"));
    tui_state.log_info(format!("Mining with {} CPU threads", cfg.effective_threads()));
    tui_state.log_info(format!("SIMD level: {}", simd_level.name()));

    // Start TUI in a separate thread
    let tui_state_clone = Arc::clone(&tui_state);
    let (tui_runner, mut cmd_rx) = TuiRunner::new(tui_state_clone);

    let tui_handle = std::thread::spawn(move || {
        if let Err(e) = tui_runner.run() {
            eprintln!("TUI error: {}", e);
        }
    });

    // Determine mining mode and run appropriate handler
    let solo_mode = cfg.is_solo_mode();
    let sv2_mode = cfg.is_sv2_mode();

    let mining_result = if solo_mode {
        tui_state.log_info("Mode: True Solo (direct to network)");
        run_solo_mode_with_tui(cfg, Arc::clone(&tui_state), &mut cmd_rx).await
    } else if sv2_mode {
        tui_state.log_info("Mode: Stratum V2 (SRI proxy)");
        run_sv2_mode_with_tui(cfg, Arc::clone(&tui_state), &mut cmd_rx).await
    } else {
        tui_state.log_info("Mode: Stratum V1 (pool)");
        run_pool_mode_with_tui(cfg, Arc::clone(&tui_state), &mut cmd_rx).await
    };

    // Signal TUI to quit
    tui_state.set_quit();

    // Wait for TUI thread to finish
    let _ = tui_handle.join();

    mining_result
}

/// Run with traditional logging (no TUI)
async fn run_with_logging(cfg: Config, debug: bool, simd_level: SimdLevel) -> anyhow::Result<()> {
    // Initialize logging based on format
    let log_level = if debug { Level::DEBUG } else { Level::INFO };

    if cfg.logging.format == LogFormat::Json {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(log_level)
            .json()
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    } else {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(log_level)
            .with_target(false)
            .compact()
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }

    // Print banner
    print_banner();

    println!(
        "{}",
        format!("  SIMD: {}", simd_level.name()).bright_cyan()
    );
    println!();

    info!("Loading configuration from btcminer.toml");
    info!("Mining with {} CPU threads", cfg.effective_threads());

    // Determine mining mode
    let solo_mode = cfg.is_solo_mode();
    let sv2_mode = cfg.is_sv2_mode();

    if solo_mode {
        print_mode("TRUE SOLO", "direct to network");
        run_solo_mode(cfg).await
    } else if sv2_mode {
        print_mode("STRATUM V2", "SRI proxy");
        run_sv2_mode(cfg).await
    } else {
        print_mode("STRATUM V1", "pool mining");
        run_pool_mode(cfg).await
    }
}

/// Solo mode mining with TUI
async fn run_solo_mode_with_tui(
    cfg: Config,
    tui_state: Arc<TuiState>,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TuiCommand>,
) -> anyhow::Result<()> {
    use bitcoin::BitcoindManager;

    tui_state.set_pool_address("Bitcoin Network (direct)");

    // Setup Bitcoin Core with all configured options
    let mut bitcoind = BitcoindManager::new()
        .with_prune(cfg.bitcoind.prune)
        .with_dbcache(cfg.bitcoind.dbcache)
        .with_assumeutxo(cfg.bitcoind.assumeutxo)
        .with_assumeutxo_magnet(cfg.bitcoind.assumeutxo_magnet.clone());

    if cfg.bitcoind.managed {
        if !bitcoind.is_installed() {
            tui_state.log_info("Bitcoin Core not found, downloading...");
            bitcoind.download_and_install().await?;
        } else if bitcoind.needs_upgrade() {
            let old_ver = bitcoind.installed_version().unwrap_or_default();
            tui_state.log_info(format!("Upgrading Bitcoin Core from v{}...", old_ver));
            bitcoind.download_and_install().await?;
        }

        // Download AssumeUTXO snapshot if enabled and not already present
        tracing::info!("AssumeUTXO check: enabled={}", cfg.bitcoind.assumeutxo);
        if cfg.bitcoind.assumeutxo && !bitcoind.has_assumeutxo_snapshot() {
            tui_state.log_info("AssumeUTXO enabled - downloading UTXO snapshot...");
            tui_state.set_downloading(true, "AssumeUTXO");

            let tui_state_clone = Arc::clone(&tui_state);
            bitcoind.download_assumeutxo_with_progress(move |bytes, total| {
                tui_state_clone.set_download_progress(bytes, total);
            }).await?;

            tui_state.set_downloading(false, "");
            tui_state.log_success("AssumeUTXO snapshot downloaded");
        } else if cfg.bitcoind.assumeutxo {
            tui_state.log_info("AssumeUTXO snapshot already present");
        }

        bitcoind.write_config()?;
        bitcoind.start()?;

        tui_state.log_info("Starting Bitcoin Core...");
        tui_state.set_bitcoin_managed(true);
        tui_state.set_syncing(true, 0.0);

        // Start tailing Bitcoin Core debug.log in background
        let debug_log_path = bitcoind.debug_log_path();
        let tui_for_logs = tui_state.clone();
        tokio::spawn(async move {
            tail_bitcoin_log(debug_log_path, tui_for_logs).await;
        });

        // Load AssumeUTXO if enabled (after Bitcoin Core starts)
        if cfg.bitcoind.assumeutxo && bitcoind.has_assumeutxo_snapshot() {
            tui_state.log_info("Waiting for headers sync before loading AssumeUTXO...");
            let tui = tui_state.clone();
            tui.set_downloading(true, "Headers (AssumeUTXO)");
            match bitcoind.load_assumeutxo_with_progress(move |progress| {
                use crate::bitcoin::LoadProgress;
                match progress {
                    LoadProgress::SyncingHeaders { current, target } => {
                        tui.set_download_progress(current, target);
                    }
                    LoadProgress::LoadingSnapshot { elapsed_secs, progress_pct } => {
                        // Encode: total = (elapsed_secs << 16) | (pct * 100, or 0xFFFF if unknown)
                        let pct_encoded = progress_pct
                            .map(|p| (p * 100.0) as u64)
                            .unwrap_or(0xFFFF);
                        let encoded_total = (elapsed_secs << 16) | pct_encoded;
                        tui.set_downloading(true, "Loading AssumeUTXO Snapshot");
                        tui.set_download_progress(0, encoded_total);
                    }
                    LoadProgress::AlreadySynced => {
                        // Nothing to do, will return soon
                    }
                }
            }).await {
                Ok(_) => {
                    tui_state.set_downloading(false, "");
                    tui_state.log_success("AssumeUTXO snapshot loaded - fast sync enabled");
                }
                Err(e) => {
                    tui_state.set_downloading(false, "");
                    tui_state.log_warn(format!("AssumeUTXO load failed: {}", e));
                }
            }
        }

        // Wait for sync
        loop {
            // Check for quit command
            if let Ok(TuiCommand::Quit) = cmd_rx.try_recv() {
                return Ok(());
            }

            match bitcoind.get_sync_status().await {
                Ok(status) => {
                    if !tui_state.is_connected() {
                        tui_state.set_connected(true);
                        tui_state.log_success("Bitcoin Core RPC connected");
                    }

                    if status.synced {
                        tui_state.set_syncing(false, 100.0);
                        tui_state.set_block_height(status.blocks as u64);
                        tui_state.log_success(format!("Synced at height {}", status.blocks));
                        break;
                    } else {
                        tui_state.set_syncing(true, status.progress);
                        tui_state.set_block_height(status.blocks as u64);
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("warming up") || msg.contains("Loading") {
                        tui_state.log_info(msg);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    // Wait for peer connections before starting to mine
    tui_state.log_info("Waiting for peer connections...");
    let rpc = bitcoind.rpc().unwrap();
    loop {
        match rpc.get_connection_count().await {
            Ok(count) if count > 0 => {
                tui_state.log_success(format!("Connected to {} peer(s)", count));
                tui_state.set_peer_count(count);
                break;
            }
            Ok(_) => {
                // No peers yet, wait
            }
            Err(e) => {
                tui_state.log_warn(format!("Peer check failed: {}", e));
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Start mining coordinator
    let thread_count = cfg.effective_threads();
    let coordinator = Coordinator::new(thread_count);

    // Setup idle monitor
    let idle_monitor = if cfg.idle.enabled {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(cfg.idle.timeout);
        tui_state.log_info(format!("Idle monitor enabled ({}s)", cfg.idle.timeout));
        monitor.start();
        Some(monitor)
    } else {
        None
    };

    let mut hashrate_calc = HashrateCalculator::new(60);
    let stats_interval = Duration::from_millis(500);
    let mut last_stats = Instant::now();

    // Wait for first valid block template before starting mining
    tui_state.log_info("Waiting for block template...");
    loop {
        match rpc.get_block_template(&cfg.pool.payout_address).await {
            Ok(template) => {
                tui_state.set_block_height(template.height as u64);

                // Calculate and set difficulty from bits
                if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
                    let difficulty = bits_to_difficulty(bits);
                    tui_state.set_difficulty(difficulty as u64);
                }

                let mut job = mining::Job::new(format!("solo-{}", template.height));
                job.version = template.version;
                job.ntime = template.timestamp as u32;

                // Parse prev_block_hash
                if let Ok(hash_bytes) = hex::decode(&template.prev_block_hash) {
                    if hash_bytes.len() == 32 {
                        let mut prev_hash = [0u8; 32];
                        prev_hash.copy_from_slice(&hash_bytes);
                        prev_hash.reverse();
                        job.prev_hash = prev_hash;
                    }
                }

                // Parse nbits
                if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
                    job.nbits = bits;
                }

                // Parse target
                if let Ok(target_bytes) = hex::decode(&template.target) {
                    if target_bytes.len() == 32 {
                        let mut target = [0u8; 32];
                        target.copy_from_slice(&target_bytes);
                        job.target = target;
                        job.network_target = Some(target);
                    }
                }

                job.clean_jobs = true;
                coordinator.update_job(job);
                coordinator.start();
                tui_state.log_success("Mining started");
                break;
            }
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("-9") && !err_str.contains("-10") {
                    tui_state.log_warn(format!("Waiting for template: {}", e));
                }
                // Update peer count while waiting
                if let Ok(count) = rpc.get_connection_count().await {
                    tui_state.set_peer_count(count);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    loop {
        // Check for TUI commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TuiCommand::Quit => {
                    tui_state.log_info("Shutting down...");
                    return Ok(());
                }
                TuiCommand::TogglePause => {
                    let paused = tui_state.toggle_pause();
                    if paused {
                        coordinator.stop();
                        tui_state.log_warn("Mining paused");
                    } else {
                        coordinator.start();
                        tui_state.log_success("Mining resumed");
                    }
                }
                TuiCommand::ClearLogs => {}
                TuiCommand::ToggleBitcoinLogs => {}
            }
        }

        // Check idle state
        if let Some(ref monitor) = idle_monitor {
            if cfg.idle.pause_on_active && !monitor.is_idle() && !tui_state.is_paused() {
                coordinator.stop();
            } else if !tui_state.is_paused() {
                coordinator.start();
            }
        }

        // Check peer count before attempting block template
        let rpc = bitcoind.rpc().unwrap();
        let peer_count = rpc.get_connection_count().await.unwrap_or(0);
        tui_state.set_peer_count(peer_count);

        if peer_count == 0 {
            // No peers - pause mining and wait
            coordinator.stop();
            // Only log once per disconnect
            static LOGGED_NO_PEERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !LOGGED_NO_PEERS.swap(true, std::sync::atomic::Ordering::Relaxed) {
                tui_state.log_warn("No peers connected - waiting for network...");
            }
        } else {
            // Reset the no-peers log flag when we have peers
            static LOGGED_NO_PEERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            LOGGED_NO_PEERS.store(false, std::sync::atomic::Ordering::Relaxed);

            // Ensure mining is running (if not paused)
            if !tui_state.is_paused() {
                coordinator.start();
            }

            // Get block template
            match rpc.get_block_template(&cfg.pool.payout_address).await {
                Ok(template) => {
                    tui_state.set_block_height(template.height as u64);

                    // Calculate and set difficulty from bits
                    if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
                        let difficulty = bits_to_difficulty(bits);
                        tui_state.set_difficulty(difficulty as u64);
                    }

                    let mut job = mining::Job::new(format!("solo-{}", template.height));
                    job.version = template.version;
                    job.ntime = template.timestamp as u32;

                    // Parse prev_block_hash (hex string to bytes, reversed for little-endian)
                    if let Ok(hash_bytes) = hex::decode(&template.prev_block_hash) {
                        if hash_bytes.len() == 32 {
                            let mut prev_hash = [0u8; 32];
                            prev_hash.copy_from_slice(&hash_bytes);
                            prev_hash.reverse(); // Bitcoin uses little-endian
                            job.prev_hash = prev_hash;
                        }
                    }

                    // Parse nbits (compact difficulty target)
                    if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
                        job.nbits = bits;
                    }

                    // Parse target (hex string to bytes)
                    if let Ok(target_bytes) = hex::decode(&template.target) {
                        if target_bytes.len() == 32 {
                            let mut target = [0u8; 32];
                            target.copy_from_slice(&target_bytes);
                            job.target = target;
                            job.network_target = Some(target);
                        }
                    }

                    job.clean_jobs = true;
                    coordinator.update_job(job);
                }
                Err(e) => {
                    // Log all errors for debugging
                    tui_state.log_warn(format!("Block template error: {}", e));
                }
            }
        }

        // Update stats
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();
            hashrate_calc.add_sample(stats.hashes);
            let hashrate = hashrate_calc.hashrate();

            tui_state.set_hashrate(hashrate);
            tui_state.set_total_hashes(stats.hashes);
            tui_state.set_shares(stats.shares_accepted, stats.shares_rejected);

            last_stats = Instant::now();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// SV2 mode mining with TUI
async fn run_sv2_mode_with_tui(
    cfg: Config,
    tui_state: Arc<TuiState>,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TuiCommand>,
) -> anyhow::Result<()> {
    use stratum::StratumV2Client;

    let proxy_addr = format!("{}:{}", cfg.sv2.proxy_host, cfg.sv2.proxy_port);
    tui_state.set_pool_address(&proxy_addr);
    tui_state.log_info(format!("Connecting to SV2 proxy at {}", proxy_addr));

    let mut client = StratumV2Client::new();
    client.connect(&cfg.sv2.proxy_host, cfg.sv2.proxy_port).await?;

    tui_state.set_connected(true);
    tui_state.log_success("Connected to SV2 proxy");

    // Start mining coordinator
    let thread_count = cfg.effective_threads();
    let coordinator = Coordinator::new(thread_count);
    coordinator.start();

    // Setup idle monitor
    let idle_monitor = if cfg.idle.enabled {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(cfg.idle.timeout);
        tui_state.log_info(format!("Idle monitor enabled ({}s)", cfg.idle.timeout));
        monitor.start();
        Some(monitor)
    } else {
        None
    };

    let mut hashrate_calc = HashrateCalculator::new(60);
    let stats_interval = Duration::from_millis(500);
    let mut last_stats = Instant::now();

    tui_state.log_success("Mining started");

    loop {
        // Check for TUI commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TuiCommand::Quit => {
                    tui_state.log_info("Shutting down...");
                    return Ok(());
                }
                TuiCommand::TogglePause => {
                    let paused = tui_state.toggle_pause();
                    if paused {
                        coordinator.stop();
                        tui_state.log_warn("Mining paused");
                    } else {
                        coordinator.start();
                        tui_state.log_success("Mining resumed");
                    }
                }
                TuiCommand::ClearLogs => {}
                TuiCommand::ToggleBitcoinLogs => {}
            }
        }

        // Check idle state
        if let Some(ref monitor) = idle_monitor {
            if cfg.idle.pause_on_active && !monitor.is_idle() && !tui_state.is_paused() {
                coordinator.stop();
            } else if !tui_state.is_paused() {
                coordinator.start();
            }
        }

        // Receive job from SV2 proxy
        match client.recv_job().await {
            Ok(job) => {
                tui_state.log_info(format!("New job: {}", job.id));
                coordinator.update_job(job);
            }
            Err(e) => {
                tui_state.set_connected(false);
                tui_state.log_error(format!("SV2 error: {}", e));
                break;
            }
        }

        // Update stats
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();
            hashrate_calc.add_sample(stats.hashes);
            let hashrate = hashrate_calc.hashrate();

            tui_state.set_hashrate(hashrate);
            tui_state.set_total_hashes(stats.hashes);
            tui_state.set_shares(stats.shares_accepted, stats.shares_rejected);

            last_stats = Instant::now();
        }
    }

    Ok(())
}

/// Pool mode mining with TUI state updates
async fn run_pool_mode_with_tui(
    cfg: Config,
    tui_state: Arc<TuiState>,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<TuiCommand>,
) -> anyhow::Result<()> {
    use stratum::StratumV1Client;

    let pool_address = cfg.pool.effective_address()
        .ok_or_else(|| anyhow::anyhow!("No pool address configured"))?;

    tui_state.log_info(format!("Connecting to {}", pool_address));

    let mut client = StratumV1Client::new();
    client.connect(&pool_address)?;
    client.subscribe()?;
    client.authorize(&cfg.pool.payout_address, &cfg.pool.password)?;

    tui_state.set_connected(true);
    tui_state.log_success("Connected and authorized with pool");

    // Start mining coordinator
    let thread_count = cfg.effective_threads();
    let mut coordinator = Coordinator::new(thread_count);
    coordinator.start();

    // Setup idle monitor if enabled
    let idle_monitor = if cfg.idle.enabled {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(cfg.idle.timeout);
        tui_state.log_info(format!("Idle monitor enabled (timeout: {}s)", cfg.idle.timeout));
        monitor.start();
        Some(monitor)
    } else {
        None
    };

    // Start web server if enabled
    let web_state = if cfg.web.port > 0 {
        let state = Arc::new(web::WebState::new(pool_address.clone(), thread_count));
        let state_clone = Arc::clone(&state);
        let web_dir = cfg.web.dir.clone();
        let web_port = cfg.web.port;

        tokio::spawn(async move {
            if let Err(e) = web::start_server(state_clone, &web_dir, web_port).await {
                eprintln!("Web server error: {}", e);
            }
        });

        tui_state.log_info(format!("Web dashboard at http://localhost:{}", cfg.web.port));
        Some(state)
    } else {
        None
    };

    // Setup stats
    let mut hashrate_calc = HashrateCalculator::new(60);
    let stats_interval = Duration::from_millis(500); // Update TUI more frequently
    let mut last_stats = Instant::now();

    tui_state.log_success("Mining started");

    loop {
        // Check for TUI commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TuiCommand::Quit => {
                    tui_state.log_info("Shutting down...");
                    return Ok(());
                }
                TuiCommand::TogglePause => {
                    let paused = tui_state.toggle_pause();
                    if paused {
                        coordinator.stop();
                        tui_state.log_warn("Mining paused");
                    } else {
                        coordinator.start();
                        tui_state.log_success("Mining resumed");
                    }
                }
                TuiCommand::ClearLogs => {}
                TuiCommand::ToggleBitcoinLogs => {}
            }
        }

        // Check idle state
        if let Some(ref monitor) = idle_monitor {
            let is_idle = monitor.is_idle();
            if cfg.idle.pause_on_active && !is_idle && !tui_state.is_paused() {
                coordinator.stop();
                if let Some(ref state) = web_state {
                    state.set_paused(true).await;
                }
            } else if !tui_state.is_paused() {
                coordinator.start();
                if let Some(ref state) = web_state {
                    state.set_paused(false).await;
                }
            }
        }

        // Poll for notifications (jobs, difficulty changes)
        match client.poll_notification() {
            Ok(Some(stratum::Notification::Job(job))) => {
                tui_state.log_info(format!("New job: {}", job.id));
                coordinator.update_job(job);
            }
            Ok(Some(stratum::Notification::Difficulty(diff))) => {
                tui_state.set_difficulty(diff as u64);
                tui_state.log_info(format!("Difficulty: {}", diff));
            }
            Ok(None) => {}
            Err(e) => {
                tui_state.set_connected(false);
                tui_state.log_error(format!("Stratum error: {}", e));
                break;
            }
        }

        // Check for found shares
        while let Some(share) = coordinator.try_recv_share() {
            let extranonce2 = hex::encode(&[0u8; 4]);
            let ntime = hex::encode(share.nonce.to_be_bytes());
            let nonce = hex::encode(share.nonce.to_le_bytes());

            match client.submit(
                &cfg.pool.payout_address,
                &share.job_id,
                &extranonce2,
                &ntime,
                &nonce,
            ) {
                Ok(true) => {
                    coordinator.stats().accept_share();
                    tui_state.record_share();
                    tui_state.log_success("Share accepted!");
                    if cfg.logging.system_bell {
                        print!("\x07");
                    }
                }
                Ok(false) => {
                    coordinator.stats().reject_share();
                    tui_state.log_error("Share rejected");
                }
                Err(e) => {
                    tui_state.log_error(format!("Submit error: {}", e));
                }
            }
        }

        // Update stats
        if last_stats.elapsed() >= stats_interval {
            let stats = coordinator.stats().snapshot();
            hashrate_calc.add_sample(stats.hashes);
            let hashrate = hashrate_calc.hashrate();

            tui_state.set_hashrate(hashrate);
            tui_state.set_total_hashes(stats.hashes);
            tui_state.set_shares(stats.shares_accepted, stats.shares_rejected);

            // Update web state
            if let Some(ref state) = web_state {
                state.update_stats(stats, hashrate).await;
            }

            last_stats = Instant::now();
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Ok(())
}

fn print_mode(mode: &str, detail: &str) {
    println!(
        "{} {} {}",
        "Mode:".bright_white(),
        mode.bright_green().bold(),
        format!("({})", detail).dimmed()
    );
}

/// Tail Bitcoin Core debug.log and send entries to TUI
async fn tail_bitcoin_log(path: std::path::PathBuf, tui: Arc<TuiState>) {
    use std::io::{BufRead, BufReader, Seek, SeekFrom};
    use tui::{LogEntry, LogLevel};

    // Wait for file to exist
    while !path.exists() {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Start from ~50KB back to show recent history
    let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let mut position = file_size.saturating_sub(50 * 1024);

    // If starting mid-file, skip to next newline to avoid partial line
    if position > 0 {
        if let Ok(file) = std::fs::File::open(&path) {
            let mut reader = BufReader::new(file);
            if reader.seek(SeekFrom::Start(position)).is_ok() {
                let mut discard = String::new();
                if reader.read_line(&mut discard).is_ok() {
                    position = reader.stream_position().unwrap_or(position);
                }
            }
        }
    }

    loop {
        if let Ok(file) = std::fs::File::open(&path) {
            let mut reader = BufReader::new(file);
            if reader.seek(SeekFrom::Start(position)).is_ok() {
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            let line = line.trim();
                            if !line.is_empty() {
                                // Parse Bitcoin Core format: "2026-01-29T17:54:45Z [category] message"
                                let (timestamp, msg) = if line.len() > 21 && line.chars().nth(10) == Some('T') {
                                    // Extract time part (HH:MM:SS) from ISO timestamp
                                    let time = &line[11..19];
                                    (time, &line[21..])
                                } else {
                                    ("??:??:??", line)
                                };

                                // Parse log level from Bitcoin Core format
                                let level = if line.contains("error") || line.contains("Error") {
                                    LogLevel::Error
                                } else if line.contains("warning") || line.contains("Warning") {
                                    LogLevel::Warn
                                } else {
                                    LogLevel::Info
                                };

                                let entry = LogEntry::with_timestamp(level, msg, timestamp);
                                tui.log_bitcoin(entry);
                            }
                        }
                        Err(_) => break,
                    }
                }
                position = reader.stream_position().unwrap_or(position);
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Run in true solo mode (no pool, direct to Bitcoin network)
async fn run_solo_mode(cfg: Config) -> anyhow::Result<()> {
    use bitcoin::BitcoindManager;

    // Setup Bitcoin Core
    let mut bitcoind = BitcoindManager::new()
        .with_prune(cfg.bitcoind.prune)
        .with_dbcache(cfg.bitcoind.dbcache)
        .with_assumeutxo(cfg.bitcoind.assumeutxo)
        .with_assumeutxo_magnet(cfg.bitcoind.assumeutxo_magnet.clone());

    if cfg.bitcoind.managed {
        // Auto-manage Bitcoin Core
        if !bitcoind.is_installed() {
            info!("Bitcoin Core not found, downloading...");
            bitcoind.download_and_install().await?;
        } else if bitcoind.needs_upgrade() {
            let old_ver = bitcoind.installed_version().unwrap_or_default();
            info!("Upgrading Bitcoin Core from v{} to latest...", old_ver);
            bitcoind.download_and_install().await?;
        }

        // Download AssumeUTXO snapshot if enabled and not already present
        if cfg.bitcoind.assumeutxo && !bitcoind.has_assumeutxo_snapshot() {
            info!("AssumeUTXO enabled - downloading UTXO snapshot via BitTorrent...");
            bitcoind.download_assumeutxo_with_progress(|bytes, total| {
                let percent = if total > 0 { (bytes as f64 / total as f64) * 100.0 } else { 0.0 };
                print!("\r⏳ Downloading AssumeUTXO snapshot: {:.1}%", percent);
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }).await?;
            println!("\n✓ AssumeUTXO snapshot downloaded");
        } else if cfg.bitcoind.assumeutxo {
            info!("AssumeUTXO snapshot already present");
        }

        bitcoind.write_config()?;
        bitcoind.start()?;

        // Wait for Bitcoin Core to start and sync
        println!(
            "{} {}",
            "⏳".bright_yellow(),
            "Starting Bitcoin Core...".bright_white()
        );

        // Load AssumeUTXO if enabled (after Bitcoin Core starts)
        if cfg.bitcoind.assumeutxo && bitcoind.has_assumeutxo_snapshot() {
            println!(
                "{} {}",
                "⏳".bright_yellow(),
                "Waiting for headers sync before loading AssumeUTXO...".bright_white()
            );
            let last_progress = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let last_elapsed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
            let loading_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let last_progress_clone = last_progress.clone();
            let last_elapsed_clone = last_elapsed.clone();
            let loading_started_clone = loading_started.clone();
            match bitcoind.load_assumeutxo_with_progress(move |progress| {
                use crate::bitcoin::LoadProgress;
                match progress {
                    LoadProgress::SyncingHeaders { current, target } => {
                        let progress_pct = (current as f64 / target as f64 * 100.0) as u64;
                        let last = last_progress_clone.load(std::sync::atomic::Ordering::Relaxed);
                        // Only print when progress changes
                        if progress_pct > last {
                            last_progress_clone.store(progress_pct, std::sync::atomic::Ordering::Relaxed);
                            let bar_width: usize = 30;
                            let filled = (progress_pct as usize * bar_width / 100).min(bar_width);
                            let empty = bar_width.saturating_sub(filled);
                            print!(
                                "\r{} [{}{}] {:.1}% ({}/{})    ",
                                "⏳".bright_yellow(),
                                "█".repeat(filled).bright_cyan(),
                                "░".repeat(empty).dimmed(),
                                progress_pct,
                                current.to_string().bright_cyan(),
                                target.to_string().dimmed()
                            );
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                    }
                    LoadProgress::LoadingSnapshot { elapsed_secs, progress_pct } => {
                        let was_started = loading_started_clone.swap(true, std::sync::atomic::Ordering::Relaxed);
                        let prev_elapsed = last_elapsed_clone.load(std::sync::atomic::Ordering::Relaxed);

                        // Print new line on first call
                        if !was_started {
                            println!();
                        }
                        // Update when elapsed time changes or percentage changes
                        if elapsed_secs != prev_elapsed || !was_started {
                            last_elapsed_clone.store(elapsed_secs, std::sync::atomic::Ordering::Relaxed);
                            let mins = elapsed_secs / 60;
                            let secs = elapsed_secs % 60;
                            let time_str = if mins > 0 {
                                format!("{}m {}s", mins, secs)
                            } else {
                                format!("{}s", secs)
                            };

                            // Show progress bar if percentage is available
                            if let Some(pct) = progress_pct {
                                let bar_width: usize = 30;
                                let filled = (pct as usize * bar_width / 100).min(bar_width);
                                let empty = bar_width.saturating_sub(filled);
                                print!(
                                    "\r{} [{}{}] {:.1}% ({} elapsed)   ",
                                    "⏳".bright_yellow(),
                                    "█".repeat(filled).bright_yellow(),
                                    "░".repeat(empty).dimmed(),
                                    pct,
                                    time_str.bright_cyan()
                                );
                            } else {
                                print!(
                                    "\r{} Loading AssumeUTXO snapshot... ({} elapsed)   ",
                                    "⏳".bright_yellow(),
                                    time_str.bright_cyan()
                                );
                            }
                            use std::io::Write;
                            std::io::stdout().flush().ok();
                        }
                    }
                    LoadProgress::AlreadySynced => {
                        // Will return soon
                    }
                }
            }).await {
                Ok(_) => {
                    println!();
                    println!("{} {}", "✓".bright_green(), "AssumeUTXO snapshot loaded - fast sync enabled".bright_white());
                }
                Err(e) => {
                    println!();
                    warn!("AssumeUTXO load failed: {}", e);
                }
            }
        }

        let start = std::time::Instant::now();
        let mut last_message = String::new();
        let mut rpc_connected = false;

        loop {
            match bitcoind.get_sync_status().await {
                Ok(status) => {
                    if !rpc_connected {
                        println!(
                            "{} {}",
                            "✓".bright_green(),
                            "Bitcoin Core RPC connected".bright_white()
                        );
                        rpc_connected = true;
                    }

                    if status.synced {
                        println!(
                            "{} {} {}",
                            "✓".bright_green(),
                            "Bitcoin Core synced at height".bright_white(),
                            status.blocks.to_string().bright_cyan()
                        );
                        break;
                    } else {
                        // Show sync progress with progress bar
                        let bar_width: usize = 30;
                        let filled = (status.progress / 100.0 * bar_width as f64) as usize;
                        let empty = bar_width.saturating_sub(filled);
                        let bar = format!(
                            "[{}{}]",
                            "█".repeat(filled).bright_cyan(),
                            "░".repeat(empty).dimmed()
                        );
                        println!(
                            "{} {} {:.2}% ({}/{})",
                            "⏳".bright_yellow(),
                            bar,
                            status.progress,
                            status.blocks.to_string().bright_cyan(),
                            status.headers.to_string().dimmed()
                        );
                    }
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_secs();
                    if elapsed > 120 {
                        return Err(anyhow::anyhow!("Bitcoin Core failed to start after 120s: {}", e));
                    }

                    // Show status message (deduplicated)
                    let msg = e.to_string();
                    let display_msg = if msg.contains("warming up") || msg.contains("Loading") || msg.contains("Verifying") {
                        msg.clone()
                    } else if msg.contains("Connection refused") || msg.contains("connection error") {
                        format!("Waiting for Bitcoin Core to start... ({}s)", elapsed)
                    } else {
                        format!("{}... ({}s)", msg.chars().take(50).collect::<String>(), elapsed)
                    };

                    if display_msg != last_message {
                        println!(
                            "{} {}",
                            "⏳".bright_yellow(),
                            display_msg.dimmed()
                        );
                        last_message = display_msg;
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    // For non-managed mode, check sync status
    if !cfg.bitcoind.managed {
        loop {
            match bitcoind.get_sync_status().await {
                Ok(status) if status.synced => {
                    println!(
                        "{} {} {}",
                        "✓".bright_green(),
                        "Bitcoin Core synced at height".bright_white(),
                        status.blocks.to_string().bright_cyan()
                    );
                    break;
                }
                Ok(status) => {
                    println!(
                        "{} {:.2}% ({}/{})",
                        "Syncing:".bright_yellow(),
                        status.progress,
                        status.blocks,
                        status.headers
                    );
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to get sync status: {}", e));
                }
            }
        }
    }

    // Wait for peer connections before starting to mine
    println!(
        "{} {}",
        "⏳".bright_yellow(),
        "Waiting for peer connections...".bright_white()
    );
    let rpc = bitcoind.rpc().unwrap();
    loop {
        match rpc.get_connection_count().await {
            Ok(count) if count > 0 => {
                println!(
                    "{} {} {}",
                    "✓".bright_green(),
                    "Connected to".bright_white(),
                    format!("{} peer(s)", count).bright_cyan()
                );
                break;
            }
            Ok(_) => {
                // No peers yet, wait
            }
            Err(e) => {
                warn!("Peer check failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Create mining coordinator (but don't start yet)
    let thread_count = cfg.effective_threads();
    let coordinator = Coordinator::new(thread_count);

    // Setup idle monitor if enabled
    let idle_monitor = setup_idle_monitor(&cfg);

    // Setup stats
    let mut hashrate_calc = HashrateCalculator::new(60);
    let start_time = Instant::now();
    let stats_interval = Duration::from_secs(cfg.logging.stats_interval as u64);
    let mut last_stats = Instant::now();

    // Wait for first valid block template before starting mining
    println!(
        "{} {}",
        "⏳".bright_yellow(),
        "Waiting for block template...".bright_white()
    );
    loop {
        match rpc.get_block_template(&cfg.pool.payout_address).await {
            Ok(template) => {
                let mut job = mining::Job::new(format!("solo-{}", template.height));
                job.version = template.version;
                job.ntime = template.timestamp as u32;

                // Parse prev_block_hash (hex string to bytes, reversed for little-endian)
                if let Ok(hash_bytes) = hex::decode(&template.prev_block_hash) {
                    if hash_bytes.len() == 32 {
                        let mut prev_hash = [0u8; 32];
                        prev_hash.copy_from_slice(&hash_bytes);
                        prev_hash.reverse(); // Bitcoin uses little-endian
                        job.prev_hash = prev_hash;
                    }
                }

                // Parse nbits (compact difficulty target)
                if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
                    job.nbits = bits;
                }

                // Parse target (hex string to bytes)
                if let Ok(target_bytes) = hex::decode(&template.target) {
                    if target_bytes.len() == 32 {
                        let mut target = [0u8; 32];
                        target.copy_from_slice(&target_bytes);
                        job.target = target;
                        job.network_target = Some(target);
                    }
                }

                job.clean_jobs = true;
                coordinator.update_job(job);
                coordinator.start();
                print_mining_started();
                break;
            }
            Err(e) => {
                let err_str = e.to_string();
                // Only log non-transient errors
                if !err_str.contains("-9") && !err_str.contains("-10") {
                    warn!("Waiting for template: {}", e);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Main loop: get block templates and mine
    loop {
        // Check idle state
        if let Some(ref monitor) = idle_monitor {
            if cfg.idle.pause_on_active && !monitor.is_idle() {
                coordinator.stop();
            } else {
                coordinator.start();
            }
        }

        // Check peer count
        let peer_count = rpc.get_connection_count().await.unwrap_or(0);
        if peer_count == 0 {
            coordinator.stop();
            warn!("No peers connected - waiting for network...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        // Get block template
        let template = match rpc.get_block_template(&cfg.pool.payout_address).await {
            Ok(t) => t,
            Err(e) => {
                warn!("Failed to get block template: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Create mining job from template with all required fields
        let mut job = mining::Job::new(format!("solo-{}", template.height));
        job.version = template.version;
        job.ntime = template.timestamp as u32;

        // Parse prev_block_hash
        if let Ok(hash_bytes) = hex::decode(&template.prev_block_hash) {
            if hash_bytes.len() == 32 {
                let mut prev_hash = [0u8; 32];
                prev_hash.copy_from_slice(&hash_bytes);
                prev_hash.reverse();
                job.prev_hash = prev_hash;
            }
        }

        // Parse nbits
        if let Ok(bits) = u32::from_str_radix(&template.bits, 16) {
            job.nbits = bits;
        }

        // Parse target
        if let Ok(target_bytes) = hex::decode(&template.target) {
            if target_bytes.len() == 32 {
                let mut target = [0u8; 32];
                target.copy_from_slice(&target_bytes);
                job.target = target;
                job.network_target = Some(target);
            }
        }

        job.clean_jobs = true;
        coordinator.update_job(job);

        // Print stats periodically
        if last_stats.elapsed() >= stats_interval {
            print_stats(&coordinator, &mut hashrate_calc, start_time, &cfg);
            last_stats = Instant::now();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Run in Stratum V2 mode via SRI proxy
async fn run_sv2_mode(cfg: Config) -> anyhow::Result<()> {
    use stratum::StratumV2Client;

    let mut client = StratumV2Client::new();
    client.connect(&cfg.sv2.proxy_host, cfg.sv2.proxy_port).await?;

    println!(
        "{} {} {}:{}",
        "✓".bright_green(),
        "Connected to SV2 proxy at".bright_white(),
        cfg.sv2.proxy_host.bright_cyan(),
        cfg.sv2.proxy_port.to_string().bright_cyan()
    );

    // Start mining coordinator
    let thread_count = cfg.effective_threads();
    let coordinator = Coordinator::new(thread_count);
    coordinator.start();

    // Setup idle monitor if enabled
    let idle_monitor = setup_idle_monitor(&cfg);

    // Setup stats
    let mut hashrate_calc = HashrateCalculator::new(60);
    let start_time = Instant::now();
    let stats_interval = Duration::from_secs(cfg.logging.stats_interval as u64);
    let mut last_stats = Instant::now();

    print_mining_started();

    loop {
        // Check idle state
        if let Some(ref monitor) = idle_monitor {
            if cfg.idle.pause_on_active && !monitor.is_idle() {
                coordinator.stop();
            } else {
                coordinator.start();
            }
        }

        // Receive job from SV2 proxy
        match client.recv_job().await {
            Ok(job) => {
                print_new_job(&job.id);
                coordinator.update_job(job);
            }
            Err(e) => {
                error!("SV2 error: {}", e);
                break;
            }
        }

        // Print stats
        if last_stats.elapsed() >= stats_interval {
            print_stats(&coordinator, &mut hashrate_calc, start_time, &cfg);
            last_stats = Instant::now();
        }
    }

    Ok(())
}

/// Run in traditional Stratum V1 pool mode
async fn run_pool_mode(cfg: Config) -> anyhow::Result<()> {
    use stratum::StratumV1Client;

    let pool_address = cfg.pool.effective_address()
        .ok_or_else(|| anyhow::anyhow!("No pool address configured"))?;

    println!(
        "{} {}",
        "Connecting to:".bright_white(),
        pool_address.bright_cyan()
    );

    let mut client = StratumV1Client::new();
    client.connect(&pool_address)?;
    client.subscribe()?;
    client.authorize(&cfg.pool.payout_address, &cfg.pool.password)?;

    println!(
        "{} {}",
        "✓".bright_green(),
        "Authorized with pool".bright_white()
    );

    // Start mining coordinator
    let thread_count = cfg.effective_threads();
    let mut coordinator = Coordinator::new(thread_count);
    coordinator.start();

    // Setup idle monitor if enabled
    let idle_monitor = setup_idle_monitor(&cfg);

    // Start web server if enabled
    let web_state = if cfg.web.port > 0 {
        let state = Arc::new(web::WebState::new(pool_address.clone(), thread_count));
        let state_clone = Arc::clone(&state);
        let web_dir = cfg.web.dir.clone();
        let web_port = cfg.web.port;

        tokio::spawn(async move {
            if let Err(e) = web::start_server(state_clone, &web_dir, web_port).await {
                error!("Web server error: {}", e);
            }
        });

        Some(state)
    } else {
        None
    };

    // Setup stats
    let mut hashrate_calc = HashrateCalculator::new(60);
    let start_time = Instant::now();
    let stats_interval = Duration::from_secs(cfg.logging.stats_interval as u64);
    let mut last_stats = Instant::now();

    print_mining_started();

    loop {
        // Check idle state and update web state
        if let Some(ref monitor) = idle_monitor {
            let is_idle = monitor.is_idle();
            if cfg.idle.pause_on_active && !is_idle {
                coordinator.stop();
                if let Some(ref state) = web_state {
                    state.set_paused(true).await;
                }
            } else {
                coordinator.start();
                if let Some(ref state) = web_state {
                    state.set_paused(false).await;
                }
            }
        }

        // Poll for notifications (jobs, difficulty changes)
        match client.poll_notification() {
            Ok(Some(stratum::Notification::Job(job))) => {
                print_new_job(&job.id);
                coordinator.update_job(job);
            }
            Ok(Some(stratum::Notification::Difficulty(diff))) => {
                println!(
                    "{} {}",
                    "Difficulty:".bright_white(),
                    diff.to_string().bright_yellow()
                );
            }
            Ok(None) => {}
            Err(e) => {
                error!("Stratum error: {}", e);
                break;
            }
        }

        // Check for found shares
        while let Some(share) = coordinator.try_recv_share() {
            let extranonce2 = hex::encode(&[0u8; 4]);
            let ntime = hex::encode(share.nonce.to_be_bytes());
            let nonce = hex::encode(share.nonce.to_le_bytes());

            match client.submit(
                &cfg.pool.payout_address,
                &share.job_id,
                &extranonce2,
                &ntime,
                &nonce,
            ) {
                Ok(true) => {
                    coordinator.stats().accept_share();
                    print_share_accepted();
                    if cfg.logging.system_bell {
                        print!("\x07");
                    }
                }
                Ok(false) => {
                    coordinator.stats().reject_share();
                    print_share_rejected();
                }
                Err(e) => {
                    error!("Submit error: {}", e);
                }
            }
        }

        // Print stats
        if last_stats.elapsed() >= stats_interval {
            let hashrate = print_stats(&coordinator, &mut hashrate_calc, start_time, &cfg);

            // Update web state
            if let Some(ref state) = web_state {
                let stats = coordinator.stats().snapshot();
                state.update_stats(stats, hashrate).await;
            }

            last_stats = Instant::now();
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}

/// Setup idle monitor if enabled
fn setup_idle_monitor(cfg: &Config) -> Option<IdleMonitor> {
    if cfg.idle.enabled {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(cfg.idle.timeout);

        let env = monitor.environment();
        println!(
            "{} {} (timeout: {}s, env: {})",
            "Idle monitor:".bright_white(),
            "enabled".bright_green(),
            cfg.idle.timeout,
            env.name()
        );

        monitor.start();
        Some(monitor)
    } else {
        None
    }
}

/// Print stats with colors
fn print_stats(
    coordinator: &Coordinator,
    hashrate_calc: &mut HashrateCalculator,
    start_time: Instant,
    _cfg: &Config,
) -> f64 {
    let stats = coordinator.stats().snapshot();
    hashrate_calc.add_sample(stats.hashes);
    let hashrate = hashrate_calc.hashrate();

    let hr_str = format_hashrate(hashrate);
    let uptime = format_duration(start_time.elapsed());

    println!(
        "{} | {} {} {} {} | {}",
        hr_str.bright_cyan().bold(),
        "Shares:".dimmed(),
        stats.shares_accepted.to_string().bright_green(),
        "/".dimmed(),
        stats.shares_rejected.to_string().bright_red(),
        uptime.dimmed()
    );

    hashrate
}

fn print_mining_started() {
    println!(
        "\n{} {}\n",
        "⛏".bright_yellow(),
        "Mining started".bright_green().bold()
    );
}

fn print_new_job(job_id: &str) {
    println!(
        "{} {}",
        "New job:".dimmed(),
        job_id.bright_blue()
    );
}

fn print_share_accepted() {
    println!(
        "{} {}",
        "✓".bright_green(),
        "Share accepted".bright_green()
    );
}

fn print_share_rejected() {
    println!(
        "{} {}",
        "✗".bright_red(),
        "Share rejected".bright_red()
    );
}

fn print_banner() {
    println!(
        "{}",
        r#"
 ____  _____ ____    __  __ _
| __ )|_   _/ ___|  |  \/  (_)_ __   ___ _ __
|  _ \  | || |      | |\/| | | '_ \ / _ \ '__|
| |_) | | || |___   | |  | | | | | |  __/ |
|____/  |_| \____|  |_|  |_|_|_| |_|\___|_|
"#
        .bright_yellow()
    );
    println!(
        "{}",
        format!("             Bitcoin Solo Miner v{}", env!("CARGO_PKG_VERSION"))
            .bright_white()
            .bold()
    );
    println!();
}
