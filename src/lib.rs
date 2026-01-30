//! BTC Miner Library
//!
//! This library provides the core functionality for Bitcoin mining:
//! - Mining coordination and worker management
//! - Stratum V1/V2 protocol support
//! - Bitcoin Core RPC integration
//! - Web dashboard API
//! - Statistics and hashrate calculation

#![allow(dead_code)] // Allow unused code during development

pub mod config;
pub mod mining;
pub mod stratum;
pub mod bitcoin;
pub mod web;
pub mod stats;
pub mod gpu;
pub mod idle;
pub mod tui;
pub mod profiling;
