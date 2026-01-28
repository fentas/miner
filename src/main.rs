#![allow(dead_code)] // Allow unused code during development

mod config;
mod mining;
mod stratum;
mod bitcoin;
mod web;
mod stats;
mod gpu;
mod idle;

use std::sync::Arc;
use std::time::{Duration, Instant};
use clap::Parser;
use colored::Colorize;
use tracing::{info, warn, error, Level};
use tracing_subscriber::FmtSubscriber;

use config::Config;
use mining::{Coordinator, SimdLevel};
use idle::IdleMonitor;
use stats::{HashrateCalculator, format_hashrate, format_duration};

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Configure colored output
    if args.no_color {
        colored::control::set_override(false);
    }

    // Initialize logging
    let log_level = if args.debug { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Print banner
    print_banner();

    // Detect SIMD capabilities
    let simd_level = SimdLevel::detect();
    println!(
        "{}",
        format!("  SIMD: {}", simd_level.name()).bright_cyan()
    );
    println!();

    // Load configuration
    info!("Loading configuration from {}", args.config);
    let mut cfg = Config::from_file(&args.config)?;

    // Override threads if specified
    if let Some(threads) = args.threads {
        cfg.mining.threads = threads;
    }

    // Override color setting from config
    if !cfg.logging.colors {
        colored::control::set_override(false);
    }

    let thread_count = cfg.effective_threads();
    info!("Mining with {} CPU threads", thread_count);

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

fn print_mode(mode: &str, detail: &str) {
    println!(
        "{} {} {}",
        "Mode:".bright_white(),
        mode.bright_green().bold(),
        format!("({})", detail).dimmed()
    );
}

/// Run in true solo mode (no pool, direct to Bitcoin network)
async fn run_solo_mode(cfg: Config) -> anyhow::Result<()> {
    use bitcoin::BitcoindManager;

    // Setup Bitcoin Core
    let mut bitcoind = BitcoindManager::new()
        .with_prune(cfg.bitcoind.prune);

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

        bitcoind.write_config()?;
        bitcoind.start()?;

        // Wait for Bitcoin Core to start and sync
        println!(
            "{} {}",
            "⏳".bright_yellow(),
            "Starting Bitcoin Core...".bright_white()
        );

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

        // Get block template
        let rpc = bitcoind.rpc().unwrap();
        let template = match rpc.get_block_template(&cfg.pool.payout_address).await {
            Ok(t) => t,
            Err(e) => {
                warn!("Failed to get block template: {}", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Create mining job from template
        let mut job = mining::Job::new(format!("solo-{}", template.height));
        job.version = template.version;
        job.ntime = template.timestamp as u32;

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
