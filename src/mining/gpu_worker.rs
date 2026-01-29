//! GPU Mining Worker
//!
//! Manages GPU mining threads that run alongside CPU workers.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tokio::sync::watch;

use super::{Job, Stats, Midstate};
use crate::gpu::{GpuManager, GpuMiner, GpuDevice};

/// GPU Worker handle
pub struct GpuWorkerHandle {
    pub device: GpuDevice,
    handle: JoinHandle<()>,
    /// Per-device hashrate (stored as f64 bits)
    hashrate: Arc<AtomicU64>,
}

impl GpuWorkerHandle {
    pub fn join(self) -> thread::Result<()> {
        self.handle.join()
    }

    /// Get this device's hashrate
    pub fn hashrate(&self) -> f64 {
        f64::from_bits(self.hashrate.load(Ordering::Relaxed))
    }
}

/// GPU Mining Coordinator
/// Manages multiple GPU mining threads
pub struct GpuCoordinator {
    workers: Vec<GpuWorkerHandle>,
    stats: Arc<Stats>,
    running: Arc<AtomicBool>,
    job_tx: watch::Sender<Option<GpuJob>>,
}

/// GPU-specific job with pre-computed midstate
#[derive(Clone)]
pub struct GpuJob {
    pub job_id: String,
    pub midstate: [u32; 8],
    pub tail: [u8; 16],
    pub target: [u8; 32],
    pub network_target: Option<[u8; 32]>,
}

/// GPU tuning parameters
#[derive(Clone, Debug)]
pub struct GpuTuningParams {
    /// Intensity (1-100)
    pub intensity: u8,
    /// Work group size (0 = auto)
    pub workgroup_size: usize,
    /// Batch size exponent (e.g., 25 = 2^25 nonces)
    pub batch_exp: u8,
}

impl Default for GpuTuningParams {
    fn default() -> Self {
        Self {
            intensity: 100,
            workgroup_size: 0,
            batch_exp: 25,
        }
    }
}

impl GpuCoordinator {
    /// Create a new GPU coordinator
    ///
    /// # Arguments
    /// * `device_ids` - List of GPU device IDs to use (empty = all available)
    /// * `intensity` - Mining intensity (1-100)
    /// * `stats` - Shared stats for hash counting
    pub fn new(device_ids: &[u32], intensity: u8, stats: Arc<Stats>) -> Self {
        Self::with_tuning(device_ids, GpuTuningParams {
            intensity,
            ..Default::default()
        }, stats)
    }

    /// Create a new GPU coordinator with custom tuning parameters
    pub fn with_tuning(device_ids: &[u32], tuning: GpuTuningParams, stats: Arc<Stats>) -> Self {
        let running = Arc::new(AtomicBool::new(false));
        let (job_tx, _job_rx) = watch::channel(None);

        let gpu_manager = GpuManager::new();
        let available_devices = gpu_manager.devices();

        let mut workers = Vec::new();

        // Determine which devices to use
        let devices_to_use: Vec<&GpuDevice> = if device_ids.is_empty() {
            // Use all available devices
            available_devices.iter().collect()
        } else {
            // Use only specified devices
            available_devices.iter()
                .filter(|d| device_ids.contains(&d.id))
                .collect()
        };

        // Clamp batch_exp to valid range (20-28)
        let batch_exp = tuning.batch_exp.clamp(20, 28);
        let batch_size: u32 = 1 << batch_exp;

        for device in devices_to_use {
            match gpu_manager.create_miner(device.id, tuning.intensity) {
                Ok(miner) => {
                    let device_clone = device.clone();
                    // Create per-device hashrate tracker
                    let device_hashrate = Arc::new(AtomicU64::new(0));
                    let handle = Self::spawn_worker(
                        miner,
                        device_clone.clone(),
                        Arc::clone(&stats),
                        Arc::clone(&running),
                        job_tx.subscribe(),
                        Arc::clone(&device_hashrate),
                        batch_size,
                    );
                    workers.push(GpuWorkerHandle {
                        device: device_clone,
                        handle,
                        hashrate: device_hashrate,
                    });
                }
                Err(e) => {
                    eprintln!("Failed to create GPU miner for device {}: {:?}", device.id, e);
                }
            }
        }

        Self {
            workers,
            stats,
            running,
            job_tx,
        }
    }

    /// Spawn a GPU worker thread
    fn spawn_worker(
        mut miner: Box<dyn GpuMiner>,
        device: GpuDevice,
        stats: Arc<Stats>,
        running: Arc<AtomicBool>,
        mut job_rx: watch::Receiver<Option<GpuJob>>,
        hashrate: Arc<AtomicU64>,
        batch_size: u32,
    ) -> JoinHandle<()> {
        thread::Builder::new()
            .name(format!("gpu-miner-{}", device.id))
            .spawn(move || {
                let mut nonce: u32 = device.id * (u32::MAX / 16); // Offset by device ID
                let mut last_job_id = String::new();

                loop {
                    // Wait for running flag
                    while !running.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }

                    // Check for job
                    let job = {
                        let job_ref = job_rx.borrow_and_update();
                        job_ref.clone()
                    };

                    let job = match job {
                        Some(j) => j,
                        None => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        }
                    };

                    // Reset nonce on new job
                    if job.job_id != last_job_id {
                        last_job_id = job.job_id.clone();
                        nonce = device.id * (u32::MAX / 16);

                        // Set job on GPU
                        miner.set_job(&job.midstate, &job.tail, &job.target);
                    }

                    // Check if job changed - if error or changed, get new job before mining
                    // Use unwrap_or(false) so we don't skip mining on channel errors
                    if job_rx.has_changed().unwrap_or(false) {
                        continue;
                    }

                    // Mine a batch
                    let results = miner.mine(nonce, batch_size);

                    // Update stats with actual hashes done
                    let hashes_this_batch = batch_size as u64;
                    stats.add_gpu_hashes(hashes_this_batch);

                    // Store current hashrate
                    hashrate.store(miner.hashrate().to_bits(), Ordering::Relaxed);

                    // Process results
                    for result in results {
                        // Check if meets network target (for solo mining)
                        if let Some(ref network_target) = job.network_target {
                            if meets_target(&result.hash, network_target) {
                                stats.found_block();
                                eprintln!(
                                    "[GPU {}] Found potential block! Nonce: {:#x}",
                                    device.id, result.nonce
                                );
                            }
                        }
                    }

                    // Advance nonce
                    nonce = nonce.wrapping_add(batch_size);
                }
            })
            .expect("Failed to spawn GPU worker thread")
    }

    /// Create a GPU job from a mining Job
    pub fn create_gpu_job(job: &Job) -> GpuJob {
        // Build header to extract midstate components
        let header = job.build_header(&[], &[], 0);

        // Compute midstate from first 64 bytes
        let mut header_prefix = [0u8; 64];
        header_prefix.copy_from_slice(&header[..64]);
        let midstate = Midstate::from_header_prefix(&header_prefix);

        // Extract tail (bytes 64-76, before nonce)
        let mut tail = [0u8; 16];
        tail[..12].copy_from_slice(&header[64..76]);
        // Bytes 76-80 are nonce, will be set by GPU

        GpuJob {
            job_id: job.id.clone(),
            midstate: midstate.state,
            tail,
            target: job.target,
            network_target: job.network_target,
        }
    }

    /// Start GPU mining
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop GPU mining
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if GPU mining is active
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Update the current job for all GPU workers
    pub fn update_job(&self, job: &Job) {
        let gpu_job = Self::create_gpu_job(job);
        let _ = self.job_tx.send(Some(gpu_job));
    }

    /// Clear the current job
    pub fn clear_job(&self) {
        let _ = self.job_tx.send(None);
    }

    /// Get number of GPU workers
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Get combined GPU hashrate (sum of all devices)
    pub fn hashrate(&self) -> f64 {
        self.workers.iter().map(|w| w.hashrate()).sum()
    }

    /// Get per-device hashrates
    pub fn device_hashrates(&self) -> Vec<(u32, f64)> {
        self.workers.iter()
            .map(|w| (w.device.id, w.hashrate()))
            .collect()
    }

    /// Get hashrate for a specific device
    pub fn device_hashrate(&self, device_id: u32) -> Option<f64> {
        self.workers.iter()
            .find(|w| w.device.id == device_id)
            .map(|w| w.hashrate())
    }

    /// Get stats reference
    pub fn stats(&self) -> &Arc<Stats> {
        &self.stats
    }

    /// Get device info for all workers
    pub fn devices(&self) -> Vec<&GpuDevice> {
        self.workers.iter().map(|w| &w.device).collect()
    }
}

impl Drop for GpuCoordinator {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Compare hash to target (hash must be <= target)
fn meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    for i in 0..32 {
        if hash[i] < target[i] {
            return true;
        }
        if hash[i] > target[i] {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meets_target() {
        let target = [0x00, 0x00, 0x00, 0x1F, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        // Zero hash should meet target
        let zero_hash = [0x00; 32];
        assert!(meets_target(&zero_hash, &target));

        // High hash should not meet target
        let high_hash = [0xFF; 32];
        assert!(!meets_target(&high_hash, &target));
    }

    #[test]
    fn test_meets_target_edge_cases() {
        // Test exact match - hash equals target
        let target = [0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let hash = target;
        assert!(meets_target(&hash, &target), "Hash equal to target should meet target");

        // Test hash just below target
        let mut hash_below = target;
        hash_below[3] = 0x0F; // One less than target at byte 3
        assert!(meets_target(&hash_below, &target), "Hash below target should meet target");

        // Test hash just above target
        let mut hash_above = target;
        hash_above[3] = 0x11; // One more than target at byte 3
        assert!(!meets_target(&hash_above, &target), "Hash above target should not meet target");

        // Test all zeros target (impossible difficulty)
        let zero_target = [0x00; 32];
        let zero_hash = [0x00; 32];
        assert!(meets_target(&zero_hash, &zero_target), "Zero hash should meet zero target");

        // Test all ones target (easiest difficulty)
        let max_target = [0xFF; 32];
        let any_hash = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
                        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
                        0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00,
                        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        assert!(meets_target(&any_hash, &max_target), "Any hash should meet max target");
    }

    #[test]
    fn test_meets_target_comparison_order() {
        // Test that comparison is done from first byte (most significant)
        let target = [0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        // Hash with lower first bytes should meet target even if later bytes are higher
        let hash = [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(meets_target(&hash, &target), "Hash with lower prefix should meet target");
    }

    #[test]
    fn test_gpu_job_creation() {
        let mut job = Job::new("test-gpu-job".to_string());
        job.version = 0x20000000;
        job.ntime = 1706400000;
        job.nbits = 0x1d00ffff;
        job.prev_hash = [0x11; 32];
        job.merkle_root_override = Some([0x22; 32]);
        job.target = [0x00, 0x00, 0x00, 0x1F, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        let gpu_job = GpuCoordinator::create_gpu_job(&job);

        assert_eq!(gpu_job.job_id, "test-gpu-job");
        assert_eq!(gpu_job.midstate.len(), 8);
        assert_eq!(gpu_job.tail.len(), 16);
        assert_eq!(gpu_job.target, job.target);
    }

    #[test]
    fn test_gpu_job_with_network_target() {
        let mut job = Job::new("solo-job".to_string());
        job.version = 0x20000000;
        job.ntime = 1706400000;
        job.nbits = 0x1d00ffff;
        job.prev_hash = [0x33; 32];
        job.merkle_root_override = Some([0x44; 32]);
        job.target = [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let network_target = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F, 0xFF,
                              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                              0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        job.network_target = Some(network_target);

        let gpu_job = GpuCoordinator::create_gpu_job(&job);

        assert_eq!(gpu_job.job_id, "solo-job");
        assert_eq!(gpu_job.network_target, Some(network_target));
        // Pool target should be easier than network target for share submission
        assert_ne!(gpu_job.target, network_target);
    }

    #[test]
    fn test_gpu_job_midstate_is_deterministic() {
        let mut job = Job::new("deterministic-test".to_string());
        job.version = 0x20000000;
        job.ntime = 1706400000;
        job.nbits = 0x1d00ffff;
        job.prev_hash = [0x55; 32];
        job.merkle_root_override = Some([0x66; 32]);
        job.target = [0xFF; 32];

        let gpu_job1 = GpuCoordinator::create_gpu_job(&job);
        let gpu_job2 = GpuCoordinator::create_gpu_job(&job);

        // Same job should produce same midstate
        assert_eq!(gpu_job1.midstate, gpu_job2.midstate);
        assert_eq!(gpu_job1.tail, gpu_job2.tail);
    }

    #[test]
    fn test_gpu_job_different_jobs_different_midstate() {
        let mut job1 = Job::new("job1".to_string());
        job1.version = 0x20000000;
        job1.ntime = 1706400000;
        job1.nbits = 0x1d00ffff;
        job1.prev_hash = [0x11; 32];
        job1.merkle_root_override = Some([0x22; 32]);
        job1.target = [0xFF; 32];

        let mut job2 = Job::new("job2".to_string());
        job2.version = 0x20000000;
        job2.ntime = 1706400000;
        job2.nbits = 0x1d00ffff;
        job2.prev_hash = [0x33; 32]; // Different prev_hash
        job2.merkle_root_override = Some([0x44; 32]); // Different merkle root
        job2.target = [0xFF; 32];

        let gpu_job1 = GpuCoordinator::create_gpu_job(&job1);
        let gpu_job2 = GpuCoordinator::create_gpu_job(&job2);

        // Different jobs should produce different midstates
        assert_ne!(gpu_job1.midstate, gpu_job2.midstate);
    }

    #[test]
    fn test_gpu_job_clone() {
        let mut job = Job::new("clone-test".to_string());
        job.version = 0x20000000;
        job.ntime = 1706400000;
        job.nbits = 0x1d00ffff;
        job.prev_hash = [0x77; 32];
        job.merkle_root_override = Some([0x88; 32]);
        job.target = [0xFF; 32];

        let gpu_job = GpuCoordinator::create_gpu_job(&job);
        let cloned = gpu_job.clone();

        assert_eq!(cloned.job_id, gpu_job.job_id);
        assert_eq!(cloned.midstate, gpu_job.midstate);
        assert_eq!(cloned.tail, gpu_job.tail);
        assert_eq!(cloned.target, gpu_job.target);
        assert_eq!(cloned.network_target, gpu_job.network_target);
    }

    #[test]
    fn test_gpu_coordinator_creation_no_gpus() {
        // Test that GpuCoordinator handles no GPUs gracefully
        let stats = Arc::new(Stats::new());
        // Use non-existent device IDs to simulate no available GPUs
        let coordinator = GpuCoordinator::new(&[999, 998, 997], 80, stats);

        // Should have 0 workers since devices don't exist
        assert_eq!(coordinator.worker_count(), 0);
    }

    #[test]
    fn test_gpu_coordinator_start_stop() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[], 80, stats);

        // Initially not running
        assert!(!coordinator.is_running());

        // Start
        coordinator.start();
        assert!(coordinator.is_running());

        // Stop
        coordinator.stop();
        assert!(!coordinator.is_running());

        // Can restart
        coordinator.start();
        assert!(coordinator.is_running());
    }

    #[test]
    fn test_gpu_coordinator_stats_reference() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[], 80, Arc::clone(&stats));

        // Stats should be shared
        stats.add_gpu_hashes(1000);
        assert_eq!(coordinator.stats().get_gpu_hashes(), 1000);
    }

    #[test]
    fn test_gpu_coordinator_devices_empty_when_no_gpus() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[999], 80, stats);

        // Should have empty devices list
        assert!(coordinator.devices().is_empty());
    }

    #[test]
    fn test_gpu_coordinator_initial_hashrate_zero() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[], 80, stats);

        // Hashrate should be 0 initially
        assert_eq!(coordinator.hashrate(), 0.0);
    }

    #[test]
    fn test_gpu_coordinator_clear_job() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[], 80, stats);

        // Should not panic
        coordinator.clear_job();
    }

    #[test]
    fn test_gpu_coordinator_update_job() {
        let stats = Arc::new(Stats::new());
        let coordinator = GpuCoordinator::new(&[], 80, stats);

        let mut job = Job::new("update-test".to_string());
        job.version = 0x20000000;
        job.ntime = 1706400000;
        job.nbits = 0x1d00ffff;
        job.prev_hash = [0xAA; 32];
        job.merkle_root_override = Some([0xBB; 32]);
        job.target = [0xFF; 32];

        // Should not panic even with no workers
        coordinator.update_job(&job);
    }

    #[test]
    fn test_gpu_coordinator_drop_stops_mining() {
        let stats = Arc::new(Stats::new());
        {
            let coordinator = GpuCoordinator::new(&[], 80, Arc::clone(&stats));
            coordinator.start();
            assert!(coordinator.is_running());
            // Coordinator goes out of scope here
        }
        // After drop, mining should be stopped (tested implicitly by no panics)
    }
}
