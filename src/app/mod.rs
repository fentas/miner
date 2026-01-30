//! Application utilities and helpers
//!
//! This module contains extracted helpers from main.rs for:
//! - Console output formatting
//! - Bitcoin Core log tailing

pub mod bitcoin_log;
pub mod output;

pub use bitcoin_log::tail_bitcoin_log;
pub use output::{
    print_banner, print_mining_started, print_mode, print_new_job, print_share_accepted,
    print_share_rejected, print_stats,
};
