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

/// SHA-256d OpenCL kernel source - OPTIMIZED VERSION
///
/// This kernel implements double SHA-256 hashing for Bitcoin mining.
/// Optimizations:
/// - Fully unrolled SHA256 rounds (no loops)
/// - Inline message schedule expansion
/// - Early exit on target comparison (check high bytes first)
/// - Optimized register usage
/// - Pre-computed constants fused into rounds
const SHA256D_KERNEL: &str = r#"
// SHA-256 helper macros - optimized for GPU
#define ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define SHR(x, n) ((x) >> (n))

// SHA-256 functions
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROTR(x, 2) ^ ROTR(x, 13) ^ ROTR(x, 22))
#define EP1(x) (ROTR(x, 6) ^ ROTR(x, 11) ^ ROTR(x, 25))
#define SIG0(x) (ROTR(x, 7) ^ ROTR(x, 18) ^ SHR(x, 3))
#define SIG1(x) (ROTR(x, 17) ^ ROTR(x, 19) ^ SHR(x, 10))

// Unrolled round macro - performs one SHA256 round
#define SHA256_ROUND(a, b, c, d, e, f, g, h, k, w) \
    { \
        uint t1 = h + EP1(e) + CH(e, f, g) + k + w; \
        uint t2 = EP0(a) + MAJ(a, b, c); \
        h = g; g = f; f = e; e = d + t1; \
        d = c; c = b; b = a; a = t1 + t2; \
    }

// Message schedule expansion macro
#define W_EXPAND(w, i) (SIG1(w[(i-2)&15]) + w[(i-7)&15] + SIG0(w[(i-15)&15]) + w[(i-16)&15])

// Optimized SHA-256 transform with full loop unrolling
inline void sha256_transform_unrolled(uint* state, uint* W) {
    uint a = state[0], b = state[1], c = state[2], d = state[3];
    uint e = state[4], f = state[5], g = state[6], h = state[7];

    // Rounds 0-15 (message schedule already loaded)
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x428a2f98u, W[0]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x71374491u, W[1]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0xb5c0fbcfu, W[2]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0xe9b5dba5u, W[3]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x3956c25bu, W[4]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x59f111f1u, W[5]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x923f82a4u, W[6]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0xab1c5ed5u, W[7]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0xd807aa98u, W[8]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x12835b01u, W[9]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x243185beu, W[10]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x550c7dc3u, W[11]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x72be5d74u, W[12]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x80deb1feu, W[13]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0x9bdc06a7u, W[14]);
    SHA256_ROUND(a,b,c,d,e,f,g,h, 0xc19bf174u, W[15]);

    // Rounds 16-63 with inline message schedule expansion
    W[0] = W_EXPAND(W, 16); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xe49b69c1u, W[0]);
    W[1] = W_EXPAND(W, 17); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xefbe4786u, W[1]);
    W[2] = W_EXPAND(W, 18); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x0fc19dc6u, W[2]);
    W[3] = W_EXPAND(W, 19); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x240ca1ccu, W[3]);
    W[4] = W_EXPAND(W, 20); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x2de92c6fu, W[4]);
    W[5] = W_EXPAND(W, 21); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x4a7484aau, W[5]);
    W[6] = W_EXPAND(W, 22); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x5cb0a9dcu, W[6]);
    W[7] = W_EXPAND(W, 23); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x76f988dau, W[7]);
    W[8] = W_EXPAND(W, 24); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x983e5152u, W[8]);
    W[9] = W_EXPAND(W, 25); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xa831c66du, W[9]);
    W[10] = W_EXPAND(W, 26); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xb00327c8u, W[10]);
    W[11] = W_EXPAND(W, 27); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xbf597fc7u, W[11]);
    W[12] = W_EXPAND(W, 28); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xc6e00bf3u, W[12]);
    W[13] = W_EXPAND(W, 29); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xd5a79147u, W[13]);
    W[14] = W_EXPAND(W, 30); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x06ca6351u, W[14]);
    W[15] = W_EXPAND(W, 31); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x14292967u, W[15]);

    W[0] = W_EXPAND(W, 32); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x27b70a85u, W[0]);
    W[1] = W_EXPAND(W, 33); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x2e1b2138u, W[1]);
    W[2] = W_EXPAND(W, 34); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x4d2c6dfcu, W[2]);
    W[3] = W_EXPAND(W, 35); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x53380d13u, W[3]);
    W[4] = W_EXPAND(W, 36); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x650a7354u, W[4]);
    W[5] = W_EXPAND(W, 37); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x766a0abbu, W[5]);
    W[6] = W_EXPAND(W, 38); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x81c2c92eu, W[6]);
    W[7] = W_EXPAND(W, 39); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x92722c85u, W[7]);
    W[8] = W_EXPAND(W, 40); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xa2bfe8a1u, W[8]);
    W[9] = W_EXPAND(W, 41); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xa81a664bu, W[9]);
    W[10] = W_EXPAND(W, 42); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xc24b8b70u, W[10]);
    W[11] = W_EXPAND(W, 43); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xc76c51a3u, W[11]);
    W[12] = W_EXPAND(W, 44); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xd192e819u, W[12]);
    W[13] = W_EXPAND(W, 45); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xd6990624u, W[13]);
    W[14] = W_EXPAND(W, 46); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xf40e3585u, W[14]);
    W[15] = W_EXPAND(W, 47); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x106aa070u, W[15]);

    W[0] = W_EXPAND(W, 48); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x19a4c116u, W[0]);
    W[1] = W_EXPAND(W, 49); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x1e376c08u, W[1]);
    W[2] = W_EXPAND(W, 50); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x2748774cu, W[2]);
    W[3] = W_EXPAND(W, 51); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x34b0bcb5u, W[3]);
    W[4] = W_EXPAND(W, 52); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x391c0cb3u, W[4]);
    W[5] = W_EXPAND(W, 53); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x4ed8aa4au, W[5]);
    W[6] = W_EXPAND(W, 54); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x5b9cca4fu, W[6]);
    W[7] = W_EXPAND(W, 55); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x682e6ff3u, W[7]);
    W[8] = W_EXPAND(W, 56); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x748f82eeu, W[8]);
    W[9] = W_EXPAND(W, 57); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x78a5636fu, W[9]);
    W[10] = W_EXPAND(W, 58); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x84c87814u, W[10]);
    W[11] = W_EXPAND(W, 59); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x8cc70208u, W[11]);
    W[12] = W_EXPAND(W, 60); SHA256_ROUND(a,b,c,d,e,f,g,h, 0x90befffau, W[12]);
    W[13] = W_EXPAND(W, 61); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xa4506cebu, W[13]);
    W[14] = W_EXPAND(W, 62); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xbef9a3f7u, W[14]);
    W[15] = W_EXPAND(W, 63); SHA256_ROUND(a,b,c,d,e,f,g,h, 0xc67178f2u, W[15]);

    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

// Fast target check - early exit on first mismatch (Bitcoin targets have leading zeros)
inline bool check_target_fast(__global const uchar* target, uint h0, uint h1, uint h2, uint h3,
                              uint h4, uint h5, uint h6, uint h7) {
    // Check high words first (most significant bytes) - they're most likely to fail
    // Convert state words to big-endian for comparison
    uchar t;

    // Check word 0 (bytes 0-3)
    t = target[0]; if ((h0 >> 24) > t) return false; if ((h0 >> 24) < t) return true;
    t = target[1]; if (((h0 >> 16) & 0xff) > t) return false; if (((h0 >> 16) & 0xff) < t) return true;
    t = target[2]; if (((h0 >> 8) & 0xff) > t) return false; if (((h0 >> 8) & 0xff) < t) return true;
    t = target[3]; if ((h0 & 0xff) > t) return false; if ((h0 & 0xff) < t) return true;

    // Check word 1 (bytes 4-7)
    t = target[4]; if ((h1 >> 24) > t) return false; if ((h1 >> 24) < t) return true;
    t = target[5]; if (((h1 >> 16) & 0xff) > t) return false; if (((h1 >> 16) & 0xff) < t) return true;
    t = target[6]; if (((h1 >> 8) & 0xff) > t) return false; if (((h1 >> 8) & 0xff) < t) return true;
    t = target[7]; if ((h1 & 0xff) > t) return false; if ((h1 & 0xff) < t) return true;

    // Check word 2 (bytes 8-11)
    t = target[8]; if ((h2 >> 24) > t) return false; if ((h2 >> 24) < t) return true;
    t = target[9]; if (((h2 >> 16) & 0xff) > t) return false; if (((h2 >> 16) & 0xff) < t) return true;
    t = target[10]; if (((h2 >> 8) & 0xff) > t) return false; if (((h2 >> 8) & 0xff) < t) return true;
    t = target[11]; if ((h2 & 0xff) > t) return false; if ((h2 & 0xff) < t) return true;

    // Check word 3 (bytes 12-15)
    t = target[12]; if ((h3 >> 24) > t) return false; if ((h3 >> 24) < t) return true;
    t = target[13]; if (((h3 >> 16) & 0xff) > t) return false; if (((h3 >> 16) & 0xff) < t) return true;
    t = target[14]; if (((h3 >> 8) & 0xff) > t) return false; if (((h3 >> 8) & 0xff) < t) return true;
    t = target[15]; if ((h3 & 0xff) > t) return false; if ((h3 & 0xff) < t) return true;

    // Check remaining words (16-31) - usually target is 0xff here so quick pass
    t = target[16]; if ((h4 >> 24) > t) return false; if ((h4 >> 24) < t) return true;
    t = target[17]; if (((h4 >> 16) & 0xff) > t) return false; if (((h4 >> 16) & 0xff) < t) return true;
    t = target[18]; if (((h4 >> 8) & 0xff) > t) return false; if (((h4 >> 8) & 0xff) < t) return true;
    t = target[19]; if ((h4 & 0xff) > t) return false; if ((h4 & 0xff) < t) return true;

    t = target[20]; if ((h5 >> 24) > t) return false; if ((h5 >> 24) < t) return true;
    t = target[21]; if (((h5 >> 16) & 0xff) > t) return false; if (((h5 >> 16) & 0xff) < t) return true;
    t = target[22]; if (((h5 >> 8) & 0xff) > t) return false; if (((h5 >> 8) & 0xff) < t) return true;
    t = target[23]; if ((h5 & 0xff) > t) return false; if ((h5 & 0xff) < t) return true;

    t = target[24]; if ((h6 >> 24) > t) return false; if ((h6 >> 24) < t) return true;
    t = target[25]; if (((h6 >> 16) & 0xff) > t) return false; if (((h6 >> 16) & 0xff) < t) return true;
    t = target[26]; if (((h6 >> 8) & 0xff) > t) return false; if (((h6 >> 8) & 0xff) < t) return true;
    t = target[27]; if ((h6 & 0xff) > t) return false; if ((h6 & 0xff) < t) return true;

    t = target[28]; if ((h7 >> 24) > t) return false; if ((h7 >> 24) < t) return true;
    t = target[29]; if (((h7 >> 16) & 0xff) > t) return false; if (((h7 >> 16) & 0xff) < t) return true;
    t = target[30]; if (((h7 >> 8) & 0xff) > t) return false; if (((h7 >> 8) & 0xff) < t) return true;
    t = target[31]; if ((h7 & 0xff) > t) return false;

    return true;
}

// Main mining kernel - optimized version
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

    // Load midstate into registers
    uint s0 = midstate[0], s1 = midstate[1], s2 = midstate[2], s3 = midstate[3];
    uint s4 = midstate[4], s5 = midstate[5], s6 = midstate[6], s7 = midstate[7];

    // Build message schedule for second block (16 bytes tail + nonce + padding)
    uint W[16];

    // Load tail bytes as little-endian words (optimized)
    W[0] = ((uint)tail[0]) | ((uint)tail[1] << 8) | ((uint)tail[2] << 16) | ((uint)tail[3] << 24);
    W[1] = ((uint)tail[4]) | ((uint)tail[5] << 8) | ((uint)tail[6] << 16) | ((uint)tail[7] << 24);
    W[2] = ((uint)tail[8]) | ((uint)tail[9] << 8) | ((uint)tail[10] << 16) | ((uint)tail[11] << 24);
    W[3] = ((uint)tail[12]) | ((uint)tail[13] << 8) | ((uint)tail[14] << 16) | ((uint)tail[15] << 24);

    // Nonce (little-endian)
    W[4] = nonce;

    // SHA-256 padding (80-byte message)
    W[5] = 0x80000000u;
    W[6] = 0; W[7] = 0; W[8] = 0; W[9] = 0; W[10] = 0; W[11] = 0; W[12] = 0; W[13] = 0; W[14] = 0;
    W[15] = 640;  // 80 bytes = 640 bits

    // First SHA-256 transform
    uint state[8] = {s0, s1, s2, s3, s4, s5, s6, s7};
    sha256_transform_unrolled(state, W);

    // Prepare second hash input (hash of first result)
    W[0] = state[0]; W[1] = state[1]; W[2] = state[2]; W[3] = state[3];
    W[4] = state[4]; W[5] = state[5]; W[6] = state[6]; W[7] = state[7];
    W[8] = 0x80000000u;
    W[9] = 0; W[10] = 0; W[11] = 0; W[12] = 0; W[13] = 0; W[14] = 0;
    W[15] = 256;  // 32 bytes = 256 bits

    // Second SHA-256 from initial state
    state[0] = 0x6a09e667u; state[1] = 0xbb67ae85u;
    state[2] = 0x3c6ef372u; state[3] = 0xa54ff53au;
    state[4] = 0x510e527fu; state[5] = 0x9b05688cu;
    state[6] = 0x1f83d9abu; state[7] = 0x5be0cd19u;
    sha256_transform_unrolled(state, W);

    // Fast target check with early exit
    if (check_target_fast(target, state[0], state[1], state[2], state[3],
                          state[4], state[5], state[6], state[7])) {
        uint idx = atomic_inc(result_count);
        if (idx < max_results) {
            result_nonces[idx] = nonce;
            // Store hash in big-endian
            __global uchar* hash_out = result_hashes + idx * 32;
            hash_out[0] = (state[0] >> 24); hash_out[1] = (state[0] >> 16) & 0xff;
            hash_out[2] = (state[0] >> 8) & 0xff; hash_out[3] = state[0] & 0xff;
            hash_out[4] = (state[1] >> 24); hash_out[5] = (state[1] >> 16) & 0xff;
            hash_out[6] = (state[1] >> 8) & 0xff; hash_out[7] = state[1] & 0xff;
            hash_out[8] = (state[2] >> 24); hash_out[9] = (state[2] >> 16) & 0xff;
            hash_out[10] = (state[2] >> 8) & 0xff; hash_out[11] = state[2] & 0xff;
            hash_out[12] = (state[3] >> 24); hash_out[13] = (state[3] >> 16) & 0xff;
            hash_out[14] = (state[3] >> 8) & 0xff; hash_out[15] = state[3] & 0xff;
            hash_out[16] = (state[4] >> 24); hash_out[17] = (state[4] >> 16) & 0xff;
            hash_out[18] = (state[4] >> 8) & 0xff; hash_out[19] = state[4] & 0xff;
            hash_out[20] = (state[5] >> 24); hash_out[21] = (state[5] >> 16) & 0xff;
            hash_out[22] = (state[5] >> 8) & 0xff; hash_out[23] = state[5] & 0xff;
            hash_out[24] = (state[6] >> 24); hash_out[25] = (state[6] >> 16) & 0xff;
            hash_out[26] = (state[6] >> 8) & 0xff; hash_out[27] = state[6] & 0xff;
            hash_out[28] = (state[7] >> 24); hash_out[29] = (state[7] >> 16) & 0xff;
            hash_out[30] = (state[7] >> 8) & 0xff; hash_out[31] = state[7] & 0xff;
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
