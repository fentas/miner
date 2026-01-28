use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Instant;

const MAX_LOGS: usize = 1000;
const MAX_HASHRATE_HISTORY: usize = 120; // 2 minutes at 1 sample/sec

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
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        let now = chrono::Local::now();
        Self {
            level,
            message: message.into(),
            timestamp: now.format("%H:%M:%S").to_string(),
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

    // Status
    pub paused: AtomicBool,
    connected: AtomicBool,
    syncing: AtomicBool,
    sync_progress: AtomicU64, // Stored as progress * 100 (for decimal precision)

    // Timing
    start_time: Instant,
    last_share_time: RwLock<Option<Instant>>,
    last_block_time: RwLock<Option<Instant>>,

    // Logs
    logs: RwLock<VecDeque<LogEntry>>,
    log_scroll: AtomicUsize,

    // Control
    should_quit: AtomicBool,
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
            paused: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            syncing: AtomicBool::new(false),
            sync_progress: AtomicU64::new(0),
            start_time: Instant::now(),
            last_share_time: RwLock::new(None),
            last_block_time: RwLock::new(None),
            logs: RwLock::new(VecDeque::with_capacity(MAX_LOGS)),
            log_scroll: AtomicUsize::new(0),
            should_quit: AtomicBool::new(false),
        }
    }

    // Setters
    pub fn set_hashrate(&self, rate: f64) {
        self.hashrate.store(rate.to_bits(), Ordering::Relaxed);

        // Add to history
        if let Ok(mut history) = self.hashrate_history.write() {
            if history.len() >= MAX_HASHRATE_HISTORY {
                history.pop_front();
            }
            history.push_back(rate);
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

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    pub fn set_syncing(&self, syncing: bool, progress: f64) {
        self.syncing.store(syncing, Ordering::Relaxed);
        self.sync_progress.store((progress * 100.0) as u64, Ordering::Relaxed);
    }

    pub fn toggle_pause(&self) -> bool {
        let prev = self.paused.fetch_xor(true, Ordering::Relaxed);
        !prev
    }

    pub fn set_quit(&self) {
        self.should_quit.store(true, Ordering::Relaxed);
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
        self.log_scroll.store(0, Ordering::Relaxed);
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
}
