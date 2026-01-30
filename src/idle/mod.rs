//! Idle Monitor - Screensaver-style mining
//!
//! Starts/stops mining based on user activity:
//! - Linux (X11): Uses xprintidle or xssstate
//! - WSL2: Uses PowerShell to query Windows idle time
//! - Windows: Uses GetLastInputInfo from user32.dll

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(not(target_os = "windows"))]
use std::process::Command;

/// Runtime environment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Unknown,
    NativeLinux,   // X11 or Wayland
    WSL2,          // Windows Subsystem for Linux
    NativeWindows, // Native Windows
    Headless,      // No display
}

impl Environment {
    pub fn name(&self) -> &'static str {
        match self {
            Environment::Unknown => "unknown",
            Environment::NativeLinux => "linux",
            Environment::WSL2 => "wsl2",
            Environment::NativeWindows => "windows",
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

        tracing::info!("Idle monitor starting in {} environment", self.env.name());

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
    // Native Windows
    #[cfg(target_os = "windows")]
    {
        return Environment::NativeWindows;
    }

    // Unix-like systems
    #[cfg(not(target_os = "windows"))]
    {
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
}

/// Get current idle time in seconds
fn get_idle_time(env: Environment) -> Option<u32> {
    match env {
        Environment::NativeLinux => get_idle_time_linux(),
        Environment::WSL2 => get_idle_time_wsl2(),
        Environment::NativeWindows => get_idle_time_windows(),
        _ => None,
    }
}

/// Get idle time on Linux via xprintidle
#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
fn get_idle_time_linux() -> Option<u32> {
    None // Not applicable on Windows
}

/// Get idle time on WSL2 via PowerShell
#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
fn get_idle_time_wsl2() -> Option<u32> {
    None // Not applicable on native Windows
}

/// Get idle time on native Windows using Win32 API
#[cfg(target_os = "windows")]
fn get_idle_time_windows() -> Option<u32> {
    use std::mem::size_of;

    #[repr(C)]
    struct LASTINPUTINFO {
        cb_size: u32,
        dw_time: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
        fn GetTickCount() -> u32;
    }

    unsafe {
        let mut lii = LASTINPUTINFO {
            cb_size: size_of::<LASTINPUTINFO>() as u32,
            dw_time: 0,
        };

        if GetLastInputInfo(&mut lii) != 0 {
            let tick_count = GetTickCount();
            let idle_ms = tick_count.wrapping_sub(lii.dw_time);
            Some(idle_ms / 1000)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_idle_time_windows() -> Option<u32> {
    None // Not applicable on non-Windows
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_name() {
        assert_eq!(Environment::Unknown.name(), "unknown");
        assert_eq!(Environment::NativeLinux.name(), "linux");
        assert_eq!(Environment::WSL2.name(), "wsl2");
        assert_eq!(Environment::NativeWindows.name(), "windows");
        assert_eq!(Environment::Headless.name(), "headless");
    }

    #[test]
    fn test_idle_monitor_new() {
        let monitor = IdleMonitor::new();
        assert!(!monitor.is_idle());
        assert_eq!(monitor.timeout_sec, 900); // Default 15 minutes
    }

    #[test]
    fn test_idle_monitor_default() {
        let monitor = IdleMonitor::default();
        assert!(!monitor.is_idle());
    }

    #[test]
    fn test_idle_monitor_set_timeout() {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(600);
        assert_eq!(monitor.timeout_sec, 600);
    }

    #[test]
    fn test_idle_monitor_environment() {
        let monitor = IdleMonitor::new();
        let env = monitor.environment();
        // Should detect some environment (may vary by system)
        assert!(matches!(
            env,
            Environment::Unknown | Environment::NativeLinux |
            Environment::WSL2 | Environment::NativeWindows | Environment::Headless
        ));
    }

    #[test]
    fn test_idle_monitor_is_idle_initial() {
        let monitor = IdleMonitor::new();
        // Initially not idle
        assert!(!monitor.is_idle());
    }

    #[test]
    fn test_idle_monitor_zero_timeout_doesnt_start() {
        let mut monitor = IdleMonitor::new();
        monitor.set_timeout(0);
        monitor.start();
        // With timeout 0, monitor shouldn't actually start
        // (the running flag should still be false since start returns early)
        assert!(!monitor.running.load(Ordering::Relaxed));
    }

    #[test]
    fn test_idle_monitor_stop_when_not_running() {
        let mut monitor = IdleMonitor::new();
        // Should not panic when stopping a monitor that was never started
        monitor.stop();
        assert!(!monitor.is_idle());
    }

    #[test]
    fn test_get_idle_time_unknown_env() {
        let result = get_idle_time(Environment::Unknown);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_idle_time_headless_env() {
        let result = get_idle_time(Environment::Headless);
        assert!(result.is_none());
    }

    #[test]
    fn test_idle_monitor_set_callback() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let mut monitor = IdleMonitor::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);

        monitor.set_callback(move |_idle| {
            called_clone.store(true, Ordering::Relaxed);
        });

        assert!(monitor.callback.is_some());
    }

    #[test]
    fn test_environment_equality() {
        assert_eq!(Environment::WSL2, Environment::WSL2);
        assert_ne!(Environment::WSL2, Environment::NativeLinux);
    }

    #[test]
    fn test_detect_environment_returns_valid() {
        let env = detect_environment();
        // The function should always return a valid Environment variant
        let valid = matches!(
            env,
            Environment::Unknown | Environment::NativeLinux |
            Environment::WSL2 | Environment::NativeWindows | Environment::Headless
        );
        assert!(valid);
    }

    #[test]
    fn test_environment_clone() {
        let env = Environment::WSL2;
        let cloned = env;
        assert_eq!(env, cloned);
    }

    #[test]
    fn test_environment_copy() {
        let env = Environment::NativeLinux;
        let copied = env;
        assert_eq!(env, copied);
    }

    #[test]
    fn test_environment_debug() {
        let env = Environment::NativeWindows;
        let debug_str = format!("{:?}", env);
        assert_eq!(debug_str, "NativeWindows");
    }

    #[test]
    fn test_idle_monitor_double_stop() {
        let mut monitor = IdleMonitor::new();
        // Should not panic when stopping twice
        monitor.stop();
        monitor.stop();
        assert!(!monitor.is_idle());
    }

    #[test]
    fn test_get_idle_time_for_each_env() {
        // Test all environment variants
        assert!(get_idle_time(Environment::Unknown).is_none());
        assert!(get_idle_time(Environment::Headless).is_none());
        // Native environments may or may not return a value depending on system
    }

    #[test]
    fn test_idle_monitor_running_flag() {
        let mut monitor = IdleMonitor::new();
        // Before start, running should be false
        assert!(!monitor.running.load(Ordering::Relaxed));

        // After setting timeout to 0, start should be a no-op
        monitor.set_timeout(0);
        monitor.start();
        assert!(!monitor.running.load(Ordering::Relaxed));
    }

    #[test]
    fn test_idle_monitor_currently_idle_atomic() {
        let monitor = IdleMonitor::new();
        // Verify currently_idle is accessible and defaults to false
        assert!(!monitor.currently_idle.load(Ordering::Relaxed));
    }

    #[test]
    fn test_idle_callback_type() {
        // Test that callback type works correctly
        let mut monitor = IdleMonitor::new();
        let was_called = Arc::new(AtomicBool::new(false));
        let was_called_clone = Arc::clone(&was_called);

        monitor.set_callback(move |idle| {
            was_called_clone.store(idle, Ordering::Relaxed);
        });

        // Callback should be set but not called yet
        assert!(monitor.callback.is_some());
        assert!(!was_called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_idle_monitor_thread_none_initially() {
        let monitor = IdleMonitor::new();
        assert!(monitor.thread.is_none());
    }
}
