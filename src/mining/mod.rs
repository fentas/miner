mod worker;
mod job;
mod hasher;
pub mod sha256_simd;
pub mod gpu_worker;

pub use worker::{Worker, WorkerHandle};
pub use job::{Job, difficulty_to_target};
pub use hasher::Hasher;
pub use sha256_simd::{SimdLevel, Midstate};
pub use gpu_worker::{GpuCoordinator, GpuTuningParams};

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

/// Mining statistics
#[derive(Debug, Default)]
pub struct Stats {
    pub hashes: AtomicU64,
    pub gpu_hashes: AtomicU64,
    pub shares_accepted: AtomicU64,
    pub shares_rejected: AtomicU64,
    pub blocks_found: AtomicU64,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_hashes(&self, count: u64) {
        self.hashes.fetch_add(count, Ordering::Relaxed);
    }

    pub fn add_gpu_hashes(&self, count: u64) {
        self.gpu_hashes.fetch_add(count, Ordering::Relaxed);
    }

    pub fn get_gpu_hashes(&self) -> u64 {
        self.gpu_hashes.load(Ordering::Relaxed)
    }

    pub fn accept_share(&self) {
        self.shares_accepted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn reject_share(&self) {
        self.shares_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn found_block(&self) {
        self.blocks_found.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_hashes(&self) -> u64 {
        self.hashes.load(Ordering::Relaxed)
    }

    pub fn reset_hashes(&self) -> u64 {
        self.hashes.swap(0, Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            hashes: self.hashes.load(Ordering::Relaxed),
            gpu_hashes: self.gpu_hashes.load(Ordering::Relaxed),
            shares_accepted: self.shares_accepted.load(Ordering::Relaxed),
            shares_rejected: self.shares_rejected.load(Ordering::Relaxed),
            blocks_found: self.blocks_found.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of mining statistics
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StatsSnapshot {
    pub hashes: u64,
    pub gpu_hashes: u64,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
}

/// Mining coordinator manages worker threads
pub struct Coordinator {
    workers: Vec<WorkerHandle>,
    stats: Arc<Stats>,
    running: Arc<AtomicBool>,
    job_tx: watch::Sender<Option<Job>>,
    share_rx: mpsc::Receiver<Share>,
}

/// A found share
#[derive(Debug, Clone)]
pub struct Share {
    pub job_id: String,
    pub nonce: u32,
    pub hash: [u8; 32],
    pub worker_id: usize,
}

impl Coordinator {
    /// Create a new mining coordinator
    pub fn new(thread_count: usize) -> Self {
        let stats = Arc::new(Stats::new());
        let running = Arc::new(AtomicBool::new(false));
        let (job_tx, _job_rx) = watch::channel(None);
        let (share_tx, share_rx) = mpsc::channel(100);

        let mut workers = Vec::with_capacity(thread_count);
        for id in 0..thread_count {
            let handle = Worker::spawn(
                id,
                Arc::clone(&stats),
                Arc::clone(&running),
                job_tx.subscribe(),
                share_tx.clone(),
            );
            workers.push(handle);
        }

        Self {
            workers,
            stats,
            running,
            job_tx,
            share_rx,
        }
    }

    /// Start mining
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop mining
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if mining is active
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Update the current job
    pub fn update_job(&self, job: Job) {
        let _ = self.job_tx.send(Some(job));
    }

    /// Clear the current job
    pub fn clear_job(&self) {
        let _ = self.job_tx.send(None);
    }

    /// Receive a found share (non-blocking)
    pub fn try_recv_share(&mut self) -> Option<Share> {
        self.share_rx.try_recv().ok()
    }

    /// Receive a found share (blocking)
    pub async fn recv_share(&mut self) -> Option<Share> {
        self.share_rx.recv().await
    }

    /// Get statistics
    pub fn stats(&self) -> &Arc<Stats> {
        &self.stats
    }

    /// Get worker count
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for Coordinator {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_new() {
        let stats = Stats::new();
        assert_eq!(stats.get_hashes(), 0);
        assert_eq!(stats.shares_accepted.load(Ordering::Relaxed), 0);
        assert_eq!(stats.shares_rejected.load(Ordering::Relaxed), 0);
        assert_eq!(stats.blocks_found.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_stats_default() {
        let stats = Stats::default();
        assert_eq!(stats.get_hashes(), 0);
    }

    #[test]
    fn test_stats_add_hashes() {
        let stats = Stats::new();
        stats.add_hashes(100);
        assert_eq!(stats.get_hashes(), 100);

        stats.add_hashes(50);
        assert_eq!(stats.get_hashes(), 150);
    }

    #[test]
    fn test_stats_reset_hashes() {
        let stats = Stats::new();
        stats.add_hashes(200);
        assert_eq!(stats.get_hashes(), 200);

        let old_value = stats.reset_hashes();
        assert_eq!(old_value, 200);
        assert_eq!(stats.get_hashes(), 0);
    }

    #[test]
    fn test_stats_accept_share() {
        let stats = Stats::new();
        stats.accept_share();
        stats.accept_share();
        assert_eq!(stats.shares_accepted.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_stats_reject_share() {
        let stats = Stats::new();
        stats.reject_share();
        assert_eq!(stats.shares_rejected.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_stats_found_block() {
        let stats = Stats::new();
        stats.found_block();
        stats.found_block();
        stats.found_block();
        assert_eq!(stats.blocks_found.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_stats_snapshot() {
        let stats = Stats::new();
        stats.add_hashes(1000);
        stats.accept_share();
        stats.accept_share();
        stats.reject_share();
        stats.found_block();

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.hashes, 1000);
        assert_eq!(snapshot.shares_accepted, 2);
        assert_eq!(snapshot.shares_rejected, 1);
        assert_eq!(snapshot.blocks_found, 1);
    }

    #[test]
    fn test_stats_snapshot_default() {
        let snapshot = StatsSnapshot::default();
        assert_eq!(snapshot.hashes, 0);
        assert_eq!(snapshot.shares_accepted, 0);
        assert_eq!(snapshot.shares_rejected, 0);
        assert_eq!(snapshot.blocks_found, 0);
    }

    #[test]
    fn test_stats_snapshot_clone() {
        let snapshot = StatsSnapshot {
            hashes: 100,
            gpu_hashes: 0,
            shares_accepted: 5,
            shares_rejected: 1,
            blocks_found: 0,
        };
        let cloned = snapshot.clone();
        assert_eq!(cloned.hashes, 100);
        assert_eq!(cloned.shares_accepted, 5);
    }

    #[test]
    fn test_share_struct() {
        let share = Share {
            job_id: "test_job_123".to_string(),
            nonce: 0xDEADBEEF,
            hash: [0xAB; 32],
            worker_id: 7,
        };

        assert_eq!(share.job_id, "test_job_123");
        assert_eq!(share.nonce, 0xDEADBEEF);
        assert_eq!(share.hash, [0xAB; 32]);
        assert_eq!(share.worker_id, 7);
    }

    #[test]
    fn test_share_clone() {
        let share = Share {
            job_id: "job".to_string(),
            nonce: 42,
            hash: [0; 32],
            worker_id: 0,
        };

        let cloned = share.clone();
        assert_eq!(cloned.job_id, share.job_id);
        assert_eq!(cloned.nonce, share.nonce);
    }

    #[test]
    fn test_coordinator_new() {
        let coord = Coordinator::new(4);
        assert_eq!(coord.worker_count(), 4);
        assert!(!coord.is_running());
    }

    #[test]
    fn test_coordinator_start_stop() {
        let coord = Coordinator::new(2);
        assert!(!coord.is_running());

        coord.start();
        assert!(coord.is_running());

        coord.stop();
        assert!(!coord.is_running());
    }

    #[test]
    fn test_coordinator_stats() {
        let coord = Coordinator::new(1);
        let stats = coord.stats();
        stats.add_hashes(500);
        assert_eq!(stats.get_hashes(), 500);
    }

    #[test]
    fn test_coordinator_try_recv_share_empty() {
        let mut coord = Coordinator::new(1);
        // No shares have been found, should return None
        assert!(coord.try_recv_share().is_none());
    }

    #[test]
    fn test_coordinator_update_job() {
        let coord = Coordinator::new(1);
        let job = Job::new("test_job".to_string());
        // Should not panic
        coord.update_job(job);
    }

    #[test]
    fn test_coordinator_clear_job() {
        let coord = Coordinator::new(1);
        // Should not panic
        coord.clear_job();
    }
}
