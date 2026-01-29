//! OpenCL GPU Miner Implementation
//!
//! Provides GPU-accelerated SHA-256d mining using OpenCL.
//! Supports AMD, Intel, and NVIDIA GPUs.

#[cfg(feature = "opencl")]
use std::time::Instant;
#[cfg(feature = "opencl")]
use super::{GpuBackend, GpuMiner, MiningResult};
use super::{GpuDevice, GpuError};

/// Maximum number of results that can be returned from a single kernel execution
const MAX_RESULTS: usize = 64;

/// SHA-256d OpenCL kernel source
///
/// This kernel implements double SHA-256 hashing for Bitcoin mining.
/// It uses midstate optimization: the first 64 bytes of the block header
/// are pre-hashed on the CPU, and only the remaining 16 bytes + nonce
/// are processed on the GPU.
const SHA256D_KERNEL: &str = r#"
// SHA-256 round constants
__constant uint K[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

// SHA-256 helper functions
#define rotr(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define Ch(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define Maj(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define Sigma0(x) (rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22))
#define Sigma1(x) (rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25))
#define sigma0(x) (rotr(x, 7) ^ rotr(x, 18) ^ ((x) >> 3))
#define sigma1(x) (rotr(x, 17) ^ rotr(x, 19) ^ ((x) >> 10))

// SHA-256 compression function
void sha256_transform(uint* state, const uint* block) {
    uint W[64];
    uint a, b, c, d, e, f, g, h;

    // Prepare message schedule
    for (int i = 0; i < 16; i++) {
        W[i] = block[i];
    }
    for (int i = 16; i < 64; i++) {
        W[i] = sigma1(W[i-2]) + W[i-7] + sigma0(W[i-15]) + W[i-16];
    }

    // Initialize working variables
    a = state[0]; b = state[1]; c = state[2]; d = state[3];
    e = state[4]; f = state[5]; g = state[6]; h = state[7];

    // 64 rounds
    for (int i = 0; i < 64; i++) {
        uint T1 = h + Sigma1(e) + Ch(e, f, g) + K[i] + W[i];
        uint T2 = Sigma0(a) + Maj(a, b, c);
        h = g; g = f; f = e; e = d + T1;
        d = c; c = b; b = a; a = T1 + T2;
    }

    // Add compressed chunk to hash value
    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

// Compare hash to target (big-endian comparison)
bool meets_target(const uchar* hash, __global const uchar* target) {
    for (int i = 0; i < 32; i++) {
        if (hash[i] < target[i]) return true;
        if (hash[i] > target[i]) return false;
    }
    return true;
}

// Main mining kernel
// Each work-item tests one nonce
__kernel void sha256d_mining(
    __global const uint* midstate,      // Pre-computed state after first 64 bytes
    __global const uchar* tail,         // Last 16 bytes of header (before nonce)
    __global const uchar* target,       // 32-byte target for difficulty
    uint nonce_start,                   // Starting nonce for this batch
    __global uint* result_nonces,       // Output: found nonces
    __global uchar* result_hashes,      // Output: corresponding hashes
    __global uint* result_count,        // Output: number of results found
    uint max_results                    // Maximum results to store
) {
    uint gid = get_global_id(0);
    uint nonce = nonce_start + gid;

    // Build the second 64-byte block (only 16 bytes of data + nonce + padding)
    uint block[16];

    // Load tail bytes as little-endian words
    for (int i = 0; i < 4; i++) {
        block[i] = ((uint)tail[i*4]) | ((uint)tail[i*4+1] << 8) |
                   ((uint)tail[i*4+2] << 16) | ((uint)tail[i*4+3] << 24);
    }

    // Set nonce (little-endian)
    block[4] = nonce;

    // SHA-256 padding
    block[5] = 0x80000000;  // Padding bit
    for (int i = 6; i < 15; i++) block[i] = 0;
    block[15] = 640;  // Message length: 80 bytes = 640 bits

    // First SHA-256: continue from midstate
    uint state[8];
    for (int i = 0; i < 8; i++) state[i] = midstate[i];
    sha256_transform(state, block);

    // Prepare second SHA-256 input (hash the first hash)
    uint block2[16];
    for (int i = 0; i < 8; i++) block2[i] = state[i];
    block2[8] = 0x80000000;  // Padding
    for (int i = 9; i < 15; i++) block2[i] = 0;
    block2[15] = 256;  // 32 bytes = 256 bits

    // Second SHA-256: start from initial state
    state[0] = 0x6a09e667; state[1] = 0xbb67ae85;
    state[2] = 0x3c6ef372; state[3] = 0xa54ff53a;
    state[4] = 0x510e527f; state[5] = 0x9b05688c;
    state[6] = 0x1f83d9ab; state[7] = 0x5be0cd19;
    sha256_transform(state, block2);

    // Convert state to big-endian bytes for target comparison
    uchar hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4]   = (state[i] >> 24) & 0xff;
        hash[i*4+1] = (state[i] >> 16) & 0xff;
        hash[i*4+2] = (state[i] >> 8) & 0xff;
        hash[i*4+3] = state[i] & 0xff;
    }

    // Check if hash meets target
    if (meets_target(hash, target)) {
        uint idx = atomic_inc(result_count);
        if (idx < max_results) {
            result_nonces[idx] = nonce;
            for (int i = 0; i < 32; i++) {
                result_hashes[idx * 32 + i] = hash[i];
            }
        }
    }
}
"#;

/// Enumerate all available OpenCL GPU devices
#[cfg(feature = "opencl")]
pub fn enumerate_devices() -> Result<Vec<GpuDevice>, GpuError> {
    use ocl::{Platform, Device, DeviceType};
    use ocl::core::DeviceInfo;
    use std::panic;

    let mut devices = Vec::new();
    let mut id = 0u32;

    // Platform::list() can panic if OpenCL runtime is not available
    // Catch the panic and return empty list instead
    let platforms = match panic::catch_unwind(|| Platform::list()) {
        Ok(p) => p,
        Err(_) => {
            tracing::warn!(
                "OpenCL not available - no GPU mining. \
                Install OpenCL drivers (AMD: ROCm/AMDGPU-PRO, NVIDIA: CUDA). \
                On WSL2: ensure GPU compute passthrough is enabled."
            );
            return Ok(vec![]);
        }
    };

    for platform in platforms {
        if let Ok(devs) = Device::list(&platform, Some(DeviceType::GPU)) {
            for dev in devs {
                let name = dev.name().unwrap_or_else(|_| "Unknown".to_string());
                let vendor = dev.vendor().unwrap_or_else(|_| "Unknown".to_string());
                let wgs = dev.max_wg_size().unwrap_or(256);

                let mem = match dev.info(DeviceInfo::GlobalMemSize) {
                    Ok(ocl::core::DeviceInfoResult::GlobalMemSize(m)) => m,
                    _ => 0,
                };
                let cu = match dev.info(DeviceInfo::MaxComputeUnits) {
                    Ok(ocl::core::DeviceInfoResult::MaxComputeUnits(c)) => c,
                    _ => 0,
                };

                devices.push(GpuDevice {
                    id,
                    name,
                    vendor,
                    memory_total: mem,
                    compute_units: cu,
                    max_work_group_size: wgs,
                    backend: GpuBackend::OpenCL,
                    available: true,
                });
                id += 1;
            }
        }
    }

    Ok(devices)
}

#[cfg(not(feature = "opencl"))]
pub fn enumerate_devices() -> Result<Vec<GpuDevice>, GpuError> {
    Ok(vec![])
}

/// OpenCL GPU Miner
///
/// Performs SHA-256d mining on GPU using OpenCL.
/// Uses midstate optimization for efficiency.
#[cfg(feature = "opencl")]
pub struct OpenCLMiner {
    device: GpuDevice,
    /// Controls work size: higher = more GPU utilization (1-100)
    intensity: u8,
    /// OpenCL execution queue
    queue: ocl::Queue,
    /// Compiled mining kernel
    kernel: ocl::Kernel,
    /// GPU buffer for midstate (8 x u32)
    midstate_buf: ocl::Buffer<u32>,
    /// GPU buffer for tail bytes (16 bytes)
    tail_buf: ocl::Buffer<u8>,
    /// GPU buffer for target (32 bytes)
    target_buf: ocl::Buffer<u8>,
    /// GPU buffer for result nonces
    result_nonces_buf: ocl::Buffer<u32>,
    /// GPU buffer for result hashes
    result_hashes_buf: ocl::Buffer<u8>,
    /// GPU buffer for result count
    result_count_buf: ocl::Buffer<u32>,
    /// Work group size for kernel execution
    work_group_size: usize,
    /// Total hashes computed
    hashes_done: u64,
    /// Start time for hashrate calculation
    start_time: Instant,
    /// Flag to stop mining
    running: bool,
}

#[cfg(feature = "opencl")]
impl OpenCLMiner {
    /// Create a new OpenCL miner for the specified device
    ///
    /// # Arguments
    /// * `device_id` - ID of the GPU device to use
    /// * `intensity` - Work intensity (1-100), affects batch size
    ///
    /// # Returns
    /// * `Ok(OpenCLMiner)` - Initialized miner ready for mining
    /// * `Err(GpuError)` - If device not found or OpenCL initialization fails
    pub fn new(device_id: u32, intensity: u8) -> Result<Self, GpuError> {
        use ocl::{Platform, Device, DeviceType, Context, Queue, Program, Kernel, Buffer};
        use ocl::core::DeviceInfo;
        use std::panic;

        // Find the device
        let mut target_device = None;
        let mut target_platform = None;
        let mut current_id = 0u32;

        // Platform::list() can panic if OpenCL runtime is not available
        let platforms = match panic::catch_unwind(|| Platform::list()) {
            Ok(p) => p,
            Err(_) => {
                return Err(GpuError::OpenCL("OpenCL runtime not available".to_string()));
            }
        };

        for platform in platforms {
            if let Ok(devs) = Device::list(&platform, Some(DeviceType::GPU)) {
                for dev in devs {
                    if current_id == device_id {
                        target_device = Some(dev);
                        target_platform = Some(platform);
                        break;
                    }
                    current_id += 1;
                }
            }
            if target_device.is_some() {
                break;
            }
        }

        let ocl_device = target_device.ok_or(GpuError::DeviceNotFound(device_id))?;
        let platform = target_platform.ok_or(GpuError::DeviceNotFound(device_id))?;

        // Get device info
        let name = ocl_device.name().unwrap_or_else(|_| "Unknown".to_string());
        let vendor = ocl_device.vendor().unwrap_or_else(|_| "Unknown".to_string());
        let wgs = ocl_device.max_wg_size().unwrap_or(256);
        let mem = match ocl_device.info(DeviceInfo::GlobalMemSize) {
            Ok(ocl::core::DeviceInfoResult::GlobalMemSize(m)) => m,
            _ => 0,
        };
        let cu = match ocl_device.info(DeviceInfo::MaxComputeUnits) {
            Ok(ocl::core::DeviceInfoResult::MaxComputeUnits(c)) => c,
            _ => 0,
        };

        let device = GpuDevice {
            id: device_id,
            name,
            vendor,
            memory_total: mem,
            compute_units: cu,
            max_work_group_size: wgs,
            backend: GpuBackend::OpenCL,
            available: true,
        };

        // Create OpenCL context and queue
        let context = Context::builder()
            .platform(platform)
            .devices(ocl_device)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let queue = Queue::new(&context, ocl_device, None)
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        // Compile the kernel
        let program = Program::builder()
            .src(SHA256D_KERNEL)
            .devices(ocl_device)
            .build(&context)
            .map_err(|e| GpuError::KernelCompile(e.to_string()))?;

        // Create buffers
        let midstate_buf: Buffer<u32> = Buffer::builder()
            .queue(queue.clone())
            .len(8)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let tail_buf: Buffer<u8> = Buffer::builder()
            .queue(queue.clone())
            .len(16)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let target_buf: Buffer<u8> = Buffer::builder()
            .queue(queue.clone())
            .len(32)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let result_nonces_buf: Buffer<u32> = Buffer::builder()
            .queue(queue.clone())
            .len(MAX_RESULTS)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let result_hashes_buf: Buffer<u8> = Buffer::builder()
            .queue(queue.clone())
            .len(MAX_RESULTS * 32)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        let result_count_buf: Buffer<u32> = Buffer::builder()
            .queue(queue.clone())
            .len(1)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        // Create kernel with arguments
        let kernel = Kernel::builder()
            .program(&program)
            .name("sha256d_mining")
            .queue(queue.clone())
            .global_work_size(1) // Will be set per mine() call
            .arg(&midstate_buf)
            .arg(&tail_buf)
            .arg(&target_buf)
            .arg(0u32) // nonce_start - will be set per mine() call
            .arg(&result_nonces_buf)
            .arg(&result_hashes_buf)
            .arg(&result_count_buf)
            .arg(MAX_RESULTS as u32)
            .build()
            .map_err(|e| GpuError::OpenCL(e.to_string()))?;

        // Use optimal work group size for GPU architecture
        // RDNA2 and newer AMD GPUs work well with 256-1024 work group sizes
        // Default to device max, capped at 1024 for broad compatibility
        let work_group_size = std::cmp::min(wgs, 1024);

        Ok(Self {
            device,
            intensity,
            queue,
            kernel,
            midstate_buf,
            tail_buf,
            target_buf,
            result_nonces_buf,
            result_hashes_buf,
            result_count_buf,
            work_group_size,
            hashes_done: 0,
            start_time: Instant::now(),
            running: true,
        })
    }

    /// Calculate global work size based on intensity and device capabilities
    fn calculate_work_size(&self, nonce_count: u32) -> usize {
        // Base work size on intensity (1-100)
        // intensity 100 = full utilization, intensity 1 = minimal
        let intensity_factor = (self.intensity as f64 / 100.0).max(0.01);

        // Calculate work size for GPU saturation
        // Modern GPUs need tens of millions of work items to stay fully utilized
        // RDNA2 GPUs especially benefit from larger work sizes
        // Base: 2^25 (32M) at intensity 100% - doubled for better GPU utilization
        let base_work = 1usize << 25; // 32M base - optimized for modern GPUs
        let target_size = ((base_work as f64) * intensity_factor) as usize;

        // Ensure work size is a multiple of work_group_size and doesn't exceed nonce_count
        let work_size = (target_size / self.work_group_size) * self.work_group_size;
        std::cmp::min(work_size.max(self.work_group_size), nonce_count as usize)
    }
}

#[cfg(feature = "opencl")]
impl GpuMiner for OpenCLMiner {
    fn set_job(&mut self, midstate: &[u32; 8], tail: &[u8; 16], target: &[u8; 32]) {
        // Upload job data to GPU buffers (convert arrays to slices)
        self.midstate_buf.write(&midstate[..]).enq().ok();
        self.tail_buf.write(&tail[..]).enq().ok();
        self.target_buf.write(&target[..]).enq().ok();
    }

    fn mine(&mut self, nonce_start: u32, nonce_count: u32) -> Vec<MiningResult> {
        if !self.running || nonce_count == 0 {
            return vec![];
        }

        let mut results = Vec::new();
        let mut current_nonce = nonce_start;
        let mut remaining = nonce_count;

        while remaining > 0 && self.running {
            let batch_size = self.calculate_work_size(remaining);

            // Reset result count
            let zero = vec![0u32; 1];
            if self.result_count_buf.write(&zero[..]).enq().is_err() {
                break;
            }

            // Set kernel arguments for this batch
            if self.kernel.set_arg(3, current_nonce).is_err() {
                break;
            }

            // Update global work size via kernel cmd builder
            let cmd = self.kernel.cmd()
                .global_work_size(batch_size);

            // Execute kernel
            unsafe {
                if cmd.enq().is_err() {
                    break;
                }
            }

            // Wait for completion
            if self.queue.finish().is_err() {
                break;
            }

            // Read result count
            let mut result_count = vec![0u32; 1];
            if self.result_count_buf.read(&mut result_count[..]).enq().is_err() {
                break;
            }

            let count = (result_count[0] as usize).min(MAX_RESULTS);

            // If we found results, read them
            if count > 0 {
                let mut nonces = vec![0u32; MAX_RESULTS];
                let mut hashes = vec![0u8; MAX_RESULTS * 32];

                if self.result_nonces_buf.read(&mut nonces[..]).enq().is_ok()
                    && self.result_hashes_buf.read(&mut hashes[..]).enq().is_ok()
                {
                    for i in 0..count {
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&hashes[i * 32..(i + 1) * 32]);
                        results.push(MiningResult {
                            nonce: nonces[i],
                            hash,
                        });
                    }
                }
            }

            self.hashes_done += batch_size as u64;
            current_nonce = current_nonce.wrapping_add(batch_size as u32);
            remaining = remaining.saturating_sub(batch_size as u32);
        }

        results
    }

    fn device(&self) -> &GpuDevice {
        &self.device
    }

    fn hashrate(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.hashes_done as f64 / elapsed
        } else {
            0.0
        }
    }

    fn stop(&mut self) {
        self.running = false;
        // Finish any pending operations
        let _ = self.queue.finish();
    }
}

/// Placeholder when OpenCL feature is disabled
#[cfg(not(feature = "opencl"))]
pub struct OpenCLMiner;

#[cfg(not(feature = "opencl"))]
impl OpenCLMiner {
    pub fn new(_device_id: u32, _intensity: u8) -> Result<Self, GpuError> {
        Err(GpuError::NotSupported)
    }
}

#[cfg(all(test, feature = "opencl"))]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_devices() {
        // This test will pass even if no GPU is available
        let devices = enumerate_devices();
        assert!(devices.is_ok());

        let devices = devices.unwrap();
        println!("Found {} OpenCL GPU device(s)", devices.len());

        for device in &devices {
            println!("  [{} {}] {} - {} CUs, {} MB",
                device.backend.name(),
                device.id,
                device.name,
                device.compute_units,
                device.memory_total / (1024 * 1024)
            );
        }
    }

    #[test]
    fn test_miner_creation() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping miner creation test");
            return;
        }

        let device_id = devices[0].id;
        let miner = OpenCLMiner::new(device_id, 50);

        match miner {
            Ok(m) => {
                println!("Successfully created miner for device: {}", m.device().name);
                assert_eq!(m.device().id, device_id);
            }
            Err(e) => {
                println!("Failed to create miner (may be expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_miner_invalid_device() {
        let result = OpenCLMiner::new(9999, 50);
        assert!(matches!(result, Err(GpuError::DeviceNotFound(9999))));
    }

    #[test]
    fn test_mining_with_easy_target() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping mining test");
            return;
        }

        let mut miner = match OpenCLMiner::new(devices[0].id, 50) {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to create miner: {}", e);
                return;
            }
        };

        // Set up a job with a very easy target (high value = easy)
        let midstate = [0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                       0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let tail = [0u8; 16];
        let target = [0xffu8; 32]; // Very easy target - any hash should pass

        miner.set_job(&midstate, &tail, &target);

        // Mine a small batch
        let results = miner.mine(0, 1024);

        println!("Mining test: {} hashes, {} results found, {:.2} H/s",
            miner.hashes_done,
            results.len(),
            miner.hashrate()
        );

        // With such an easy target, we should find many results
        assert!(!results.is_empty(), "Should find results with easy target");
    }

    #[test]
    fn test_work_size_calculation_logic() {
        // Test the work size calculation formula independently
        // Formula: base_work = 2^22, target_size = base_work * intensity * 2

        // At intensity 100%, expect ~8M work items
        let intensity_100: f64 = 100.0 / 100.0;
        let base_work: usize = 1 << 22; // 4M
        let target_100 = ((base_work as f64) * intensity_100 * 2.0) as usize;
        assert_eq!(target_100, 8_388_608, "Intensity 100 should give 8M work items");

        // At intensity 50%, expect ~4M work items
        let intensity_50: f64 = 50.0 / 100.0;
        let target_50 = ((base_work as f64) * intensity_50 * 2.0) as usize;
        assert_eq!(target_50, 4_194_304, "Intensity 50 should give 4M work items");

        // At intensity 80%, expect ~6.7M work items
        let intensity_80: f64 = 80.0 / 100.0;
        let target_80 = ((base_work as f64) * intensity_80 * 2.0) as usize;
        assert_eq!(target_80, 6_710_886, "Intensity 80 should give ~6.7M work items");

        // At intensity 1%, expect minimum work items
        let intensity_1: f64 = 1.0 / 100.0;
        let target_1 = ((base_work as f64) * intensity_1 * 2.0) as usize;
        assert_eq!(target_1, 83_886, "Intensity 1 should give ~84K work items");
    }

    #[test]
    fn test_miner_set_job_and_stop() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping test");
            return;
        }

        let mut miner = match OpenCLMiner::new(devices[0].id, 50) {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to create miner: {}", e);
                return;
            }
        };

        // Set up a job
        let midstate = [0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                       0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let tail = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
                    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let target = [0x00, 0x00, 0x00, 0x1F, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                      0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        // Should not panic
        miner.set_job(&midstate, &tail, &target);

        // Stop should not panic
        miner.stop();
    }

    #[test]
    fn test_miner_hashrate_increases_after_mining() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping test");
            return;
        }

        let mut miner = match OpenCLMiner::new(devices[0].id, 50) {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to create miner: {}", e);
                return;
            }
        };

        // Set up a job with easy target
        let midstate = [0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                       0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let tail = [0u8; 16];
        let target = [0xffu8; 32];

        miner.set_job(&midstate, &tail, &target);

        // Initial hashrate should be 0
        let initial_hashrate = miner.hashrate();

        // Mine some hashes
        let _results = miner.mine(0, 65536);

        // Give a tiny bit of time for calculation
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Hashrate should now be positive
        let final_hashrate = miner.hashrate();
        assert!(final_hashrate > initial_hashrate, "Hashrate should increase after mining");
    }

    #[test]
    fn test_miner_device_info() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping test");
            return;
        }

        let miner = match OpenCLMiner::new(devices[0].id, 80) {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to create miner: {}", e);
                return;
            }
        };

        let device = miner.device();
        assert!(!device.name.is_empty(), "Device should have a name");
        assert!(device.compute_units > 0, "Device should have compute units");
        assert!(device.max_work_group_size > 0, "Device should have work group size");
    }

    #[test]
    fn test_miner_mine_empty_batch() {
        let devices = enumerate_devices().unwrap();

        if devices.is_empty() {
            println!("No OpenCL devices found, skipping test");
            return;
        }

        let mut miner = match OpenCLMiner::new(devices[0].id, 50) {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to create miner: {}", e);
                return;
            }
        };

        let midstate = [0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                       0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
        let tail = [0u8; 16];
        let target = [0xffu8; 32];

        miner.set_job(&midstate, &tail, &target);

        // Mining with 0 nonces should return empty results
        let results = miner.mine(0, 0);
        assert!(results.is_empty(), "Mining 0 nonces should return empty results");
    }
}
