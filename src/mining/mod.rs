mod worker;
mod job;
mod hasher;
pub mod sha256_simd;

pub use worker::{Worker, WorkerHandle};
pub use job::{Job, difficulty_to_target};
pub use hasher::Hasher;
pub use sha256_simd::SimdLevel;

use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

/// Mining statistics
#[derive(Debug, Default)]
pub struct Stats {
    pub hashes: AtomicU64,
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
