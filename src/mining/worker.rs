use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tokio::sync::{mpsc, watch};

use super::{Job, Share, Stats, Hasher};

/// Handle to a mining worker thread
pub struct WorkerHandle {
    id: usize,
    handle: JoinHandle<()>,
}

impl WorkerHandle {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn join(self) -> thread::Result<()> {
        self.handle.join()
    }
}

/// Mining worker that runs in its own thread
pub struct Worker {
    id: usize,
    stats: Arc<Stats>,
    running: Arc<AtomicBool>,
    job_rx: watch::Receiver<Option<Job>>,
    share_tx: mpsc::Sender<Share>,
    hasher: Hasher,
    extranonce1: Vec<u8>,
    extranonce2_size: usize,
}

impl Worker {
    /// Spawn a new worker thread
    pub fn spawn(
        id: usize,
        stats: Arc<Stats>,
        running: Arc<AtomicBool>,
        job_rx: watch::Receiver<Option<Job>>,
        share_tx: mpsc::Sender<Share>,
    ) -> WorkerHandle {
        let handle = thread::Builder::new()
            .name(format!("miner-{}", id))
            .spawn(move || {
                let mut worker = Worker {
                    id,
                    stats,
                    running,
                    job_rx,
                    share_tx,
                    hasher: Hasher::new(),
                    extranonce1: vec![0u8; 4],
                    extranonce2_size: 4,
                };
                worker.run();
            })
            .expect("Failed to spawn worker thread");

        WorkerHandle { id, handle }
    }

    /// Set extranonce parameters
    pub fn set_extranonce(&mut self, extranonce1: Vec<u8>, extranonce2_size: usize) {
        self.extranonce1 = extranonce1;
        self.extranonce2_size = extranonce2_size;
    }

    /// Main mining loop
    fn run(&mut self) {
        const BATCH_SIZE: u32 = 65536; // Hash batch before checking for new job

        while self.running.load(Ordering::Relaxed) {
            // Wait for a job
            let job = match self.wait_for_job() {
                Some(job) => job,
                None => continue,
            };

            // Generate extranonce2 based on worker id
            let extranonce2 = self.generate_extranonce2();

            // Build header prefix (everything except nonce)
            let header = job.build_header(&self.extranonce1, &extranonce2, 0);
            let mut header_prefix = [0u8; 76];
            header_prefix.copy_from_slice(&header[..76]);

            // Mine nonce range assigned to this worker
            let nonce_start = (self.id as u32) * (u32::MAX / 16);
            let mut nonce = nonce_start;

            loop {
                // Check if still running and job hasn't changed
                if !self.running.load(Ordering::Relaxed) {
                    break;
                }

                if self.job_rx.has_changed().unwrap_or(true) {
                    break;
                }

                // Hash a batch of nonces
                let result = self.hasher.hash_nonce_range(
                    &header_prefix,
                    nonce,
                    BATCH_SIZE,
                    &job.target,
                );

                // Update stats
                self.stats.add_hashes(BATCH_SIZE as u64);

                // Check for share
                if let Some((found_nonce, hash)) = result {
                    let share = Share {
                        job_id: job.id.clone(),
                        nonce: found_nonce,
                        hash,
                        worker_id: self.id,
                    };

                    // Check if it's a block (meets network target)
                    if job.meets_network_target(&hash) {
                        self.stats.found_block();
                    }

                    // Send share (non-blocking)
                    let _ = self.share_tx.try_send(share);
                }

                // Advance nonce
                nonce = nonce.wrapping_add(BATCH_SIZE);
                if nonce < nonce_start {
                    // Wrapped around, need new extranonce2
                    break;
                }
            }
        }
    }

    /// Wait for a valid job
    fn wait_for_job(&mut self) -> Option<Job> {
        loop {
            // Check if we should stop
            if !self.running.load(Ordering::Relaxed) {
                return None;
            }

            // Check current job
            let job = self.job_rx.borrow().clone();
            if job.is_some() {
                return job;
            }

            // Wait for job update (with timeout to check running flag)
            let _ = self.job_rx.changed();
        }
    }

    /// Generate extranonce2 for this worker
    fn generate_extranonce2(&self) -> Vec<u8> {
        let mut extranonce2 = vec![0u8; self.extranonce2_size];

        // Use worker id and timestamp for uniqueness
        let id_bytes = (self.id as u32).to_le_bytes();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let time_bytes = timestamp.to_le_bytes();

        // Fill extranonce2
        for (i, byte) in extranonce2.iter_mut().enumerate() {
            if i < 4 {
                *byte = id_bytes.get(i).copied().unwrap_or(0);
            } else if i < 8 {
                *byte = time_bytes.get(i - 4).copied().unwrap_or(0);
            }
        }

        extranonce2
    }
}
