use std::time::{Duration, Instant};

/// Hashrate calculator with sliding window
pub struct HashrateCalculator {
    samples: Vec<(Instant, u64)>,
    window_size: Duration,
}

impl HashrateCalculator {
    pub fn new(window_seconds: u64) -> Self {
        Self {
            samples: Vec::new(),
            window_size: Duration::from_secs(window_seconds),
        }
    }

    /// Add a hash count sample
    pub fn add_sample(&mut self, hashes: u64) {
        let now = Instant::now();

        // Remove old samples outside the window
        let cutoff = now - self.window_size;
        self.samples.retain(|(t, _)| *t > cutoff);

        // Add new sample
        self.samples.push((now, hashes));
    }

    /// Calculate current hashrate in H/s
    pub fn hashrate(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }

        let (first_time, first_hashes) = self.samples.first().unwrap();
        let (last_time, last_hashes) = self.samples.last().unwrap();

        let duration = last_time.duration_since(*first_time).as_secs_f64();
        if duration <= 0.0 {
            return 0.0;
        }

        let total_hashes = last_hashes.saturating_sub(*first_hashes);
        total_hashes as f64 / duration
    }
}

/// Format hashrate for display
pub fn format_hashrate(hashrate: f64) -> String {
    if hashrate >= 1e15 {
        format!("{:.2} PH/s", hashrate / 1e15)
    } else if hashrate >= 1e12 {
        format!("{:.2} TH/s", hashrate / 1e12)
    } else if hashrate >= 1e9 {
        format!("{:.2} GH/s", hashrate / 1e9)
    } else if hashrate >= 1e6 {
        format!("{:.2} MH/s", hashrate / 1e6)
    } else if hashrate >= 1e3 {
        format!("{:.2} KH/s", hashrate / 1e3)
    } else {
        format!("{:.2} H/s", hashrate)
    }
}

/// Format duration for display
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Calculate expected time to find a share/block
pub fn expected_time_to_find(hashrate: f64, difficulty: f64) -> Duration {
    if hashrate <= 0.0 || difficulty <= 0.0 {
        return Duration::MAX;
    }

    // Expected hashes to find a share at given difficulty
    // difficulty 1 = 2^32 hashes on average
    let expected_hashes = difficulty * (1u64 << 32) as f64;
    let seconds = expected_hashes / hashrate;

    Duration::from_secs_f64(seconds.min(u64::MAX as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hashrate() {
        assert_eq!(format_hashrate(1000.0), "1.00 KH/s");
        assert_eq!(format_hashrate(1_000_000.0), "1.00 MH/s");
        assert_eq!(format_hashrate(1_000_000_000.0), "1.00 GH/s");
    }

    #[test]
    fn test_format_hashrate_all_units() {
        assert_eq!(format_hashrate(500.0), "500.00 H/s");
        assert_eq!(format_hashrate(1.5e3), "1.50 KH/s");
        assert_eq!(format_hashrate(2.5e6), "2.50 MH/s");
        assert_eq!(format_hashrate(3.5e9), "3.50 GH/s");
        assert_eq!(format_hashrate(4.5e12), "4.50 TH/s");
        assert_eq!(format_hashrate(5.5e15), "5.50 PH/s");
    }

    #[test]
    fn test_format_duration_seconds() {
        let d = Duration::from_secs(45);
        assert_eq!(format_duration(d), "45s");
    }

    #[test]
    fn test_format_duration_minutes() {
        let d = Duration::from_secs(125); // 2m 5s
        assert_eq!(format_duration(d), "2m 5s");
    }

    #[test]
    fn test_format_duration_hours() {
        let d = Duration::from_secs(3725); // 1h 2m 5s
        assert_eq!(format_duration(d), "1h 2m 5s");
    }

    #[test]
    fn test_format_duration_days() {
        let d = Duration::from_secs(90125); // 1d 1h 2m
        assert_eq!(format_duration(d), "1d 1h 2m");
    }

    #[test]
    fn test_hashrate_calculator() {
        let mut calc = HashrateCalculator::new(60);
        calc.add_sample(0);
        std::thread::sleep(Duration::from_millis(100));
        calc.add_sample(1000);

        let hr = calc.hashrate();
        assert!(hr > 0.0);
    }

    #[test]
    fn test_hashrate_calculator_no_samples() {
        let calc = HashrateCalculator::new(60);
        assert_eq!(calc.hashrate(), 0.0);
    }

    #[test]
    fn test_hashrate_calculator_one_sample() {
        let mut calc = HashrateCalculator::new(60);
        calc.add_sample(1000);
        assert_eq!(calc.hashrate(), 0.0);
    }

    #[test]
    fn test_expected_time_to_find() {
        // At 1 GH/s with difficulty 1, should be about 4.295 seconds
        let hr = 1e9; // 1 GH/s
        let diff = 1.0;
        let time = expected_time_to_find(hr, diff);

        // 2^32 hashes / 1e9 H/s ≈ 4295 seconds
        // The function should return a reasonable time
        assert!(time.as_secs() > 0);
        assert!(time < Duration::MAX);
    }

    #[test]
    fn test_expected_time_to_find_zero_hashrate() {
        let time = expected_time_to_find(0.0, 1.0);
        assert_eq!(time, Duration::MAX);
    }

    #[test]
    fn test_expected_time_to_find_zero_difficulty() {
        let time = expected_time_to_find(1e12, 0.0);
        assert_eq!(time, Duration::MAX);
    }

    #[test]
    fn test_expected_time_to_find_negative() {
        let time = expected_time_to_find(-1.0, 1.0);
        assert_eq!(time, Duration::MAX);

        let time = expected_time_to_find(1e12, -1.0);
        assert_eq!(time, Duration::MAX);
    }

    #[test]
    fn test_format_hashrate_zero() {
        assert_eq!(format_hashrate(0.0), "0.00 H/s");
    }

    #[test]
    fn test_format_hashrate_very_small() {
        assert_eq!(format_hashrate(0.001), "0.00 H/s");
        assert_eq!(format_hashrate(0.5), "0.50 H/s");
    }

    #[test]
    fn test_format_hashrate_negative() {
        // Negative hashrate should still format (even if semantically invalid)
        let result = format_hashrate(-100.0);
        assert!(result.contains("H/s"));
    }

    #[test]
    fn test_format_hashrate_boundary_values() {
        // Just under 1 KH/s
        assert_eq!(format_hashrate(999.99), "999.99 H/s");
        // Exactly 1 KH/s
        assert_eq!(format_hashrate(1000.0), "1.00 KH/s");
        // Just under 1 MH/s
        let result = format_hashrate(999_999.0);
        assert!(result.contains("KH/s"));
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(Duration::ZERO), "0s");
    }

    #[test]
    fn test_format_duration_exact_boundaries() {
        // Exactly 1 minute
        assert_eq!(format_duration(Duration::from_secs(60)), "1m 0s");
        // Exactly 1 hour
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h 0m 0s");
        // Exactly 1 day
        assert_eq!(format_duration(Duration::from_secs(86400)), "1d 0h 0m");
    }

    #[test]
    fn test_expected_time_very_high_difficulty() {
        // Very high difficulty should not overflow
        let hr = 1e15; // 1 PH/s
        let diff = 1e18; // Extremely high difficulty
        let time = expected_time_to_find(hr, diff);
        // Should still be a valid duration
        assert!(time > Duration::ZERO);
    }

    #[test]
    fn test_hashrate_calculator_window_expiry() {
        // Create a calculator with 1 second window
        let mut calc = HashrateCalculator::new(1);
        calc.add_sample(0);

        // Sleep longer than the window
        std::thread::sleep(Duration::from_millis(1100));

        // Old sample should be expired, this should be the only sample
        calc.add_sample(1000);

        // With only one sample after expiry, hashrate should be 0
        assert_eq!(calc.hashrate(), 0.0);
    }

    #[test]
    fn test_hashrate_calculator_many_samples() {
        let mut calc = HashrateCalculator::new(60);

        // Add many samples
        for i in 0..100 {
            calc.add_sample(i * 1000);
            std::thread::sleep(Duration::from_millis(1));
        }

        let hr = calc.hashrate();
        assert!(hr > 0.0, "Hashrate should be positive with many samples");
    }

    #[test]
    fn test_hashrate_calculator_saturating_sub() {
        let mut calc = HashrateCalculator::new(60);

        // Add samples where last < first (shouldn't happen normally, but test the safety)
        calc.samples.push((Instant::now(), 1000));
        std::thread::sleep(Duration::from_millis(10));
        calc.samples.push((Instant::now(), 500));

        // Should return 0 due to saturating_sub
        let hr = calc.hashrate();
        assert_eq!(hr, 0.0);
    }
}
