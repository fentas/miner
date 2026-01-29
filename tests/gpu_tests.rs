//! GPU Mining Integration Tests
//!
//! Tests for GPU mining functionality including:
//! - GpuCoordinator creation and management
//! - GpuJob creation and validation
//! - Target comparison functions
//! - GPU tuning parameters

use btcminer::mining::{GpuCoordinator, GpuTuningParams, Job, Stats};
use std::sync::Arc;

// ============================================================================
// GpuCoordinator Tests
// ============================================================================

#[test]
fn test_gpu_coordinator_lifecycle() {
    let stats = Arc::new(Stats::new());
    let coordinator = GpuCoordinator::new(&[], 80, stats);

    // Initial state
    assert!(!coordinator.is_running());
    assert_eq!(coordinator.hashrate(), 0.0);

    // Start mining
    coordinator.start();
    assert!(coordinator.is_running());

    // Stop mining
    coordinator.stop();
    assert!(!coordinator.is_running());
}

#[test]
fn test_gpu_coordinator_with_invalid_device_ids() {
    let stats = Arc::new(Stats::new());
    // Request non-existent devices
    let coordinator = GpuCoordinator::new(&[100, 200, 300], 80, stats);

    // Should have no workers
    assert_eq!(coordinator.worker_count(), 0);
    assert!(coordinator.devices().is_empty());
}

#[test]
fn test_gpu_coordinator_job_update() {
    let stats = Arc::new(Stats::new());
    let coordinator = GpuCoordinator::new(&[], 80, stats);

    // Create a test job
    let mut job = Job::new("test-job-1".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0xAA; 32];
    job.merkle_root_override = Some([0xBB; 32]);
    job.target = [0xFF; 32];

    // Update job should not panic
    coordinator.update_job(&job);

    // Update with new job
    let mut job2 = Job::new("test-job-2".to_string());
    job2.version = 0x20000000;
    job2.ntime = 1706400001;
    job2.nbits = 0x1d00ffff;
    job2.prev_hash = [0xCC; 32];
    job2.merkle_root_override = Some([0xDD; 32]);
    job2.target = [0xFF; 32];

    coordinator.update_job(&job2);

    // Clear job
    coordinator.clear_job();
}

#[test]
fn test_gpu_coordinator_stats_are_shared() {
    let stats = Arc::new(Stats::new());
    let coordinator = GpuCoordinator::new(&[], 80, Arc::clone(&stats));

    // Add hashes through stats
    stats.add_gpu_hashes(5000);

    // Should be visible through coordinator
    assert_eq!(coordinator.stats().get_gpu_hashes(), 5000);

    // Add more
    stats.add_gpu_hashes(3000);
    assert_eq!(coordinator.stats().get_gpu_hashes(), 8000);
}

#[test]
fn test_gpu_coordinator_intensity_levels() {
    let stats = Arc::new(Stats::new());

    // Test with different intensity levels
    for intensity in [1, 25, 50, 75, 100] {
        let coordinator = GpuCoordinator::new(&[], intensity, Arc::clone(&stats));
        // Should not panic at any intensity
        coordinator.start();
        coordinator.stop();
    }
}

// ============================================================================
// GpuJob Tests
// ============================================================================

#[test]
fn test_gpu_job_from_mining_job() {
    let mut job = Job::new("gpu-job-test".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0x12; 32];
    job.merkle_root_override = Some([0x34; 32]);
    job.target = [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

    let gpu_job = GpuCoordinator::create_gpu_job(&job);

    assert_eq!(gpu_job.job_id, "gpu-job-test");
    assert_eq!(gpu_job.target, job.target);
    assert_eq!(gpu_job.network_target, job.network_target);
}

#[test]
fn test_gpu_job_with_solo_mining_target() {
    let mut job = Job::new("solo-mining-test".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x170c69e6; // Mainnet difficulty
    job.prev_hash = [0x56; 32];
    job.merkle_root_override = Some([0x78; 32]);
    // Pool target (easier)
    job.target = [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                  0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    // Network target (harder)
    job.network_target = Some([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F,
                               0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                               0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                               0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

    let gpu_job = GpuCoordinator::create_gpu_job(&job);

    assert!(gpu_job.network_target.is_some());
    // Network target should be harder (lower values)
    assert!(gpu_job.network_target.unwrap() < gpu_job.target);
}

#[test]
fn test_gpu_job_midstate_format() {
    let mut job = Job::new("midstate-test".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0x9A; 32];
    job.merkle_root_override = Some([0xBC; 32]);
    job.target = [0xFF; 32];

    let gpu_job = GpuCoordinator::create_gpu_job(&job);

    // Midstate should be 8 x u32
    assert_eq!(gpu_job.midstate.len(), 8);

    // All midstate values should be non-zero (SHA-256 initial values modified by header)
    let all_zero = gpu_job.midstate.iter().all(|&v| v == 0);
    assert!(!all_zero, "Midstate should have non-zero values");
}

#[test]
fn test_gpu_job_tail_format() {
    let mut job = Job::new("tail-test".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0xDE; 32];
    job.merkle_root_override = Some([0xF0; 32]);
    job.target = [0xFF; 32];

    let gpu_job = GpuCoordinator::create_gpu_job(&job);

    // Tail should be 16 bytes
    assert_eq!(gpu_job.tail.len(), 16);

    // First 12 bytes come from header[64..76], last 4 are padding for nonce
    // The first 12 bytes should contain ntime (4 bytes) + nbits (4 bytes) + padding
}

#[test]
fn test_gpu_job_consistency() {
    let mut job = Job::new("consistency-test".to_string());
    job.version = 0x20000000;
    job.ntime = 1706400000;
    job.nbits = 0x1d00ffff;
    job.prev_hash = [0x11; 32];
    job.merkle_root_override = Some([0x22; 32]);
    job.target = [0xFF; 32];

    // Create multiple GPU jobs from the same mining job
    let gpu_job1 = GpuCoordinator::create_gpu_job(&job);
    let gpu_job2 = GpuCoordinator::create_gpu_job(&job);
    let gpu_job3 = GpuCoordinator::create_gpu_job(&job);

    // All should be identical
    assert_eq!(gpu_job1.midstate, gpu_job2.midstate);
    assert_eq!(gpu_job2.midstate, gpu_job3.midstate);
    assert_eq!(gpu_job1.tail, gpu_job2.tail);
    assert_eq!(gpu_job2.tail, gpu_job3.tail);
}

// ============================================================================
// Stats GPU Hash Tracking Tests
// ============================================================================

#[test]
fn test_stats_gpu_hash_counting() {
    let stats = Stats::new();

    assert_eq!(stats.get_gpu_hashes(), 0);

    stats.add_gpu_hashes(1_000_000);
    assert_eq!(stats.get_gpu_hashes(), 1_000_000);

    stats.add_gpu_hashes(500_000);
    assert_eq!(stats.get_gpu_hashes(), 1_500_000);
}

#[test]
fn test_stats_snapshot_includes_gpu_hashes() {
    let stats = Stats::new();

    stats.add_hashes(100_000); // CPU hashes
    stats.add_gpu_hashes(200_000); // GPU hashes

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.hashes, 100_000);
    assert_eq!(snapshot.gpu_hashes, 200_000);
}

#[test]
fn test_stats_concurrent_gpu_hash_updates() {
    use std::thread;

    let stats = Arc::new(Stats::new());
    let mut handles = vec![];

    // Spawn multiple threads adding GPU hashes
    for _ in 0..4 {
        let stats_clone = Arc::clone(&stats);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                stats_clone.add_gpu_hashes(100);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 4 threads * 1000 iterations * 100 hashes = 400,000
    assert_eq!(stats.get_gpu_hashes(), 400_000);
}

// ============================================================================
// GpuTuningParams Tests
// ============================================================================

#[test]
fn test_gpu_tuning_params_default() {
    let params = GpuTuningParams::default();

    assert_eq!(params.intensity, 100);
    assert_eq!(params.workgroup_size, 0);
    assert_eq!(params.batch_exp, 25);
}

#[test]
fn test_gpu_tuning_params_custom() {
    let params = GpuTuningParams {
        intensity: 50,
        workgroup_size: 256,
        batch_exp: 24,
    };

    assert_eq!(params.intensity, 50);
    assert_eq!(params.workgroup_size, 256);
    assert_eq!(params.batch_exp, 24);
}

#[test]
fn test_gpu_coordinator_with_tuning() {
    let stats = Arc::new(Stats::new());
    let tuning = GpuTuningParams {
        intensity: 75,
        workgroup_size: 512,
        batch_exp: 26,
    };

    let coordinator = GpuCoordinator::with_tuning(&[], tuning, stats);

    // Should work without panicking
    assert!(!coordinator.is_running());
    coordinator.start();
    assert!(coordinator.is_running());
    coordinator.stop();
}

#[test]
fn test_gpu_coordinator_with_tuning_intensity_extremes() {
    let stats = Arc::new(Stats::new());

    // Test minimum intensity
    let tuning_min = GpuTuningParams {
        intensity: 1,
        workgroup_size: 0,
        batch_exp: 20,
    };
    let coord_min = GpuCoordinator::with_tuning(&[], tuning_min, Arc::clone(&stats));
    coord_min.start();
    coord_min.stop();

    // Test maximum intensity
    let tuning_max = GpuTuningParams {
        intensity: 100,
        workgroup_size: 1024,
        batch_exp: 28,
    };
    let coord_max = GpuCoordinator::with_tuning(&[], tuning_max, Arc::clone(&stats));
    coord_max.start();
    coord_max.stop();
}

#[test]
fn test_gpu_coordinator_with_tuning_batch_exp_clamping() {
    let stats = Arc::new(Stats::new());

    // Test batch_exp below minimum (should clamp to 20)
    let tuning_low = GpuTuningParams {
        intensity: 100,
        workgroup_size: 0,
        batch_exp: 10, // Below minimum
    };
    let coord_low = GpuCoordinator::with_tuning(&[], tuning_low, Arc::clone(&stats));
    coord_low.start();
    coord_low.stop();

    // Test batch_exp above maximum (should clamp to 28)
    let tuning_high = GpuTuningParams {
        intensity: 100,
        workgroup_size: 0,
        batch_exp: 35, // Above maximum
    };
    let coord_high = GpuCoordinator::with_tuning(&[], tuning_high, Arc::clone(&stats));
    coord_high.start();
    coord_high.stop();
}

#[test]
fn test_gpu_tuning_params_clone() {
    let params = GpuTuningParams {
        intensity: 80,
        workgroup_size: 256,
        batch_exp: 25,
    };

    let cloned = params.clone();

    assert_eq!(cloned.intensity, params.intensity);
    assert_eq!(cloned.workgroup_size, params.workgroup_size);
    assert_eq!(cloned.batch_exp, params.batch_exp);
}

#[test]
fn test_gpu_coordinator_device_hashrates_empty() {
    let stats = Arc::new(Stats::new());
    let coordinator = GpuCoordinator::new(&[], 80, stats);

    // With no actual GPUs, device_hashrates should be empty
    let hashrates = coordinator.device_hashrates();
    assert!(hashrates.is_empty() || hashrates.iter().all(|(_, rate)| *rate == 0.0));
}

#[test]
fn test_gpu_coordinator_device_hashrate_not_found() {
    let stats = Arc::new(Stats::new());
    let coordinator = GpuCoordinator::new(&[], 80, stats);

    // Request hashrate for non-existent device
    let rate = coordinator.device_hashrate(9999);
    assert!(rate.is_none());
}
