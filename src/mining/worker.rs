use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use super::{Job, Share, Stats, Hasher};

/// Notification mechanism for waking workers
///
/// Uses a condvar to allow workers to block instead of busy-waiting.
/// Workers wait on the condvar with a timeout, waking immediately when
/// notified or after the timeout to check for state changes.
pub struct Notifier {
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    /// Wake all waiting workers
    pub fn notify_all(&self) {
        // Lock to ensure memory visibility, then notify
        let _guard = self.mutex.lock().unwrap();
        self.condvar.notify_all();
    }

    /// Wait for a notification or timeout
    ///
    /// Returns immediately if notified, otherwise waits up to `timeout`.
    pub fn wait_timeout(&self, timeout: Duration) {
        let guard = self.mutex.lock().unwrap();
        let _ = self.condvar.wait_timeout(guard, timeout);
    }
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

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
    notifier: Arc<Notifier>,
    job_rx: watch::Receiver<Option<Arc<Job>>>,
    share_tx: mpsc::Sender<Share>,
    hasher: Hasher,
    extranonce1: Vec<u8>,
    extranonce2_size: usize,
    batch_size: u32,
}

impl Worker {
    /// Spawn a new worker thread
    ///
    /// Returns `Ok(WorkerHandle)` on success, or an error if thread creation fails.
    pub fn spawn(
        id: usize,
        stats: Arc<Stats>,
        running: Arc<AtomicBool>,
        notifier: Arc<Notifier>,
        job_rx: watch::Receiver<Option<Arc<Job>>>,
        share_tx: mpsc::Sender<Share>,
        batch_size: u32,
    ) -> Result<WorkerHandle, std::io::Error> {
        let handle = thread::Builder::new()
            .name(format!("miner-{}", id))
            .spawn(move || {
                let mut worker = Worker {
                    id,
                    stats,
                    running,
                    notifier,
                    job_rx,
                    share_tx,
                    hasher: Hasher::new(),
                    extranonce1: vec![0u8; 4],
                    extranonce2_size: 4,
                    batch_size,
                };
                worker.run();
            })?;

        Ok(WorkerHandle { id, handle })
    }

    /// Set extranonce parameters
    pub fn set_extranonce(&mut self, extranonce1: Vec<u8>, extranonce2_size: usize) {
        self.extranonce1 = extranonce1;
        self.extranonce2_size = extranonce2_size;
    }

    /// Main mining loop
    fn run(&mut self) {
        let batch_size = self.batch_size;

        loop {
            // Wait for running flag to be set (coordinator.start() called)
            // Uses condvar instead of busy-wait to reduce idle CPU usage
            while !self.running.load(Ordering::Acquire) {
                self.notifier.wait_timeout(Duration::from_millis(100));
            }

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
                if !self.running.load(Ordering::Acquire) {
                    break;
                }

                if self.job_rx.has_changed().unwrap_or(true) {
                    break;
                }

                // Hash a batch of nonces
                let result = self.hasher.hash_nonce_range(
                    &header_prefix,
                    nonce,
                    batch_size,
                    &job.target,
                );

                // Update stats
                self.stats.add_hashes(batch_size as u64);

                // Check for share
                if let Some((found_nonce, hash)) = result {
                    // Note: job.id is only cloned when share found (rare event in real mining)
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
                nonce = nonce.wrapping_add(batch_size);
                if nonce < nonce_start {
                    // Wrapped around, need new extranonce2
                    break;
                }
            }
        }
    }

    /// Wait for a valid job
    fn wait_for_job(&mut self) -> Option<Arc<Job>> {
        loop {
            // Check if we should stop
            if !self.running.load(Ordering::Acquire) {
                return None;
            }

            // Check current job using borrow_and_update to clear the "changed" flag
            // This ensures has_changed() only returns true for actual new jobs
            // Arc::clone is cheap - just increments reference count
            let job = self.job_rx.borrow_and_update().clone();
            if job.is_some() {
                return job;
            }

            // Wait on condvar instead of busy-wait spinning
            // Wakes immediately when notified or after timeout to check state
            self.notifier.wait_timeout(Duration::from_millis(100));
        }
    }

    /// Generate extranonce2 for this worker
    fn generate_extranonce2(&self) -> Vec<u8> {
        let mut extranonce2 = vec![0u8; self.extranonce2_size];

        // Use worker id and high-resolution timestamp for uniqueness
        let id_bytes = (self.id as u32).to_le_bytes();
        // Use subsec_nanos for higher resolution to avoid collisions
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let time_bytes = nanos.to_le_bytes();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_handle_id() {
        // We can't easily test spawn without a full runtime setup,
        // but we can test the WorkerHandle struct
        let handle = std::thread::spawn(|| {});
        let worker_handle = WorkerHandle {
            id: 42,
            handle,
        };
        assert_eq!(worker_handle.id(), 42);
        // Join the thread to clean up
        assert!(worker_handle.join().is_ok());
    }

    #[test]
    fn test_generate_extranonce2_length() {
        // Create a minimal worker for testing extranonce2 generation
        let (_share_tx, _share_rx) = mpsc::channel(10);
        let (_job_tx, job_rx) = watch::channel(None::<Arc<Job>>);
        let stats = Arc::new(Stats::new());
        let running = Arc::new(AtomicBool::new(false));
        let notifier = Arc::new(Notifier::new());

        let worker = Worker {
            id: 5,
            stats,
            running,
            notifier,
            job_rx,
            share_tx: _share_tx,
            hasher: Hasher::new(),
            extranonce1: vec![0u8; 4],
            extranonce2_size: 8,
            batch_size: 65536,
        };

        let extranonce2 = worker.generate_extranonce2();
        assert_eq!(extranonce2.len(), 8);

        // First 4 bytes should contain worker id (5)
        let id_from_extranonce = u32::from_le_bytes([
            extranonce2[0], extranonce2[1], extranonce2[2], extranonce2[3]
        ]);
        assert_eq!(id_from_extranonce, 5);
    }

    #[test]
    fn test_generate_extranonce2_small_size() {
        let (_share_tx, _share_rx) = mpsc::channel(10);
        let (_job_tx, job_rx) = watch::channel(None::<Arc<Job>>);
        let stats = Arc::new(Stats::new());
        let running = Arc::new(AtomicBool::new(false));
        let notifier = Arc::new(Notifier::new());

        let worker = Worker {
            id: 3,
            stats,
            running,
            notifier,
            job_rx,
            share_tx: _share_tx,
            hasher: Hasher::new(),
            extranonce1: vec![0u8; 4],
            extranonce2_size: 2,
            batch_size: 65536,
        };

        let extranonce2 = worker.generate_extranonce2();
        assert_eq!(extranonce2.len(), 2);
    }

    #[test]
    fn test_set_extranonce() {
        let (_share_tx, _share_rx) = mpsc::channel(10);
        let (_job_tx, job_rx) = watch::channel(None::<Arc<Job>>);
        let stats = Arc::new(Stats::new());
        let running = Arc::new(AtomicBool::new(false));
        let notifier = Arc::new(Notifier::new());

        let mut worker = Worker {
            id: 0,
            stats,
            running,
            notifier,
            job_rx,
            share_tx: _share_tx,
            hasher: Hasher::new(),
            extranonce1: vec![],
            extranonce2_size: 4,
            batch_size: 65536,
        };

        let new_extranonce1 = vec![0xAA, 0xBB, 0xCC, 0xDD];
        worker.set_extranonce(new_extranonce1.clone(), 6);

        assert_eq!(worker.extranonce1, new_extranonce1);
        assert_eq!(worker.extranonce2_size, 6);
    }

    #[test]
    fn test_worker_different_ids_different_extranonce() {
        let (_share_tx1, _) = mpsc::channel(10);
        let (_share_tx2, _) = mpsc::channel(10);
        let (_, job_rx1) = watch::channel(None::<Arc<Job>>);
        let (_, job_rx2) = watch::channel(None::<Arc<Job>>);
        let stats = Arc::new(Stats::new());
        let running = Arc::new(AtomicBool::new(false));
        let notifier = Arc::new(Notifier::new());

        let worker1 = Worker {
            id: 0,
            stats: stats.clone(),
            running: running.clone(),
            notifier: notifier.clone(),
            job_rx: job_rx1,
            share_tx: _share_tx1,
            hasher: Hasher::new(),
            extranonce1: vec![0u8; 4],
            extranonce2_size: 4,
            batch_size: 65536,
        };

        let worker2 = Worker {
            id: 1,
            stats,
            running,
            notifier,
            job_rx: job_rx2,
            share_tx: _share_tx2,
            hasher: Hasher::new(),
            extranonce1: vec![0u8; 4],
            extranonce2_size: 4,
            batch_size: 65536,
        };

        let en1 = worker1.generate_extranonce2();
        let en2 = worker2.generate_extranonce2();

        // Different workers should have different extranonce2
        assert_ne!(en1, en2);
    }

    #[test]
    fn test_notifier_new() {
        let notifier = Notifier::new();
        // Should be able to create without panic
        notifier.notify_all();
    }

    #[test]
    fn test_notifier_wait_timeout() {
        let notifier = Arc::new(Notifier::new());
        let notifier_clone = Arc::clone(&notifier);

        // Test that wait_timeout returns after timeout
        let start = std::time::Instant::now();
        notifier.wait_timeout(Duration::from_millis(10));
        let elapsed = start.elapsed();

        // Should have waited approximately 10ms (allow some tolerance)
        assert!(elapsed.as_millis() >= 8);
        assert!(elapsed.as_millis() < 100);

        // Test that notify wakes up a waiting thread early
        let handle = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            notifier_clone.wait_timeout(Duration::from_secs(10));
            start.elapsed()
        });

        // Give the thread time to start waiting
        std::thread::sleep(Duration::from_millis(10));
        notifier.notify_all();

        let elapsed = handle.join().unwrap();
        // Should have been woken up much earlier than 10 seconds
        assert!(elapsed.as_millis() < 500);
    }
}
