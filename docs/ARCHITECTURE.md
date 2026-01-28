# Architecture

This document describes the internal architecture of BTC Miner.

## Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                            main.cpp                                  │
│  - CLI argument parsing    - Config loading    - Signal handling     │
└────────┬──────────────┬──────────────┬──────────────┬───────────────┘
         │              │              │              │
         ▼              ▼              ▼              ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│   Stratum   │ │   Miner     │ │ Web Server  │ │    Idle     │
│   Client    │◄│   Core      │ │             │ │   Monitor   │
│ (V1 or V2)  │ │             │ │             │ │             │
└──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └─────────────┘
       │               │               │
       ▼               ▼               ▼
  ┌─────────┐   ┌─────────────────────────────────┐
  │  Pool   │   │         Hashing Engine           │
  │         │   ├─────────────┬─────────────┬─────┤
  └─────────┘   │  CPU (SIMD) │  GPU (CUDA) │ GPU │
                │  SHA-NI     │             │ OCL │
                │  AVX2/512   │             │     │
                └─────────────┴─────────────┴─────┘
```

## Components

### 1. SHA-256 Engine

Header-only implementation with compile-time feature detection.

#### Files

| File | Description |
| ---- | ----------- |
| `sha256.hpp` | Generic + SHA-NI implementation |
| `sha256_avx.hpp` | AVX2/AVX512 multi-hashing |

#### Implementations

| Function | Description | Performance |
| -------- | ----------- | ----------- |
| `transform_shani()` | SHA-NI hardware acceleration | ~800 MH/s |
| `transform_generic()` | Portable C++ implementation | ~20 MH/s |
| `sha256d_midstate_8way()` | AVX2 8-way parallel | ~200-300 MH/s |
| `sha256d_midstate_16way()` | AVX512 16-way parallel | ~400-500 MH/s |

#### Key Functions

```cpp
// Full 80-byte header double-hash
void sha256d_80(const uint8_t header[80], uint8_t hash[32]);

// Pre-compute midstate for first 64 bytes
void compute_midstate(const uint8_t header[80], Midstate& ms);

// Hash with midstate (fast path for mining)
void sha256d_midstate(const Midstate& ms, uint32_t nonce, uint8_t hash[32]);

// AVX2: Process 8 nonces, returns bitmask of valid hashes
uint8_t sha256d_midstate_8way(const Midstate& ms, uint32_t nonce_base,
                               const uint8_t target[32], uint8_t hashes[8][32]);

// AVX512: Process 16 nonces
uint16_t sha256d_midstate_16way(const Midstate& ms, uint32_t nonce_base,
                                 const uint8_t target[32], uint8_t hashes[16][32]);
```

#### SHA-NI Instructions

- `_mm_sha256rnds2_epu32`: 2 rounds of SHA-256 compression
- `_mm_sha256msg1_epu32`: First part of message schedule
- `_mm_sha256msg2_epu32`: Second part of message schedule

#### AVX2/AVX512 Multi-hashing

Process multiple nonces in parallel using SIMD registers:

```text
AVX2 (256-bit registers):
┌────────────────────────────────────────────────────────────┐
│ Lane 0 │ Lane 1 │ Lane 2 │ Lane 3 │ Lane 4 │ Lane 5 │ ... │
│ nonce  │ nonce  │ nonce  │ nonce  │ nonce  │ nonce  │     │
│   +0   │   +1   │   +2   │   +3   │   +4   │   +5   │     │
└────────────────────────────────────────────────────────────┘
                    ↓ Single instruction ↓
            8 SHA-256 rounds computed in parallel
```

### 2. GPU Mining (`src/gpu/`)

CUDA and OpenCL implementations for GPU acceleration.

#### Files

| File | Description |
| ---- | ----------- |
| `gpu.hpp` | Abstract GPU miner interface |
| `gpu.cpp` | GPU manager and coordinator |
| `cuda_miner.cuh` | CUDA miner class |
| `sha256d.cu` | CUDA SHA-256d kernel |
| `opencl_miner.hpp` | OpenCL miner class |
| `sha256d.cl` | OpenCL SHA-256d kernel |

#### Architecture

```text
┌───────────────────────────────────────────────────────┐
│                   GPUCoordinator                       │
│  - Manages multiple GPUs                               │
│  - Distributes nonce ranges                            │
│  - Collects results                                    │
└─────────────────────────┬─────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│  CUDAMiner    │ │  CUDAMiner    │ │ OpenCLMiner   │
│  GPU 0        │ │  GPU 1        │ │  GPU 2        │
└───────┬───────┘ └───────┬───────┘ └───────┬───────┘
        │                 │                 │
        ▼                 ▼                 ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ sha256d.cu    │ │ sha256d.cu    │ │ sha256d.cl    │
│ (kernel)      │ │ (kernel)      │ │ (kernel)      │
└───────────────┘ └───────────────┘ └───────────────┘
```

#### GPU Kernel Design

```cpp
// Each GPU thread processes one nonce
__global__ void sha256d_kernel(
    const uint32_t* midstate,    // Pre-computed SHA-256 state
    const uint8_t* tail,         // Last 16 bytes of header
    const uint8_t* target,       // Difficulty target
    uint32_t nonce_start,        // Starting nonce for this batch
    uint32_t* result_nonces,     // Found nonces
    uint8_t* result_hashes,      // Corresponding hashes
    uint32_t* result_count,      // Number of results
    int max_results
);
```

### 3. Stratum V1 Client (`stratum.hpp/cpp`)

JSON-based Stratum mining protocol.

#### Protocol Messages

| Message | Direction | Purpose |
| ------- | --------- | ------- |
| `mining.subscribe` | Client → Pool | Initialize connection |
| `mining.authorize` | Client → Pool | Authenticate worker |
| `mining.notify` | Pool → Client | New job notification |
| `mining.submit` | Client → Pool | Submit found share |
| `mining.set_difficulty` | Pool → Client | Update share difficulty |

#### Job Structure

```cpp
struct Job {
    std::string job_id;
    std::string prevhash;        // Previous block hash
    std::string coinb1, coinb2;  // Coinbase transaction parts
    std::vector<std::string> merkle_branch;
    std::string version, nbits, ntime;
    bool clean_jobs;

    uint8_t header_template[80]; // Pre-built header
    uint8_t target[32];          // Difficulty target
};
```

### 4. Stratum V2 Client (`stratum_v2.hpp/cpp`)

Binary Stratum V2 protocol with Noise encryption.

#### Features

- Binary message encoding (more efficient than JSON)
- Noise protocol encryption (optional)
- Channel-based job negotiation
- Better bandwidth utilization

#### Message Format

```text
┌──────────────────────────────────────────────────────┐
│ Extension Type (2 bytes) │ Msg Type (1) │ Length (3) │
├──────────────────────────────────────────────────────┤
│                    Payload (variable)                 │
└──────────────────────────────────────────────────────┘
```

#### Message Types

| Type | Value | Description |
| ---- | ----- | ----------- |
| SetupConnection | 0x40 | Initialize connection |
| OpenStandardMiningChannel | 0x00 | Open mining channel |
| NewMiningJob | 0x11 | New job notification |
| SetNewPrevHash | 0x10 | New block detected |
| SubmitSharesStandard | 0x20 | Submit share |
| SetTarget | 0x30 | Update difficulty |

#### ByteBuffer Serialization

```cpp
class ByteBuffer {
    // Little-endian serialization
    void write_u8(uint8_t v);
    void write_u16(uint16_t v);
    void write_u32(uint32_t v);
    void write_u64(uint64_t v);
    void write_string(const std::string& s);  // u8 length + data
    void write_bool(bool v);

    // Corresponding read methods
    uint8_t read_u8();
    // ...
};
```

### 5. Miner Core (`miner.hpp/cpp`)

Multi-threaded mining engine with work distribution.

#### Threading Model

```text
┌──────────────────────────────────────────┐
│              Miner Core                   │
│  - Job distribution                       │
│  - Nonce range allocation                 │
│  - Stats aggregation                      │
└─────────────────┬────────────────────────┘
                  │
     ┌────────────┼────────────┐
     │            │            │
     ▼            ▼            ▼
┌─────────┐ ┌─────────┐ ┌─────────┐
│Worker 0 │ │Worker 1 │ │Worker N │
│Nonce:   │ │Nonce:   │ │Nonce:   │
│0x000000-│ │0x100000-│ │0xN00000-│
│0x0FFFFF │ │0x1FFFFF │ │0xNFFFFF │
└─────────┘ └─────────┘ └─────────┘
```

#### Work Distribution

- Workers request nonce chunks atomically (`NONCE_CHUNK = 0x100000`)
- No lock contention during hashing
- Job changes trigger immediate worker restart
- Random nonce start per job (avoids duplicate work)

#### Statistics

```cpp
struct Stats {
    std::atomic<uint64_t> hashes{0};
    std::atomic<uint64_t> shares_found{0};
    std::atomic<uint64_t> shares_accepted{0};
    std::atomic<uint64_t> shares_rejected{0};
    std::atomic<uint64_t> benchmark_hashes{0};
};
```

### 6. Configuration (`config.hpp`)

Key-value config parser with pool presets and validation.

#### Pool Presets

```cpp
enum class SoloPool {
    Custom,     // User-defined pool
    CKPool,     // solo.ckpool.org (Stratum V1)
    Braiins     // solo.stratum.braiins.com (Stratum V1)
};
```

| Preset | Host | Port | Protocol |
| ------ | ---- | ---- | -------- |
| CKPool | solo.ckpool.org | 3333 | Stratum V1 |
| Braiins | solo.stratum.braiins.com | 3333 | Stratum V1 |

#### Config Struct

```cpp
struct Config {
    // Pool
    SoloPool solo_pool;           // Pool preset
    std::string pool_host;
    uint16_t pool_port;
    std::string payout_address;
    std::string password;
    std::string stratum_version;  // "v1" or "v2"

    // Mining
    unsigned threads;
    bool enable_gpu;

    // Web
    uint16_t web_port;
    std::string web_dir;

    // Idle mining
    unsigned idle_timeout;
    bool idle_mining_enabled;

    // Output
    bool colors;
    std::string log_file;
    bool system_bell;
    unsigned stats_interval;

    // Network
    unsigned reconnect_delay;
    unsigned max_reconnect_delay;
};
```

#### Validation

The config system validates settings and reports errors/warnings:

| Check | Type | Description |
| ----- | ---- | ----------- |
| PayoutAddress | Error | Required, validates Bitcoin address format |
| Stratum version | Error | Must be "v1" or "v2" |
| Pool port | Error | Cannot be 0 |
| CKPool + V2 | Warning | CKPool doesn't support Stratum V2 |
| Braiins + V1 | Warning | Braiins supports V2 for better efficiency |
| High thread count | Warning | Unusually high (>256) |

### 7. Web Server (`webserver.hpp`)

Header-only HTTP/WebSocket server for the dashboard.

#### Features

- Static file serving (HTML, CSS, JS)
- REST API endpoints for stats, settings, and control
- WebSocket for real-time stats broadcast
- No external dependencies (uses raw sockets)

#### API Routes

| Route | Method | Description |
| ----- | ------ | ----------- |
| `/api/stats` | GET | Mining statistics |
| `/api/settings` | GET/POST | Configuration |
| `/api/control/*` | POST | Miner control (start/stop/pause) |
| `/api/benchmark` | GET/POST | Benchmark mode |
| `/api/idle` | GET/POST | Idle mining control |
| `/ws` | WebSocket | Real-time updates |

### 8. Idle Monitor (`idle_monitor.hpp`)

Cross-platform idle detection for screensaver-style mining.

#### Environment Detection

```cpp
enum class Environment {
    NativeLinux,  // X11 via xprintidle
    WSL2,         // Windows via PowerShell
    Headless      // No display (disabled)
};
```

#### Idle Detection Methods

| Environment | Method | API |
| ----------- | ------ | --- |
| Native Linux | xprintidle / xssstate | X11 screen saver extension |
| WSL2 | PowerShell | Windows GetLastInputInfo |

## Data Flow

### Mining Loop

```text
1. Pool sends mining.notify
   │
   ▼
2. Stratum client parses job
   │
   ▼
3. Build block header template
   │
   ├─ version (4 bytes)
   ├─ prevhash (32 bytes)
   ├─ merkle_root (32 bytes, computed)
   ├─ ntime (4 bytes)
   ├─ nbits (4 bytes)
   └─ nonce (4 bytes, placeholder)
   │
   ▼
4. Compute midstate for bytes 0-63
   │
   ▼
5. Workers iterate nonces:
   │
   │  for nonce in range:
   │      hash = sha256d_midstate(midstate, nonce)
   │      if hash < target:
   │          submit(nonce)
   │
   ▼
6. On new job, goto step 2
```

### Block Header Structure

```text
Offset  Size  Field
──────  ────  ─────────────────
0       4     Version (little-endian)
4       32    Previous block hash
36      32    Merkle root
68      4     Timestamp (ntime)
72      4     Difficulty target (nbits)
76      4     Nonce ← Miner iterates this
──────  ────
Total   80 bytes
```

## Performance Optimizations

### 1. Midstate Pre-computation

Since bytes 0-63 of the header don't change, we compute SHA-256 state once:

```cpp
// First 64 bytes → midstate (computed once per job)
// Last 16 bytes + nonce → final hash (computed per nonce)
```

### 2. SHA-NI Intrinsics

Direct CPU instruction usage eliminates:

- Memory lookups for K constants
- Branch mispredictions
- Most of the 64-round loop overhead

### 3. AVX2/AVX512 Multi-hashing

Process 8 (AVX2) or 16 (AVX512) nonces simultaneously:

```cpp
// AVX2: 8 independent SHA-256 computations per instruction
__m256i state[8];  // 8 lanes, each lane is one SHA-256 computation
```

### 4. Nonce Chunking

Workers grab large nonce ranges atomically to minimize synchronization:

```cpp
uint32_t nonce_start = nonce_base_.fetch_add(NONCE_CHUNK);
// Mine 1M nonces without touching atomics
```

### 5. Target Pre-computation

nbits to 256-bit target conversion done once per job, not per hash.

### 6. GPU Parallelism

Thousands of GPU threads each compute one nonce:

```text
Grid: 256 blocks × 256 threads = 65,536 parallel hashes
```

## Testing

### Test Structure

```text
tests/
├── test_framework.hpp    # Minimal test framework
├── test_main.cpp         # Test runner
├── test_sha256.cpp       # SHA-256 basic tests
├── test_sha256_avx.cpp   # AVX2/AVX512 tests
├── test_stratum.cpp      # Stratum V1 tests
├── test_stratum_v2.cpp   # Stratum V2 tests
├── test_gpu.cpp          # GPU interface tests
└── test_config.cpp       # Config validation tests
```

### Coverage

```bash
# Generate coverage report
./scripts/run_coverage.sh
# Open build_coverage/coverage_html/index.html
```

## References

- [Bitcoin Block Header](https://developer.bitcoin.org/reference/block_chain.html#block-headers)
- [Stratum V1 Protocol](https://braiins.com/stratum-v1/docs)
- [Stratum V2 Specification](https://stratumprotocol.org/)
- [Intel SHA Extensions](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sha-extensions.html)
- [SHA-256 Specification (FIPS 180-4)](https://csrc.nist.gov/publications/detail/fips/180/4/final)
- [CUDA Programming Guide](https://docs.nvidia.com/cuda/cuda-c-programming-guide/)
- [OpenCL Specification](https://www.khronos.org/opencl/)
