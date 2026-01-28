//! Idle Monitor - Screensaver-style mining
//!
//! Starts/stops mining based on user activity:
//! - Linux (X11): Uses xprintidle or xssstate
//! - WSL2: Uses PowerShell to query Windows idle time

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Runtime environment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Unknown,
    NativeLinux,  // X11 or Wayland
    WSL2,         // Windows Subsystem for Linux
    Headless,     // No display
}

impl Environment {
    pub fn name(&self) -> &'static str {
        match self {
            Environment::Unknown => "unknown",
            Environment::NativeLinux => "linux",
            Environment::WSL2 => "wsl2",
            Environment::Headless => "headless",
        }
    }
}

/// Callback type for idle state changes
pub type IdleCallback = Box<dyn Fn(bool) + Send + 'static>;

/// Idle monitor for screensaver-style mining
pub struct IdleMonitor {
    env: Environment,
    timeout_sec: u32,
    running: Arc<AtomicBool>,
    currently_idle: Arc<AtomicBool>,
    callback: Option<IdleCallback>,
    thread: Option<thread::JoinHandle<()>>,
}

impl IdleMonitor {
    /// Create a new idle monitor
    pub fn new() -> Self {
        let env = detect_environment();
        Self {
            env,
            timeout_sec: 900, // 15 minutes default
            running: Arc::new(AtomicBool::new(false)),
            currently_idle: Arc::new(AtomicBool::new(false)),
            callback: None,
            thread: None,
        }
    }

    /// Set idle timeout in seconds
    pub fn set_timeout(&mut self, seconds: u32) {
        self.timeout_sec = seconds;
    }

    /// Set callback for idle state changes
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(bool) + Send + 'static,
    {
        self.callback = Some(Box::new(callback));
    }

    /// Get detected environment
    pub fn environment(&self) -> Environment {
        self.env
    }

    /// Check if currently idle
    pub fn is_idle(&self) -> bool {
        self.currently_idle.load(Ordering::Relaxed)
    }

    /// Start monitoring
    pub fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) || self.timeout_sec == 0 {
            return;
        }

        if self.env == Environment::Headless || self.env == Environment::Unknown {
            tracing::warn!("Idle monitoring not available in {} environment", self.env.name());
            return;
        }

        self.running.store(true, Ordering::Relaxed);

        let running = Arc::clone(&self.running);
        let currently_idle = Arc::clone(&self.currently_idle);
        let timeout_sec = self.timeout_sec;
        let env = self.env;

        self.thread = Some(thread::spawn(move || {
            monitor_loop(running, currently_idle, timeout_sec, env);
        }));

        tracing::info!("Idle monitor started (timeout: {}s, env: {})", self.timeout_sec, self.env.name());
    }

    /// Stop monitoring
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Default for IdleMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IdleMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Detect runtime environment
fn detect_environment() -> Environment {
    // Check for WSL2
    if let Ok(content) = std::fs::read_to_string("/proc/version") {
        if content.to_lowercase().contains("microsoft") || content.to_lowercase().contains("wsl") {
            return Environment::WSL2;
        }
    }

    // Check for display (X11)
    if std::env::var("DISPLAY").is_ok() {
        // Check if xprintidle or xssstate is available
        if Command::new("which").arg("xprintidle").output().map(|o| o.status.success()).unwrap_or(false) {
            return Environment::NativeLinux;
        }
        if Command::new("which").arg("xssstate").output().map(|o| o.status.success()).unwrap_or(false) {
            return Environment::NativeLinux;
        }
    }

    // Check for Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Environment::NativeLinux;
    }

    Environment::Headless
}

/// Get current idle time in seconds
fn get_idle_time(env: Environment) -> Option<u32> {
    match env {
        Environment::NativeLinux => get_idle_time_linux(),
        Environment::WSL2 => get_idle_time_wsl2(),
        _ => None,
    }
}

/// Get idle time on Linux via xprintidle
fn get_idle_time_linux() -> Option<u32> {
    // Try xprintidle first
    if let Ok(output) = Command::new("xprintidle").output() {
        if output.status.success() {
            if let Ok(ms_str) = String::from_utf8(output.stdout) {
                if let Ok(ms) = ms_str.trim().parse::<u64>() {
                    return Some((ms / 1000) as u32);
                }
            }
        }
    }

    // Try xssstate as fallback
    if let Ok(output) = Command::new("xssstate").arg("-i").output() {
        if output.status.success() {
            if let Ok(ms_str) = String::from_utf8(output.stdout) {
                if let Ok(ms) = ms_str.trim().parse::<u64>() {
                    return Some((ms / 1000) as u32);
                }
            }
        }
    }

    None
}

/// Get idle time on WSL2 via PowerShell
fn get_idle_time_wsl2() -> Option<u32> {
    let script = r#"
        Add-Type @'
        using System;
        using System.Runtime.InteropServices;
        public class IdleTime {
            [DllImport("user32.dll")]
            static extern bool GetLastInputInfo(ref LASTINPUTINFO plii);
            [StructLayout(LayoutKind.Sequential)]
            struct LASTINPUTINFO {
                public uint cbSize;
                public uint dwTime;
            }
            public static uint GetIdleTime() {
                LASTINPUTINFO lii = new LASTINPUTINFO();
                lii.cbSize = (uint)Marshal.SizeOf(lii);
                if (GetLastInputInfo(ref lii)) {
                    return ((uint)Environment.TickCount - lii.dwTime) / 1000;
                }
                return 0;
            }
        }
'@
        [IdleTime]::GetIdleTime()
    "#;

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()?;

    if output.status.success() {
        let output_str = String::from_utf8(output.stdout).ok()?;
        output_str.trim().parse::<u32>().ok()
    } else {
        None
    }
}

/// Monitor loop
fn monitor_loop(
    running: Arc<AtomicBool>,
    currently_idle: Arc<AtomicBool>,
    timeout_sec: u32,
    env: Environment,
) {
    let poll_interval = Duration::from_secs(5);
    let mut was_idle = false;

    while running.load(Ordering::Relaxed) {
        if let Some(idle_time) = get_idle_time(env) {
            let is_idle = idle_time >= timeout_sec;

            if is_idle != was_idle {
                currently_idle.store(is_idle, Ordering::Relaxed);
                if is_idle {
                    tracing::info!("User idle for {}s, enabling mining", idle_time);
                } else {
                    tracing::info!("User active, pausing mining");
                }
                was_idle = is_idle;
            }
        }

        thread::sleep(poll_interval);
    }
}
