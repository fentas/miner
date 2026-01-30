//! Bitcoin Core log tailing
//!
//! This module provides functionality to tail Bitcoin Core's debug.log
//! and send entries to the TUI.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::tui::{LogEntry, LogLevel, TuiState};

/// Tail Bitcoin Core debug.log and send entries to TUI
///
/// This function reads the Bitcoin Core debug.log file continuously,
/// parsing log entries and sending them to the TUI state for display.
/// It starts from near the end of the file to show recent history.
pub async fn tail_bitcoin_log(path: PathBuf, tui: Arc<TuiState>) {
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
                                let entry = parse_bitcoin_log_line(line);
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

/// Parse a Bitcoin Core log line into a LogEntry
///
/// Bitcoin Core format: "2026-01-29T17:54:45Z [category] message"
fn parse_bitcoin_log_line(line: &str) -> LogEntry {
    // Extract timestamp and message
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

    LogEntry::with_timestamp(level, msg, timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bitcoin_log_line_normal() {
        let line = "2026-01-29T17:54:45Z [net] Received block header";
        let entry = parse_bitcoin_log_line(line);
        assert_eq!(entry.timestamp, "17:54:45");
        assert_eq!(entry.level, LogLevel::Info);
    }

    #[test]
    fn test_parse_bitcoin_log_line_error() {
        let line = "2026-01-29T17:54:45Z [validation] error: Invalid block";
        let entry = parse_bitcoin_log_line(line);
        assert_eq!(entry.timestamp, "17:54:45");
        assert_eq!(entry.level, LogLevel::Error);
    }

    #[test]
    fn test_parse_bitcoin_log_line_warning() {
        let line = "2026-01-29T17:54:45Z [net] warning: Slow peer detected";
        let entry = parse_bitcoin_log_line(line);
        assert_eq!(entry.timestamp, "17:54:45");
        assert_eq!(entry.level, LogLevel::Warn);
    }

    #[test]
    fn test_parse_bitcoin_log_line_malformed() {
        let line = "Some random log without timestamp";
        let entry = parse_bitcoin_log_line(line);
        assert_eq!(entry.timestamp, "??:??:??");
        assert_eq!(entry.level, LogLevel::Info);
    }
}
