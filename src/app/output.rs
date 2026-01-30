//! Output helpers for console printing
//!
//! This module contains print helper functions for non-TUI mode output.

use colored::Colorize;

use crate::mining::Coordinator;
use crate::stats::{format_duration, format_hashrate, HashrateCalculator};
use std::time::Instant;

/// Print the application banner
pub fn print_banner() {
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
        format!(
            "             Bitcoin Solo Miner v{}",
            env!("CARGO_PKG_VERSION")
        )
        .bright_white()
        .bold()
    );
    println!();
}

/// Print the current mining mode
pub fn print_mode(mode: &str, detail: &str) {
    println!(
        "{} {} {}",
        "Mode:".bright_white(),
        mode.bright_green().bold(),
        format!("({})", detail).dimmed()
    );
}

/// Print "Mining started" message
pub fn print_mining_started() {
    println!(
        "\n{} {}\n",
        "⛏".bright_yellow(),
        "Mining started".bright_green().bold()
    );
}

/// Print new job notification
pub fn print_new_job(job_id: &str) {
    println!("{} {}", "New job:".dimmed(), job_id.bright_blue());
}

/// Print share accepted message
pub fn print_share_accepted() {
    println!(
        "{} {}",
        "✓".bright_green(),
        "Share accepted".bright_green()
    );
}

/// Print share rejected message
pub fn print_share_rejected() {
    println!("{} {}", "✗".bright_red(), "Share rejected".bright_red());
}

/// Print stats with colors and return the calculated hashrate
pub fn print_stats(
    coordinator: &Coordinator,
    hashrate_calc: &mut HashrateCalculator,
    start_time: Instant,
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

// Note: Print functions write to stdout and are tested implicitly through integration tests.
// Unit testing would require capturing stdout which adds complexity without much value.
