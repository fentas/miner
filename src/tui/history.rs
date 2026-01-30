//! Hashrate history types for TUI display
//!
//! This module contains types for tracking hashrate samples and display modes.

use std::time::Instant;

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

/// Hashrate display mode
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum HashrateDisplayMode {
    #[default]
    Total,         // Show total (CPU + GPU combined)
    CpuOnly,       // Show CPU hashrate only
    GpuOnly,       // Show GPU hashrate only (combined all GPUs)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hashrate_display_mode_default() {
        let mode = HashrateDisplayMode::default();
        assert_eq!(mode, HashrateDisplayMode::Total);
    }

    #[test]
    fn test_hashrate_display_mode_as_str() {
        assert_eq!(HashrateDisplayMode::Total.as_str(), "Total");
        assert_eq!(HashrateDisplayMode::CpuOnly.as_str(), "CPU");
        assert_eq!(HashrateDisplayMode::GpuOnly.as_str(), "GPU");
        assert_eq!(HashrateDisplayMode::GpuDevice(0).as_str(), "GPU#");
    }

    #[test]
    fn test_hashrate_display_mode_next_no_gpu() {
        let mode = HashrateDisplayMode::Total;
        let next = mode.next(0);
        assert_eq!(next, HashrateDisplayMode::CpuOnly);

        // CpuOnly goes back to Total when no GPU
        let next = next.next(0);
        assert_eq!(next, HashrateDisplayMode::Total);
    }

    #[test]
    fn test_hashrate_display_mode_next_single_gpu() {
        let mode = HashrateDisplayMode::Total;
        let next = mode.next(1);
        assert_eq!(next, HashrateDisplayMode::CpuOnly);

        let next = next.next(1);
        assert_eq!(next, HashrateDisplayMode::GpuOnly);

        // With single GPU, GpuOnly goes back to Total (no device cycling)
        let next = next.next(1);
        assert_eq!(next, HashrateDisplayMode::Total);
    }

    #[test]
    fn test_hashrate_display_mode_next_multi_gpu() {
        let mode = HashrateDisplayMode::Total;
        let next = mode.next(3);
        assert_eq!(next, HashrateDisplayMode::CpuOnly);

        let next = next.next(3);
        assert_eq!(next, HashrateDisplayMode::GpuOnly);

        // With multiple GPUs, GpuOnly goes to GpuDevice(0)
        let next = next.next(3);
        assert_eq!(next, HashrateDisplayMode::GpuDevice(0));

        let next = next.next(3);
        assert_eq!(next, HashrateDisplayMode::GpuDevice(1));

        let next = next.next(3);
        assert_eq!(next, HashrateDisplayMode::GpuDevice(2));

        // After last device, back to Total
        let next = next.next(3);
        assert_eq!(next, HashrateDisplayMode::Total);
    }

    #[test]
    fn test_hashrate_sample_clone() {
        let sample = HashrateSample {
            timestamp: Instant::now(),
            cpu_hashrate: 100.0,
            gpu_hashrate: 200.0,
            gpu_device_hashrates: vec![100.0, 100.0],
            total_hashrate: 300.0,
            block_height: 800000,
            new_block: false,
        };

        let cloned = sample.clone();
        assert_eq!(cloned.cpu_hashrate, sample.cpu_hashrate);
        assert_eq!(cloned.gpu_hashrate, sample.gpu_hashrate);
        assert_eq!(cloned.total_hashrate, sample.total_hashrate);
        assert_eq!(cloned.block_height, sample.block_height);
        assert_eq!(cloned.new_block, sample.new_block);
    }
}
