//! Log types and filtering for TUI display
//!
//! This module contains types for log entries, levels, and time-based filtering.

use std::time::Instant;

/// Time filter for logs (k9s style: 0=tail, 1=1m, 2=5m, 3=15m, 4=30m, 5=1h, 6=all)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LogTimeFilter {
    #[default]
    Tail,       // 0 - Show last N logs (default 100)
    OneMin,     // 1 - Last 1 minute
    FiveMin,    // 2 - Last 5 minutes
    FifteenMin, // 3 - Last 15 minutes
    ThirtyMin,  // 4 - Last 30 minutes
    OneHour,    // 5 - Last 1 hour
    All,        // 6 - All logs (up to MAX_LOGS)
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
            LogTimeFilter::Tail => None, // Handled separately (last N)
            LogTimeFilter::OneMin => Some(std::time::Duration::from_secs(60)),
            LogTimeFilter::FiveMin => Some(std::time::Duration::from_secs(5 * 60)),
            LogTimeFilter::FifteenMin => Some(std::time::Duration::from_secs(15 * 60)),
            LogTimeFilter::ThirtyMin => Some(std::time::Duration::from_secs(30 * 60)),
            LogTimeFilter::OneHour => Some(std::time::Duration::from_secs(60 * 60)),
            LogTimeFilter::All => None, // Show all
        }
    }

    pub fn is_tail(&self) -> bool {
        matches!(self, LogTimeFilter::Tail)
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
    pub fn with_timestamp(
        level: LogLevel,
        message: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            level,
            created_at: Instant::now(),
            message: message.into(),
            timestamp: timestamp.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Success.as_str(), "OK");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Warn);
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
    fn test_log_entry_clone() {
        let entry = LogEntry::info("test message");
        let cloned = entry.clone();
        assert_eq!(cloned.level, entry.level);
        assert_eq!(cloned.message, entry.message);
        assert_eq!(cloned.timestamp, entry.timestamp);
    }

    #[test]
    fn test_log_entry_with_timestamp() {
        let entry = LogEntry::with_timestamp(LogLevel::Info, "message", "12:00:00");
        assert_eq!(entry.timestamp, "12:00:00");
    }

    #[test]
    fn test_log_time_filter_as_str() {
        assert_eq!(LogTimeFilter::Tail.as_str(), "tail");
        assert_eq!(LogTimeFilter::OneMin.as_str(), "1m");
        assert_eq!(LogTimeFilter::FiveMin.as_str(), "5m");
        assert_eq!(LogTimeFilter::FifteenMin.as_str(), "15m");
        assert_eq!(LogTimeFilter::ThirtyMin.as_str(), "30m");
        assert_eq!(LogTimeFilter::OneHour.as_str(), "1h");
        assert_eq!(LogTimeFilter::All.as_str(), "all");
    }

    #[test]
    fn test_log_time_filter_from_key() {
        assert_eq!(LogTimeFilter::from_key('0'), Some(LogTimeFilter::Tail));
        assert_eq!(LogTimeFilter::from_key('1'), Some(LogTimeFilter::OneMin));
        assert_eq!(LogTimeFilter::from_key('2'), Some(LogTimeFilter::FiveMin));
        assert_eq!(LogTimeFilter::from_key('3'), Some(LogTimeFilter::FifteenMin));
        assert_eq!(LogTimeFilter::from_key('4'), Some(LogTimeFilter::ThirtyMin));
        assert_eq!(LogTimeFilter::from_key('5'), Some(LogTimeFilter::OneHour));
        assert_eq!(LogTimeFilter::from_key('6'), Some(LogTimeFilter::All));
        assert_eq!(LogTimeFilter::from_key('7'), None);
        assert_eq!(LogTimeFilter::from_key('x'), None);
    }

    #[test]
    fn test_log_time_filter_max_age() {
        assert!(LogTimeFilter::Tail.max_age().is_none());
        assert_eq!(
            LogTimeFilter::OneMin.max_age(),
            Some(std::time::Duration::from_secs(60))
        );
        assert_eq!(
            LogTimeFilter::FiveMin.max_age(),
            Some(std::time::Duration::from_secs(300))
        );
        assert!(LogTimeFilter::All.max_age().is_none());
    }

    #[test]
    fn test_log_time_filter_is_tail() {
        assert!(LogTimeFilter::Tail.is_tail());
        assert!(!LogTimeFilter::OneMin.is_tail());
        assert!(!LogTimeFilter::All.is_tail());
    }

    #[test]
    fn test_log_time_filter_default() {
        let filter = LogTimeFilter::default();
        assert_eq!(filter, LogTimeFilter::Tail);
    }
}
