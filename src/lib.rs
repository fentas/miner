//! BTC Miner Library
//!
//! High-performance Bitcoin solo miner written in Rust.
//!
//! # Features
//!
//! - **Mining**: Multi-threaded CPU mining with SIMD (SHA-NI, AVX2) and GPU (OpenCL, CUDA)
//! - **Protocols**: Stratum V1/V2 pool support and direct Bitcoin Core RPC
//! - **Interface**: TUI dashboard, web interface with WebSocket real-time updates
//! - **Configuration**: TOML-based configuration with pool presets
//!
//! # Modules
//!
//! - [`config`] - TOML configuration parsing and validation
//! - [`mining`] - Mining coordination, workers, and SHA-256d hashing
//! - [`stratum`] - Stratum V1/V2 protocol clients
//! - [`bitcoin`] - Bitcoin Core RPC client and manager
//! - [`gpu`] - GPU mining support (OpenCL, CUDA)
//! - [`web`] - Web dashboard REST API and WebSocket
//! - [`tui`] - Terminal UI (ratatui-based)
//! - [`stats`] - Hashrate calculation and statistics
//! - [`idle`] - Idle detection for screensaver-style mining
//! - [`profiling`] - Performance profiling and benchmarking

#![allow(dead_code)] // Some internal APIs are exposed for flexibility

/// Configuration parsing and validation
pub mod config;
/// Mining coordination and worker management
pub mod mining;
/// Stratum V1/V2 protocol clients
pub mod stratum;
/// Bitcoin Core RPC integration
pub mod bitcoin;
/// Web dashboard API
pub mod web;
/// Statistics and hashrate calculation
pub mod stats;
/// GPU mining support
pub mod gpu;
/// Idle detection
pub mod idle;
/// Terminal UI
pub mod tui;
/// Performance profiling
pub mod profiling;
