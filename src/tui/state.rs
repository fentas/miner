use std::collections::VecDeque;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

const MAX_LOGS: usize = 1000;
const MAX_HASHRATE_HISTORY: usize = 3600; // 1 hour at 1 sample/sec
const DEFAULT_LOG_TAIL: usize = 100; // Default tail count for display
const DEFAULT_HISTORY_TAIL: usize = 120; // 2 minutes for compact view

/// A hashrate sample with per-source breakdown and event markers
#[derive(Debug, Clone)]
pub struct HashrateSample {
    pub timestamp: Instant,
    pub cpu_hashrate: f64,
    pub gpu_hashrate: f64,
    pub gpu_device_hashrates: Vec<f64>, // Per-device GPU hashrates
    pub total_hashrate: f64,
    pub block_height: u64,
    pub new_block: bool, // True if this sample marks a new block
}

/// Time filter for logs (k9s style: 0=tail, 1=1m, 2=5m, 3=15m, 4=30m, 5=1h, 6=all)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LogTimeFilter {
    #[default]
    Tail,      // 0 - Show last N logs (default 100)
    OneMin,    // 1 - Last 1 minute
    FiveMin,   // 2 - Last 5 minutes
    FifteenMin,// 3 - Last 15 minutes
    ThirtyMin, // 4 - Last 30 minutes
    OneHour,   // 5 - Last 1 hour
    All,       // 6 - All logs (up to MAX_LOGS)
}

impl LogTimeFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogTimeFilter::Tail => "tail",
            LogTimeFilter::OneMin => "1m",
            LogTimeFilter::FiveMin => "5m",
            LogTimeFilter::FifteenMin => "15m",
            LogTimeFilter::ThirtyMin => "30m",
            LogTimeFilter::OneHour => "1h",
            LogTimeFilter::All => "all",
        }
    }

    pub fn from_key(key: char) -> Option<Self> {
        match key {
            '0' => Some(LogTimeFilter::Tail),
            '1' => Some(LogTimeFilter::OneMin),
            '2' => Some(LogTimeFilter::FiveMin),
            '3' => Some(LogTimeFilter::FifteenMin),
            '4' => Some(LogTimeFilter::ThirtyMin),
            '5' => Some(LogTimeFilter::OneHour),
            '6' => Some(LogTimeFilter::All),
            _ => None,
        }
    }

    /// Returns the maximum age for logs (None for Tail/All which don't filter by time)
    pub fn max_age(&self) -> Option<std::time::Duration> {
        match self {
            LogTimeFilter::Tail => None,  // Handled separately (last N)
            LogTimeFilter::OneMin => Some(std::time::Duration::from_secs(60)),
            LogTimeFilter::FiveMin => Some(std::time::Duration::from_secs(5 * 60)),
            LogTimeFilter::FifteenMin => Some(std::time::Duration::from_secs(15 * 60)),
            LogTimeFilter::ThirtyMin => Some(std::time::Duration::from_secs(30 * 60)),
            LogTimeFilter::OneHour => Some(std::time::Duration::from_secs(60 * 60)),
            LogTimeFilter::All => None,  // Show all
        }
    }

    pub fn is_tail(&self) -> bool {
        matches!(self, LogTimeFilter::Tail)
    }
}

/// Hashrate display mode
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum HashrateDisplayMode {
    #[default]
    Total,     // Show total (CPU + GPU combined)
    CpuOnly,   // Show CPU hashrate only
    GpuOnly,   // Show GPU hashrate only (combined all GPUs)
    GpuDevice(u8), // Show specific GPU device (0, 1, 2, ...)
}

impl HashrateDisplayMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            HashrateDisplayMode::Total => "Total",
            HashrateDisplayMode::CpuOnly => "CPU",
            HashrateDisplayMode::GpuOnly => "GPU",
            HashrateDisplayMode::GpuDevice(_) => "GPU#",
        }
    }

    /// Cycle to next mode
    pub fn next(self, gpu_count: usize) -> Self {
        match self {
            HashrateDisplayMode::Total => HashrateDisplayMode::CpuOnly,
            HashrateDisplayMode::CpuOnly => {
                if gpu_count > 0 {
                    HashrateDisplayMode::GpuOnly
                } else {
                    HashrateDisplayMode::Total
                }
            }
            HashrateDisplayMode::GpuOnly => {
                if gpu_count > 1 {
                    HashrateDisplayMode::GpuDevice(0)
                } else {
                    HashrateDisplayMode::Total
                }
            }
            HashrateDisplayMode::GpuDevice(idx) => {
                let next_idx = idx + 1;
                if (next_idx as usize) < gpu_count {
                    HashrateDisplayMode::GpuDevice(next_idx)
                } else {
                    HashrateDisplayMode::Total
                }
            }
        }
    }
}

/// Log level for TUI display
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Success,
    Debug,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Success => "OK",
            LogLevel::Debug => "DEBUG",
        }
    }
}

/// A log entry for display
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: String,
    pub created_at: Instant,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        let now = chrono::Local::now();
        Self {
            level,
            message: message.into(),
            timestamp: now.format("%H:%M:%S").to_string(),
            created_at: Instant::now(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, message)
    }

    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warn, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, message)
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Success, message)
    }

    /// Create a log entry with a custom timestamp
    pub fn with_timestamp(level: LogLevel, message: impl Into<String>, timestamp: impl Into<String>) -> Self {
        Self {
            level,
            created_at: Instant::now(),
            message: message.into(),
            timestamp: timestamp.into(),
        }
    }
}

/// Shared state between mining threads and TUI
pub struct TuiState {
    // Mining stats
    pub hashrate: AtomicU64,
    pub total_hashes: AtomicU64,
    pub shares_accepted: AtomicU64,
    pub shares_rejected: AtomicU64,
    pub block_height: AtomicU64,
    pub difficulty: AtomicU64,

    // Hashrate history for graph
    hashrate_history: RwLock<VecDeque<f64>>,

    // Connection info
    pool_address: RwLock<String>,
    mode: RwLock<String>,
    simd_level: RwLock<String>,
    worker_count: AtomicUsize,
    web_url: RwLock<String>,

    // GPU info
    gpu_configured: AtomicBool,  // User enabled GPU in config
    gpu_available: AtomicBool,   // GPU actually detected and usable
    gpu_devices: RwLock<Vec<String>>,
    gpu_hashrate: AtomicU64,
    /// Per-device hashrates: Vec<(device_id, hashrate_bits)>
    gpu_device_hashrates: RwLock<Vec<(u32, u64)>>,

    // Status
    pub paused: AtomicBool,
    connected: AtomicBool,
    syncing: AtomicBool,
    sync_progress: AtomicU64, // Stored as progress * 100 (for decimal precision)

    // Idle mode status
    idle_enabled: AtomicBool,
    idle_active: AtomicBool,  // True when system is idle (mining active)
    idle_cpu: AtomicBool,     // CPU follows idle state
    idle_gpu: AtomicBool,     // GPU follows idle state

    // Download progress (e.g., for AssumeUTXO)
    downloading: AtomicBool,
    download_name: RwLock<String>,
    download_bytes: AtomicU64,
    download_total: AtomicU64,

    // Timing
    start_time: Instant,
    last_share_time: RwLock<Option<Instant>>,
    last_block_time: RwLock<Option<Instant>>,

    // Logs
    logs: RwLock<VecDeque<LogEntry>>,
    bitcoin_logs: RwLock<VecDeque<LogEntry>>,
    log_scroll: AtomicUsize,

    // Control
    should_quit: AtomicBool,

    // Bitcoin Core managed mode
    bitcoin_managed: AtomicBool,
    show_bitcoin_logs: AtomicBool,
    logs_focused: AtomicBool,
    logs_autoscroll: AtomicBool,
    peer_count: AtomicU64,

    // Hashrate display mode
    hashrate_display_mode: RwLock<HashrateDisplayMode>,

    // k9s-style features
    show_help: AtomicBool,
    command_mode: AtomicBool,
    command_buffer: RwLock<String>,
    log_time_filter: RwLock<LogTimeFilter>,

    // History panel (Shift+H to focus)
    history_focused: AtomicBool,
    history_time_filter: RwLock<LogTimeFilter>,
    history_samples: RwLock<VecDeque<HashrateSample>>,

    // Desktop overlay (separate floating window)
    overlay_enabled: AtomicBool,
    overlay_process: Mutex<Option<Child>>,
}

impl TuiState {
    pub fn new() -> Self {
        Self {
            hashrate: AtomicU64::new(0),
            total_hashes: AtomicU64::new(0),
            shares_accepted: AtomicU64::new(0),
            shares_rejected: AtomicU64::new(0),
            block_height: AtomicU64::new(0),
            difficulty: AtomicU64::new(0),
            hashrate_history: RwLock::new(VecDeque::with_capacity(MAX_HASHRATE_HISTORY)),
            pool_address: RwLock::new(String::new()),
            mode: RwLock::new("Initializing".to_string()),
            simd_level: RwLock::new("Unknown".to_string()),
            worker_count: AtomicUsize::new(0),
            web_url: RwLock::new(String::new()),
            gpu_configured: AtomicBool::new(false),
            gpu_available: AtomicBool::new(false),
            gpu_devices: RwLock::new(Vec::new()),
            gpu_hashrate: AtomicU64::new(0),
            gpu_device_hashrates: RwLock::new(Vec::new()),
            paused: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            syncing: AtomicBool::new(false),
            sync_progress: AtomicU64::new(0),
            idle_enabled: AtomicBool::new(false),
            idle_active: AtomicBool::new(false),
            idle_cpu: AtomicBool::new(true),
            idle_gpu: AtomicBool::new(true),
            downloading: AtomicBool::new(false),
            download_name: RwLock::new(String::new()),
            download_bytes: AtomicU64::new(0),
            download_total: AtomicU64::new(0),
            start_time: Instant::now(),
            last_share_time: RwLock::new(None),
            last_block_time: RwLock::new(None),
            logs: RwLock::new(VecDeque::with_capacity(MAX_LOGS)),
            bitcoin_logs: RwLock::new(VecDeque::with_capacity(MAX_LOGS)),
            log_scroll: AtomicUsize::new(0),
            should_quit: AtomicBool::new(false),
            bitcoin_managed: AtomicBool::new(false),
            show_bitcoin_logs: AtomicBool::new(false),
            logs_focused: AtomicBool::new(false),
            logs_autoscroll: AtomicBool::new(true),
            peer_count: AtomicU64::new(0),
            show_help: AtomicBool::new(false),
            command_mode: AtomicBool::new(false),
            command_buffer: RwLock::new(String::new()),
            log_time_filter: RwLock::new(LogTimeFilter::default()),
            hashrate_display_mode: RwLock::new(HashrateDisplayMode::default()),
            history_focused: AtomicBool::new(false),
            history_time_filter: RwLock::new(LogTimeFilter::default()),
            history_samples: RwLock::new(VecDeque::with_capacity(MAX_HASHRATE_HISTORY)),
            overlay_enabled: AtomicBool::new(false),
            overlay_process: Mutex::new(None),
        }
    }

    // Setters
    pub fn set_hashrate(&self, rate: f64) {
        self.hashrate.store(rate.to_bits(), Ordering::Relaxed);

        // Add to simple history (legacy)
        if let Ok(mut history) = self.hashrate_history.write() {
            if history.len() >= MAX_HASHRATE_HISTORY {
                history.pop_front();
            }
            history.push_back(rate);
        }

        // Add to detailed history samples
        self.record_hashrate_sample(rate);
    }

    /// Record a detailed hashrate sample with per-source breakdown
    pub fn record_hashrate_sample(&self, total: f64) {
        let gpu_rate = f64::from_bits(self.gpu_hashrate.load(Ordering::Relaxed));
        let cpu_rate = total - gpu_rate;

        // Get per-device GPU hashrates
        let device_rates: Vec<f64> = self.gpu_device_hashrates
            .read()
            .map(|h| h.iter().map(|(_, bits)| f64::from_bits(*bits)).collect())
            .unwrap_or_default();

        let sample = HashrateSample {
            timestamp: Instant::now(),
            cpu_hashrate: cpu_rate.max(0.0),
            gpu_hashrate: gpu_rate,
            gpu_device_hashrates: device_rates,
            total_hashrate: total,
            block_height: self.block_height.load(Ordering::Relaxed),
            new_block: false,
        };

        if let Ok(mut samples) = self.history_samples.write() {
            if samples.len() >= MAX_HASHRATE_HISTORY {
                samples.pop_front();
            }
            samples.push_back(sample);
        }
    }

    /// Mark the most recent sample as a new block event
    pub fn mark_new_block(&self) {
        if let Ok(mut samples) = self.history_samples.write() {
            if let Some(sample) = samples.back_mut() {
                sample.new_block = true;
            }
        }
    }

    pub fn set_total_hashes(&self, hashes: u64) {
        self.total_hashes.store(hashes, Ordering::Relaxed);
    }

    pub fn set_shares(&self, accepted: u64, rejected: u64) {
        self.shares_accepted.store(accepted, Ordering::Relaxed);
        self.shares_rejected.store(rejected, Ordering::Relaxed);
    }

    pub fn record_share(&self) {
        if let Ok(mut time) = self.last_share_time.write() {
            *time = Some(Instant::now());
        }
    }

    pub fn set_block_height(&self, height: u64) {
        let prev = self.block_height.swap(height, Ordering::Relaxed);
        if height > prev && prev > 0 {
            if let Ok(mut time) = self.last_block_time.write() {
                *time = Some(Instant::now());
            }
            // Mark new block in hashrate history
            self.mark_new_block();
        }
    }

    pub fn set_difficulty(&self, diff: u64) {
        self.difficulty.store(diff, Ordering::Relaxed);
    }

    pub fn set_pool_address(&self, addr: impl Into<String>) {
        if let Ok(mut pool) = self.pool_address.write() {
            *pool = addr.into();
        }
    }

    pub fn set_mode(&self, mode: impl Into<String>) {
        if let Ok(mut m) = self.mode.write() {
            *m = mode.into();
        }
    }

    pub fn set_simd_level(&self, level: impl Into<String>) {
        if let Ok(mut s) = self.simd_level.write() {
            *s = level.into();
        }
    }

    pub fn set_worker_count(&self, count: usize) {
        self.worker_count.store(count, Ordering::Relaxed);
    }

    pub fn set_web_url(&self, url: impl Into<String>) {
        if let Ok(mut u) = self.web_url.write() {
            *u = url.into();
        }
    }

    pub fn set_gpu_configured(&self, configured: bool) {
        self.gpu_configured.store(configured, Ordering::Relaxed);
    }

    pub fn set_gpu_available(&self, available: bool) {
        self.gpu_available.store(available, Ordering::Relaxed);
    }

    /// Convenience method: set both configured and available
    pub fn set_gpu_enabled(&self, enabled: bool) {
        self.gpu_configured.store(enabled, Ordering::Relaxed);
        self.gpu_available.store(enabled, Ordering::Relaxed);
    }

    pub fn set_gpu_devices(&self, devices: Vec<String>) {
        if let Ok(mut d) = self.gpu_devices.write() {
            *d = devices;
        }
    }

    pub fn set_gpu_hashrate(&self, rate: f64) {
        self.gpu_hashrate.store(rate.to_bits(), Ordering::Relaxed);
    }

    /// Set per-device GPU hashrates
    pub fn set_gpu_device_hashrates(&self, hashrates: Vec<(u32, f64)>) {
        if let Ok(mut h) = self.gpu_device_hashrates.write() {
            *h = hashrates.into_iter()
                .map(|(id, rate)| (id, rate.to_bits()))
                .collect();
        }
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    pub fn set_syncing(&self, syncing: bool, progress: f64) {
        self.syncing.store(syncing, Ordering::Relaxed);
        self.sync_progress.store((progress * 100.0) as u64, Ordering::Relaxed);
    }

    pub fn set_downloading(&self, downloading: bool, name: impl Into<String>) {
        self.downloading.store(downloading, Ordering::Relaxed);
        if let Ok(mut n) = self.download_name.write() {
            *n = name.into();
        }
        if !downloading {
            self.download_bytes.store(0, Ordering::Relaxed);
            self.download_total.store(0, Ordering::Relaxed);
        }
    }

    pub fn set_download_progress(&self, bytes: u64, total: u64) {
        self.download_bytes.store(bytes, Ordering::Relaxed);
        self.download_total.store(total, Ordering::Relaxed);
    }

    pub fn toggle_pause(&self) -> bool {
        let prev = self.paused.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn set_quit(&self) {
        self.should_quit.store(true, Ordering::Relaxed);
    }

    pub fn set_bitcoin_managed(&self, managed: bool) {
        self.bitcoin_managed.store(managed, Ordering::Relaxed);
    }

    pub fn set_peer_count(&self, count: u64) {
        self.peer_count.store(count, Ordering::Relaxed);
    }

    pub fn get_peer_count(&self) -> u64 {
        self.peer_count.load(Ordering::Relaxed)
    }

    pub fn toggle_bitcoin_logs(&self) -> bool {
        let prev = self.show_bitcoin_logs.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn toggle_logs_focus(&self) -> bool {
        let prev = self.logs_focused.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn is_logs_focused(&self) -> bool {
        self.logs_focused.load(Ordering::Relaxed)
    }

    pub fn toggle_autoscroll(&self) -> bool {
        let prev = self.logs_autoscroll.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn is_autoscroll(&self) -> bool {
        self.logs_autoscroll.load(Ordering::Relaxed)
    }

    // Help toggle
    pub fn toggle_help(&self) -> bool {
        let prev = self.show_help.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn is_help_visible(&self) -> bool {
        self.show_help.load(Ordering::Relaxed)
    }

    pub fn hide_help(&self) {
        self.show_help.store(false, Ordering::Relaxed);
    }

    // Command mode
    pub fn enter_command_mode(&self) {
        self.command_mode.store(true, Ordering::Relaxed);
        if let Ok(mut buf) = self.command_buffer.write() {
            buf.clear();
        }
    }

    pub fn exit_command_mode(&self) {
        self.command_mode.store(false, Ordering::Relaxed);
        if let Ok(mut buf) = self.command_buffer.write() {
            buf.clear();
        }
    }

    pub fn is_command_mode(&self) -> bool {
        self.command_mode.load(Ordering::Relaxed)
    }

    pub fn command_push(&self, c: char) {
        if let Ok(mut buf) = self.command_buffer.write() {
            buf.push(c);
        }
    }

    pub fn command_pop(&self) {
        if let Ok(mut buf) = self.command_buffer.write() {
            buf.pop();
        }
    }

    pub fn get_command(&self) -> String {
        self.command_buffer.read().map(|b| b.clone()).unwrap_or_default()
    }

    // Time filter
    pub fn set_log_time_filter(&self, filter: LogTimeFilter) {
        if let Ok(mut f) = self.log_time_filter.write() {
            *f = filter;
        }
    }

    pub fn get_log_time_filter(&self) -> LogTimeFilter {
        self.log_time_filter.read().map(|f| *f).unwrap_or_default()
    }

    // History panel focus (Shift+H)
    pub fn toggle_history_focus(&self) -> bool {
        // Unfocus logs when focusing history
        if !self.history_focused.load(Ordering::Relaxed) {
            self.logs_focused.store(false, Ordering::Relaxed);
        }
        let prev = self.history_focused.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn is_history_focused(&self) -> bool {
        self.history_focused.load(Ordering::Relaxed)
    }

    pub fn set_history_time_filter(&self, filter: LogTimeFilter) {
        if let Ok(mut f) = self.history_time_filter.write() {
            *f = filter;
        }
    }

    pub fn get_history_time_filter(&self) -> LogTimeFilter {
        self.history_time_filter.read().map(|f| *f).unwrap_or_default()
    }

    /// Get filtered history samples based on current time filter
    pub fn get_filtered_history_samples(&self) -> Vec<HashrateSample> {
        let filter = self.get_history_time_filter();
        let now = Instant::now();

        if let Ok(samples) = self.history_samples.read() {
            if filter.is_tail() {
                // Tail mode: return last N samples (2 minutes = 120)
                let skip = samples.len().saturating_sub(DEFAULT_HISTORY_TAIL);
                samples.iter().skip(skip).cloned().collect()
            } else if let Some(max_age) = filter.max_age() {
                // Time-based filter
                samples.iter()
                    .filter(|s| now.duration_since(s.timestamp) <= max_age)
                    .cloned()
                    .collect()
            } else {
                // All samples
                samples.iter().cloned().collect()
            }
        } else {
            Vec::new()
        }
    }

    // Getters
    pub fn get_hashrate(&self) -> f64 {
        f64::from_bits(self.hashrate.load(Ordering::Relaxed))
    }

    pub fn get_hashrate_history(&self) -> Vec<f64> {
        self.hashrate_history.read().map(|h| h.iter().copied().collect()).unwrap_or_default()
    }

    pub fn get_total_hashes(&self) -> u64 {
        self.total_hashes.load(Ordering::Relaxed)
    }

    pub fn get_shares(&self) -> (u64, u64) {
        (
            self.shares_accepted.load(Ordering::Relaxed),
            self.shares_rejected.load(Ordering::Relaxed),
        )
    }

    pub fn get_block_height(&self) -> u64 {
        self.block_height.load(Ordering::Relaxed)
    }

    pub fn get_difficulty(&self) -> u64 {
        self.difficulty.load(Ordering::Relaxed)
    }

    pub fn get_pool_address(&self) -> String {
        self.pool_address.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn get_mode(&self) -> String {
        self.mode.read().map(|m| m.clone()).unwrap_or_default()
    }

    pub fn get_simd_level(&self) -> String {
        self.simd_level.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn get_worker_count(&self) -> usize {
        self.worker_count.load(Ordering::Relaxed)
    }

    pub fn get_web_url(&self) -> String {
        self.web_url.read().map(|u| u.clone()).unwrap_or_default()
    }

    pub fn is_gpu_configured(&self) -> bool {
        self.gpu_configured.load(Ordering::Relaxed)
    }

    pub fn is_gpu_available(&self) -> bool {
        self.gpu_available.load(Ordering::Relaxed)
    }

    /// Alias for is_gpu_available for backwards compatibility
    pub fn is_gpu_enabled(&self) -> bool {
        self.gpu_available.load(Ordering::Relaxed)
    }

    pub fn get_gpu_devices(&self) -> Vec<String> {
        self.gpu_devices.read().map(|d| d.clone()).unwrap_or_default()
    }

    pub fn get_gpu_hashrate(&self) -> f64 {
        f64::from_bits(self.gpu_hashrate.load(Ordering::Relaxed))
    }

    /// Get hashrate for a specific GPU device
    pub fn get_gpu_device_hashrate(&self, device_id: u32) -> f64 {
        self.gpu_device_hashrates.read()
            .ok()
            .and_then(|h| h.iter().find(|(id, _)| *id == device_id).map(|(_, bits)| f64::from_bits(*bits)))
            .unwrap_or(0.0)
    }

    /// Get current hashrate display mode
    pub fn get_hashrate_display_mode(&self) -> HashrateDisplayMode {
        self.hashrate_display_mode.read().map(|m| *m).unwrap_or_default()
    }

    /// Toggle hashrate display mode (cycles: Total -> CPU -> GPU -> GPU#0 -> GPU#1 -> ... -> Total)
    pub fn toggle_hashrate_display_mode(&self) -> HashrateDisplayMode {
        let gpu_count = self.gpu_devices.read().map(|d| d.len()).unwrap_or(0);
        let new_mode = if let Ok(mut mode) = self.hashrate_display_mode.write() {
            *mode = mode.next(gpu_count);
            *mode
        } else {
            HashrateDisplayMode::default()
        };
        new_mode
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn is_syncing(&self) -> bool {
        self.syncing.load(Ordering::Relaxed)
    }

    pub fn get_sync_progress(&self) -> f64 {
        self.sync_progress.load(Ordering::Relaxed) as f64 / 100.0
    }

    pub fn is_downloading(&self) -> bool {
        self.downloading.load(Ordering::Relaxed)
    }

    pub fn get_download_name(&self) -> String {
        self.download_name.read().map(|n| n.clone()).unwrap_or_default()
    }

    pub fn get_download_progress(&self) -> (u64, u64) {
        (
            self.download_bytes.load(Ordering::Relaxed),
            self.download_total.load(Ordering::Relaxed),
        )
    }

    // Idle mode methods
    pub fn set_idle_enabled(&self, enabled: bool) {
        self.idle_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_idle_enabled(&self) -> bool {
        self.idle_enabled.load(Ordering::Relaxed)
    }

    pub fn set_idle_active(&self, active: bool) {
        self.idle_active.store(active, Ordering::Relaxed);
    }

    pub fn is_idle_active(&self) -> bool {
        self.idle_active.load(Ordering::Relaxed)
    }

    pub fn set_idle_cpu(&self, cpu: bool) {
        self.idle_cpu.store(cpu, Ordering::Relaxed);
    }

    pub fn is_idle_cpu(&self) -> bool {
        self.idle_cpu.load(Ordering::Relaxed)
    }

    pub fn set_idle_gpu(&self, gpu: bool) {
        self.idle_gpu.store(gpu, Ordering::Relaxed);
    }

    pub fn is_idle_gpu(&self) -> bool {
        self.idle_gpu.load(Ordering::Relaxed)
    }

    pub fn get_uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn get_last_share_ago(&self) -> Option<std::time::Duration> {
        self.last_share_time.read().ok().and_then(|t| t.map(|t| t.elapsed()))
    }

    pub fn get_last_block_ago(&self) -> Option<std::time::Duration> {
        self.last_block_time.read().ok().and_then(|t| t.map(|t| t.elapsed()))
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit.load(Ordering::Relaxed)
    }

    pub fn is_bitcoin_managed(&self) -> bool {
        self.bitcoin_managed.load(Ordering::Relaxed)
    }

    pub fn is_showing_bitcoin_logs(&self) -> bool {
        self.show_bitcoin_logs.load(Ordering::Relaxed)
    }

    // Logging
    pub fn log(&self, entry: LogEntry) {
        if let Ok(mut logs) = self.logs.write() {
            if logs.len() >= MAX_LOGS {
                logs.pop_front();
            }
            logs.push_back(entry);
        }
    }

    pub fn log_info(&self, message: impl Into<String>) {
        self.log(LogEntry::info(message));
    }

    pub fn log_warn(&self, message: impl Into<String>) {
        self.log(LogEntry::warn(message));
    }

    pub fn log_error(&self, message: impl Into<String>) {
        self.log(LogEntry::error(message));
    }

    pub fn log_success(&self, message: impl Into<String>) {
        self.log(LogEntry::success(message));
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.read().map(|l| l.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn get_log_count(&self) -> usize {
        self.logs.read().map(|l| l.len()).unwrap_or(0)
    }

    pub fn clear_logs(&self) {
        if let Ok(mut logs) = self.logs.write() {
            logs.clear();
        }
        if let Ok(mut logs) = self.bitcoin_logs.write() {
            logs.clear();
        }
        self.log_scroll.store(0, Ordering::Relaxed);
    }

    // Bitcoin Core logs
    pub fn log_bitcoin(&self, entry: LogEntry) {
        if let Ok(mut logs) = self.bitcoin_logs.write() {
            if logs.len() >= MAX_LOGS {
                logs.pop_front();
            }
            logs.push_back(entry);
        }
    }

    pub fn get_bitcoin_logs(&self) -> Vec<LogEntry> {
        self.bitcoin_logs.read().map(|l| l.iter().cloned().collect()).unwrap_or_default()
    }

    // Scroll
    pub fn scroll_up(&self, amount: usize) {
        let current = self.log_scroll.load(Ordering::Relaxed);
        self.log_scroll.store(current.saturating_sub(amount), Ordering::Relaxed);
    }

    pub fn scroll_down(&self, amount: usize) {
        let current = self.log_scroll.load(Ordering::Relaxed);
        let max = self.get_log_count().saturating_sub(1);
        self.log_scroll.store((current + amount).min(max), Ordering::Relaxed);
    }

    pub fn scroll_to_top(&self) {
        self.log_scroll.store(0, Ordering::Relaxed);
    }

    pub fn scroll_to_bottom(&self) {
        let max = self.get_log_count().saturating_sub(1);
        self.log_scroll.store(max, Ordering::Relaxed);
    }

    pub fn get_log_scroll(&self) -> usize {
        self.log_scroll.load(Ordering::Relaxed)
    }

    // Overlay controls
    pub fn set_overlay_enabled(&self, enabled: bool) {
        self.overlay_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_overlay_enabled(&self) -> bool {
        self.overlay_enabled.load(Ordering::Relaxed)
    }

    pub fn is_overlay_running(&self) -> bool {
        self.overlay_process
            .lock()
            .map(|p| p.is_some())
            .unwrap_or(false)
    }

    /// Toggle the overlay window. Returns true if overlay is now running.
    pub fn toggle_overlay(&self) -> bool {
        let mut process = match self.overlay_process.lock() {
            Ok(p) => p,
            Err(_) => return false,
        };

        if let Some(ref mut child) = *process {
            // Overlay is running, kill it
            let _ = child.kill();
            let _ = child.wait();
            *process = None;
            false
        } else {
            // Overlay is not running, try to start it
            // Look for btcminer-overlay in same directory as current executable
            let overlay_path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("btcminer-overlay")));

            if let Some(path) = overlay_path {
                if path.exists() {
                    match Command::new(&path).spawn() {
                        Ok(child) => {
                            *process = Some(child);
                            true
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
    }

    /// Check if overlay binary exists (for showing shortcut in TUI)
    pub fn overlay_available(&self) -> bool {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("btcminer-overlay")))
            .is_some_and(|p| p.exists())
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_state_new() {
        let state = TuiState::new();
        assert_eq!(state.get_hashrate(), 0.0);
        assert_eq!(state.get_total_hashes(), 0);
        assert_eq!(state.get_shares(), (0, 0));
        assert!(!state.is_paused());
        assert!(!state.is_connected());
        assert!(!state.should_quit());
    }

    #[test]
    fn test_tui_state_hashrate() {
        let state = TuiState::new();
        state.set_hashrate(1234.56);
        assert!((state.get_hashrate() - 1234.56).abs() < 0.001);

        // Test history
        state.set_hashrate(2000.0);
        state.set_hashrate(3000.0);
        let history = state.get_hashrate_history();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_tui_state_shares() {
        let state = TuiState::new();
        state.set_shares(10, 2);
        assert_eq!(state.get_shares(), (10, 2));
    }

    #[test]
    fn test_tui_state_pause_toggle() {
        let state = TuiState::new();
        assert!(!state.is_paused());

        let paused = state.toggle_pause();
        assert!(paused);
        assert!(state.is_paused());

        let paused = state.toggle_pause();
        assert!(!paused);
        assert!(!state.is_paused());
    }

    #[test]
    fn test_tui_state_logging() {
        let state = TuiState::new();

        state.log_info("Info message");
        state.log_warn("Warning message");
        state.log_error("Error message");
        state.log_success("Success message");

        assert_eq!(state.get_log_count(), 4);

        let logs = state.get_logs();
        assert_eq!(logs[0].level, LogLevel::Info);
        assert_eq!(logs[1].level, LogLevel::Warn);
        assert_eq!(logs[2].level, LogLevel::Error);
        assert_eq!(logs[3].level, LogLevel::Success);
    }

    #[test]
    fn test_tui_state_log_scroll() {
        let state = TuiState::new();

        // Add some logs
        for i in 0..20 {
            state.log_info(format!("Log {}", i));
        }

        assert_eq!(state.get_log_scroll(), 0);

        state.scroll_down(5);
        assert_eq!(state.get_log_scroll(), 5);

        state.scroll_up(2);
        assert_eq!(state.get_log_scroll(), 3);

        state.scroll_to_bottom();
        assert_eq!(state.get_log_scroll(), 19);

        state.scroll_to_top();
        assert_eq!(state.get_log_scroll(), 0);
    }

    #[test]
    fn test_tui_state_clear_logs() {
        let state = TuiState::new();

        state.log_info("Test log");
        assert_eq!(state.get_log_count(), 1);

        state.clear_logs();
        assert_eq!(state.get_log_count(), 0);
        assert_eq!(state.get_log_scroll(), 0);
    }

    #[test]
    fn test_tui_state_connection() {
        let state = TuiState::new();

        state.set_pool_address("solo.ckpool.org:3333");
        assert_eq!(state.get_pool_address(), "solo.ckpool.org:3333");

        state.set_connected(true);
        assert!(state.is_connected());
    }

    #[test]
    fn test_tui_state_syncing() {
        let state = TuiState::new();

        state.set_syncing(true, 45.5);
        assert!(state.is_syncing());
        assert!((state.get_sync_progress() - 45.5).abs() < 0.1);
    }

    #[test]
    fn test_tui_state_mode_and_simd() {
        let state = TuiState::new();

        state.set_mode("Stratum V1");
        assert_eq!(state.get_mode(), "Stratum V1");

        state.set_simd_level("SHA-NI");
        assert_eq!(state.get_simd_level(), "SHA-NI");

        state.set_worker_count(8);
        assert_eq!(state.get_worker_count(), 8);
    }

    #[test]
    fn test_tui_state_quit() {
        let state = TuiState::new();
        assert!(!state.should_quit());

        state.set_quit();
        assert!(state.should_quit());
    }

    #[test]
    fn test_log_entry_levels() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Success.as_str(), "OK");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    }

    #[test]
    fn test_log_entry_new() {
        let entry = LogEntry::new(LogLevel::Debug, "Debug message");
        assert_eq!(entry.level, LogLevel::Debug);
        assert_eq!(entry.message, "Debug message");
        // Timestamp should be in HH:MM:SS format
        assert_eq!(entry.timestamp.len(), 8);
        assert!(entry.timestamp.contains(':'));
    }

    #[test]
    fn test_log_entry_factory_methods() {
        let info = LogEntry::info("info");
        assert_eq!(info.level, LogLevel::Info);

        let warn = LogEntry::warn("warn");
        assert_eq!(warn.level, LogLevel::Warn);

        let error = LogEntry::error("error");
        assert_eq!(error.level, LogLevel::Error);

        let success = LogEntry::success("success");
        assert_eq!(success.level, LogLevel::Success);
    }

    #[test]
    fn test_tui_state_default() {
        let state = TuiState::default();
        assert_eq!(state.get_hashrate(), 0.0);
        assert!(!state.is_connected());
    }

    #[test]
    fn test_tui_state_gpu_methods() {
        let state = TuiState::new();

        // Test GPU configured/available separately
        assert!(!state.is_gpu_configured());
        assert!(!state.is_gpu_available());

        state.set_gpu_configured(true);
        assert!(state.is_gpu_configured());
        assert!(!state.is_gpu_available());

        state.set_gpu_available(true);
        assert!(state.is_gpu_available());
        assert!(state.is_gpu_enabled()); // Alias test

        // Test set_gpu_enabled sets both
        state.set_gpu_enabled(false);
        assert!(!state.is_gpu_configured());
        assert!(!state.is_gpu_available());
    }

    #[test]
    fn test_tui_state_gpu_devices() {
        let state = TuiState::new();

        let devices = vec!["GPU 0: NVIDIA RTX 3080".to_string(), "GPU 1: AMD RX 6800".to_string()];
        state.set_gpu_devices(devices.clone());

        let retrieved = state.get_gpu_devices();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0], "GPU 0: NVIDIA RTX 3080");
    }

    #[test]
    fn test_tui_state_gpu_hashrate() {
        let state = TuiState::new();
        state.set_gpu_hashrate(1500000.5);
        assert!((state.get_gpu_hashrate() - 1500000.5).abs() < 0.1);
    }

    #[test]
    fn test_tui_state_download_progress() {
        let state = TuiState::new();

        // Start download
        state.set_downloading(true, "utxo-snapshot.dat");
        assert!(state.is_downloading());
        assert_eq!(state.get_download_name(), "utxo-snapshot.dat");

        // Update progress
        state.set_download_progress(1_000_000, 10_000_000);
        let (bytes, total) = state.get_download_progress();
        assert_eq!(bytes, 1_000_000);
        assert_eq!(total, 10_000_000);

        // Stop download (should reset progress)
        state.set_downloading(false, "");
        assert!(!state.is_downloading());
        let (bytes, total) = state.get_download_progress();
        assert_eq!(bytes, 0);
        assert_eq!(total, 0);
    }

    #[test]
    fn test_tui_state_total_hashes() {
        let state = TuiState::new();
        state.set_total_hashes(1_000_000);
        assert_eq!(state.get_total_hashes(), 1_000_000);
    }

    #[test]
    fn test_tui_state_difficulty() {
        let state = TuiState::new();
        state.set_difficulty(55_000_000_000_000);
        assert_eq!(state.get_difficulty(), 55_000_000_000_000);
    }

    #[test]
    fn test_tui_state_block_height_new_block() {
        let state = TuiState::new();

        // Set initial height
        state.set_block_height(800000);
        assert_eq!(state.get_block_height(), 800000);

        // New block should record time
        state.set_block_height(800001);
        assert_eq!(state.get_block_height(), 800001);

        // Check that last block time was recorded
        let ago = state.get_last_block_ago();
        assert!(ago.is_some());
    }

    #[test]
    fn test_tui_state_record_share() {
        let state = TuiState::new();

        // Initially no last share time
        assert!(state.get_last_share_ago().is_none());

        // Record a share
        state.record_share();

        // Now should have a last share time
        let ago = state.get_last_share_ago();
        assert!(ago.is_some());
        assert!(ago.unwrap().as_secs() < 1); // Should be very recent
    }

    #[test]
    fn test_tui_state_web_url() {
        let state = TuiState::new();
        state.set_web_url("http://localhost:8080");
        assert_eq!(state.get_web_url(), "http://localhost:8080");
    }

    #[test]
    fn test_tui_state_bitcoin_managed() {
        let state = TuiState::new();

        assert!(!state.is_bitcoin_managed());
        state.set_bitcoin_managed(true);
        assert!(state.is_bitcoin_managed());
    }

    #[test]
    fn test_tui_state_bitcoin_logs_toggle() {
        let state = TuiState::new();

        assert!(!state.is_showing_bitcoin_logs());

        let showing = state.toggle_bitcoin_logs();
        assert!(showing);
        assert!(state.is_showing_bitcoin_logs());

        let showing = state.toggle_bitcoin_logs();
        assert!(!showing);
        assert!(!state.is_showing_bitcoin_logs());
    }

    #[test]
    fn test_tui_state_uptime() {
        let state = TuiState::new();
        let uptime = state.get_uptime();
        // Should be at least 0 and less than 1 second (test just started)
        assert!(uptime.as_millis() < 1000);
    }

    #[test]
    fn test_tui_state_hashrate_history_limit() {
        let state = TuiState::new();

        // Add more than MAX_HASHRATE_HISTORY samples (3600)
        // Test with a smaller number that still exceeds reasonable limit
        for i in 0..200 {
            state.set_hashrate(i as f64 * 100.0);
        }

        let history = state.get_hashrate_history();
        // Should store all 200 samples (max is now 3600)
        assert_eq!(history.len(), 200);
        // Verify oldest sample was pushed first
        assert_eq!(history[0], 0.0);
        assert_eq!(history[199], 199.0 * 100.0);
    }

    #[test]
    fn test_tui_state_scroll_bounds() {
        let state = TuiState::new();

        // Scroll up when at 0 should stay at 0
        state.scroll_up(10);
        assert_eq!(state.get_log_scroll(), 0);

        // Add some logs
        for i in 0..10 {
            state.log_info(format!("Log {}", i));
        }

        // Scroll down past max should clamp
        state.scroll_down(100);
        assert_eq!(state.get_log_scroll(), 9); // max is len - 1 = 9

        // Scroll up from there
        state.scroll_up(5);
        assert_eq!(state.get_log_scroll(), 4);
    }

    #[test]
    fn test_tui_state_log_max_limit() {
        let state = TuiState::new();

        // Add more than MAX_LOGS entries
        for i in 0..1100 {
            state.log_info(format!("Log {}", i));
        }

        // Should be limited to MAX_LOGS (1000)
        assert_eq!(state.get_log_count(), 1000);
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Warn);
    }

    #[test]
    fn test_log_entry_clone() {
        let entry = LogEntry::info("test message");
        let cloned = entry.clone();
        assert_eq!(cloned.level, entry.level);
        assert_eq!(cloned.message, entry.message);
        assert_eq!(cloned.timestamp, entry.timestamp);
    }

    #[test]
    fn test_tui_state_log_custom_entry() {
        let state = TuiState::new();

        let custom_entry = LogEntry {
            level: LogLevel::Debug,
            message: "Custom debug".to_string(),
            timestamp: "12:00:00".to_string(),
            created_at: std::time::Instant::now(),
        };

        state.log(custom_entry);

        let logs = state.get_logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].level, LogLevel::Debug);
        assert_eq!(logs[0].message, "Custom debug");
        assert_eq!(logs[0].timestamp, "12:00:00");
    }

    #[test]
    fn test_tui_state_idle_enabled() {
        let state = TuiState::new();

        // Default should be false
        assert!(!state.is_idle_enabled());

        state.set_idle_enabled(true);
        assert!(state.is_idle_enabled());

        state.set_idle_enabled(false);
        assert!(!state.is_idle_enabled());
    }

    #[test]
    fn test_tui_state_idle_active() {
        let state = TuiState::new();

        // Default should be false
        assert!(!state.is_idle_active());

        state.set_idle_active(true);
        assert!(state.is_idle_active());

        state.set_idle_active(false);
        assert!(!state.is_idle_active());
    }

    #[test]
    fn test_tui_state_idle_cpu_gpu() {
        let state = TuiState::new();

        // Defaults should be true
        assert!(state.is_idle_cpu());
        assert!(state.is_idle_gpu());

        state.set_idle_cpu(false);
        assert!(!state.is_idle_cpu());

        state.set_idle_gpu(false);
        assert!(!state.is_idle_gpu());

        // Can set independently
        state.set_idle_cpu(true);
        assert!(state.is_idle_cpu());
        assert!(!state.is_idle_gpu());
    }

    #[test]
    fn test_tui_state_idle_combined() {
        let state = TuiState::new();

        // Simulate idle mode setup
        state.set_idle_enabled(true);
        state.set_idle_cpu(true);
        state.set_idle_gpu(true);

        // Simulate system becoming idle
        state.set_idle_active(true);

        assert!(state.is_idle_enabled());
        assert!(state.is_idle_active());
        assert!(state.is_idle_cpu());
        assert!(state.is_idle_gpu());

        // Simulate user becoming active
        state.set_idle_active(false);
        assert!(!state.is_idle_active());

        // Settings should remain
        assert!(state.is_idle_enabled());
        assert!(state.is_idle_cpu());
        assert!(state.is_idle_gpu());
    }
}
