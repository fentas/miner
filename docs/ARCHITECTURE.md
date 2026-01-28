# Architecture

This document describes the internal architecture of BTC Miner (Rust edition).

## Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                               main.rs                                        │
│  - CLI argument parsing  - Config loading  - SIMD detection  - TUI/Logging  │
└────────┬──────────┬──────────┬──────────┬──────────┬──────────┬─────────────┘
         │          │          │          │          │          │
         ▼          ▼          ▼          ▼          ▼          ▼
┌─────────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│   Stratum   │ │ Mining │ │  TUI   │ │Bitcoin │ │  GPU   │ │  Idle  │
│   Client    │◄│Coordin-│ │(ratatui│ │  RPC   │ │Manager │ │Monitor │
│  (V1/V2)    │ │  ator  │ │+Web)   │ │        │ │        │ │        │
└──────┬──────┘ └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘
       │            │          │          │          │          │
       ▼            ▼          ▼          ▼          ▼          ▼
  ┌─────────┐ ┌─────────────────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
  │  Pool   │ │   Worker Threads    │ │WebSocket│ │Bitcoin  │ │X11/WSL2 │
  │         │ │ (CPU + GPU hashing) │ │ Clients │ │  Core   │ │ Detect  │
  └─────────┘ └─────────────────────┘ └─────────┘ └─────────┘ └─────────┘
```

## Project Structure

```text
src/
├── main.rs              # Entry point, colored output, SIMD detection
├── config.rs            # TOML configuration parsing
├── mining/              # Mining core
│   ├── mod.rs           # Coordinator, stats
│   ├── worker.rs        # Mining worker threads
│   ├── job.rs           # Mining job structure
│   ├── hasher.rs        # SHA-256d implementation
│   └── sha256_simd.rs   # SIMD optimizations (SHA-NI, AVX2, AVX512)
├── gpu/                 # GPU mining support
│   ├── mod.rs           # GPU types and errors
│   ├── manager.rs       # Device management and orchestration
│   └── opencl.rs        # OpenCL implementation (kernel + host)
├── stratum/             # Pool protocols
│   ├── mod.rs           # Common types
│   ├── v1.rs            # Stratum V1 (JSON-RPC)
│   └── v2.rs            # Stratum V2 (binary)
├── bitcoin/             # Bitcoin Core integration
│   ├── mod.rs           # Types
│   ├── rpc.rs           # JSON-RPC client
│   └── manager.rs       # Bitcoind lifecycle management
├── idle/                # Idle mining support
│   └── mod.rs           # X11/WSL2 idle detection
├── tui/                 # Terminal UI (ratatui)
│   ├── mod.rs           # TUI runner and event loop
│   ├── state.rs         # Shared state (thread-safe)
│   └── widgets.rs       # UI components (hashrate, logs, etc.)
├── web/                 # Web interface
│   └── mod.rs           # REST API, WebSocket, static files
└── stats/               # Statistics
    └── mod.rs           # Hashrate calculation, formatting

web/                     # Web dashboard frontend
├── index.html           # Dashboard HTML structure
├── css/
│   └── styles.css       # Dashboard styles
└── js/
    ├── app.js           # Main application logic
    ├── chart.js         # Hashrate chart (Chart.js)
    ├── websocket.js     # Real-time WebSocket updates
    ├── idle.js          # Idle monitoring UI
    └── notifications.js # Web Notifications API
```

## Components

### 1. Configuration (`config.rs`)

TOML-based configuration with SRI-compatible structure.

```toml
[pool]
preset = "ckpool"  # ckpool, braiins, ocean, none, custom
payout_address = "bc1q..."

[mining]
enable_cpu = true
threads = 0        # 0 = auto-detect
enable_gpu = false
gpu_devices = []   # Empty = all GPUs
gpu_intensity = 80

[bitcoind]
enabled = false
managed = false
prune = 10000
dbcache = 0            # 0 = auto-calculate based on RAM
assumeutxo = false     # Fast sync via UTXO snapshot

[sv2]
enabled = false
proxy_host = "127.0.0.1"
proxy_port = 34255

[idle]
enabled = false
timeout = 900          # 15 minutes
pause_on_active = true

[web]
port = 8080

[logging]
colors = true
stats_interval = 10
system_bell = true
hud = true             # Enable TUI dashboard
format = "text"        # text or json
```

#### Pool Presets

| Preset | Pool | Protocol |
|--------|------|----------|
| `ckpool` | solo.ckpool.org:3333 | Stratum V1 |
| `braiins` | solo.stratum.braiins.com:3333 | Stratum V1 |
| `ocean` | mine.ocean.xyz:3334 | Stratum V1 |
| `none` | True solo (requires Bitcoin Core) | Direct |
| `custom` | User-defined | V1 or V2 |

### 2. Mining Coordinator (`mining/mod.rs`)

Manages CPU and GPU workers, job distribution, and share collection.

```rust
pub struct Coordinator {
    workers: Vec<WorkerHandle>,
    gpu_manager: Option<GpuManager>,
    stats: Arc<Stats>,
    running: Arc<AtomicBool>,
    job_tx: watch::Sender<Option<Job>>,
    share_rx: mpsc::Receiver<Share>,
}
```

#### Statistics

```rust
pub struct Stats {
    pub hashes: AtomicU64,
    pub shares_accepted: AtomicU64,
    pub shares_rejected: AtomicU64,
    pub blocks_found: AtomicU64,
}
```

### 3. SHA-256 SIMD Optimizations (`mining/sha256_simd.rs`)

Hardware-accelerated hashing with automatic detection and fallback.

```text
Detection Priority:
1. SHA-NI     (x86_64 with SHA extensions) - ~3x faster
2. AVX-512   (x86_64 with AVX-512)        - ~2.5x faster
3. AVX2      (x86_64 with AVX2)           - ~2x faster
4. Software  (portable fallback)
```

```rust
pub enum SimdCapability {
    ShaNi,    // Intel SHA Extensions (Goldmont+, Ice Lake+)
    Avx512,   // AVX-512F + AVX-512VL
    Avx2,     // AVX2 (Haswell+)
    None,     // Software fallback
}

pub fn detect_simd() -> SimdCapability {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sha") { return SimdCapability::ShaNi; }
        if is_x86_feature_detected!("avx512f") { return SimdCapability::Avx512; }
        if is_x86_feature_detected!("avx2") { return SimdCapability::Avx2; }
    }
    SimdCapability::None
}
```

### 4. GPU Mining (`gpu/`)

GPU acceleration via OpenCL (AMD, Intel, NVIDIA) with planned CUDA support.

#### Device Management (`manager.rs`)

```rust
pub struct GpuDevice {
    pub id: u32,
    pub name: String,
    pub vendor: String,
    pub memory_total: u64,
    pub compute_units: u32,
    pub max_work_group_size: usize,
    pub backend: GpuBackend,
    pub available: bool,
}

pub enum GpuBackend {
    OpenCL,
    Cuda,  // Future
}

pub trait GpuMiner: Send {
    fn set_job(&mut self, midstate: &[u32; 8], tail: &[u8; 16], target: &[u8; 32]);
    fn mine(&mut self, nonce_start: u32, nonce_count: u32) -> Vec<MiningResult>;
    fn device(&self) -> &GpuDevice;
    fn hashrate(&self) -> f64;
    fn stop(&mut self);
}
```

#### OpenCL Implementation (`opencl.rs`)

Full GPU mining implementation with the `ocl` crate:

```rust
pub struct OpenCLMiner {
    device: GpuDevice,
    intensity: u8,              // Work size control (1-100)
    queue: ocl::Queue,          // OpenCL command queue
    kernel: ocl::Kernel,        // Compiled SHA-256d kernel
    midstate_buf: ocl::Buffer<u32>,   // 8 x u32 midstate
    tail_buf: ocl::Buffer<u8>,        // 16 bytes tail
    target_buf: ocl::Buffer<u8>,      // 32 bytes target
    result_nonces_buf: ocl::Buffer<u32>,
    result_hashes_buf: ocl::Buffer<u8>,
    result_count_buf: ocl::Buffer<u32>,
    // ...
}
```

#### Mining Flow (GPU)

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         Host (Rust)                                  │
├─────────────────────────────────────────────────────────────────────┤
│ 1. Receive job (version, prev_hash, merkle_root, ntime, nbits)      │
│ 2. Compute midstate: SHA-256 of first 64 bytes                      │
│ 3. Upload to GPU: midstate, tail (16 bytes), target                 │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         GPU (OpenCL Kernel)                          │
├─────────────────────────────────────────────────────────────────────┤
│ Each work-item (one per nonce):                                      │
│   a. Load midstate, tail from global memory                         │
│   b. Build final block: tail + nonce + padding                      │
│   c. Complete first SHA-256 from midstate                           │
│   d. Perform second SHA-256 (double hash)                           │
│   e. Compare hash to target (big-endian)                            │
│   f. If meets target: atomic_inc(result_count), store result        │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Host (Rust)                                  │
├─────────────────────────────────────────────────────────────────────┤
│ 4. Wait for kernel completion                                        │
│ 5. Read result_count from GPU                                        │
│ 6. If results found: read nonces and hashes                         │
│ 7. Submit valid shares to pool/network                              │
└─────────────────────────────────────────────────────────────────────┘
```

#### Intensity Control

Work size is calculated based on intensity setting (1-100):

```rust
fn calculate_work_size(&self, nonce_count: u32) -> usize {
    let intensity_factor = (self.intensity as f64 / 100.0).max(0.01);
    let base_size = compute_units * work_group_size * 64;
    let target_size = (base_size * intensity_factor) as usize;
    // Round to work_group_size multiple
    (target_size / work_group_size) * work_group_size
}
```

| Intensity | Typical Work Size | GPU Utilization | Use Case |
|-----------|-------------------|-----------------|----------|
| 1-20      | ~16K-64K          | Low             | Background mining |
| 50        | ~256K             | Medium          | Balanced |
| 80-100    | ~1M+              | High            | Maximum hashrate |

### 5. Idle Mining (`idle/mod.rs`)

Screensaver-style mining that activates when the user is away.

```rust
pub struct IdleMonitor {
    timeout: Duration,
    pause_on_active: bool,
    running: Arc<AtomicBool>,
}

impl IdleMonitor {
    pub async fn get_idle_time() -> Option<Duration> {
        #[cfg(target_os = "linux")]
        {
            // X11: xprintidle or xssstate
            // WSL2: PowerShell query
        }
        #[cfg(target_os = "macos")]
        {
            // IOKit HIDIdleTime
        }
    }
}
```

#### Platform Support

| Platform | Method | Command |
|----------|--------|---------|
| Linux (X11) | xprintidle | `xprintidle` (milliseconds) |
| Linux (X11) | xssstate | `xssstate -i` (milliseconds) |
| WSL2 | PowerShell | Queries Windows LastInputInfo |
| macOS | IOKit | HIDIdleTime (planned) |

### 6. Mining Worker (`mining/worker.rs`)

Each worker runs in its own thread with:

- Dedicated nonce range to avoid conflicts
- SIMD-optimized hashing based on detected capabilities
- Job updates via watch channel
- Share submission via mpsc channel

```rust
// Mining loop (per worker)
loop {
    let job = wait_for_job();
    let header = job.build_header(&extranonce1, &extranonce2, 0);
    let hasher = select_hasher(simd_capability);

    for nonce in nonce_range {
        let hash = hasher.hash_header(&header, nonce);
        stats.hashes.fetch_add(1, Ordering::Relaxed);

        if meets_target(&hash, &job.target) {
            submit_share(job_id, nonce, hash);
        }
    }
}
```

### 7. Stratum V1 Client (`stratum/v1.rs`)

JSON-RPC based pool protocol.

#### Protocol Messages

| Message | Direction | Purpose |
|---------|-----------|---------|
| `mining.subscribe` | Client → Pool | Initialize connection |
| `mining.authorize` | Client → Pool | Authenticate worker |
| `mining.notify` | Pool → Client | New job notification |
| `mining.submit` | Client → Pool | Submit found share |
| `mining.set_difficulty` | Pool → Client | Update difficulty |

#### Job Structure

```rust
pub struct Job {
    pub id: String,
    pub prev_hash: [u8; 32],
    pub coinbase1: Vec<u8>,
    pub coinbase2: Vec<u8>,
    pub merkle_branches: Vec<[u8; 32]>,
    pub version: u32,
    pub nbits: u32,
    pub ntime: u32,
    pub target: [u8; 32],
}
```

### 8. Stratum V2 Client (`stratum/v2.rs`)

Binary protocol for efficient communication.

#### Frame Format

```text
┌────────────────────────────────────────────────────┐
│ Extension Type (2) │ Msg Type (1) │ Length (3)     │
├────────────────────────────────────────────────────┤
│                    Payload (variable)              │
└────────────────────────────────────────────────────┘
```

#### Message Types

| Type | Description |
|------|-------------|
| SetupConnection | Initialize connection |
| OpenMiningChannel | Open mining channel |
| NewMiningJob | New job notification |
| SubmitShares | Submit found share |

### 9. Bitcoin Core Integration (`bitcoin/`)

For true solo mining without a pool.

#### RPC Client (`rpc.rs`)

```rust
impl BitcoinRpc {
    pub async fn get_blockchain_info(&self) -> Result<SyncStatus>;
    pub async fn get_block_template(&self, address: &str) -> Result<BlockTemplate>;
    pub async fn submit_block(&self, block_hex: &str) -> Result<()>;
}
```

#### Manager (`manager.rs`)

Handles automatic Bitcoin Core lifecycle:

- Download and install Bitcoin Core
- Generate secure RPC credentials
- Start/stop daemon
- Monitor sync status
- Configure pruning for disk space efficiency

### 10. Web Interface (`web/mod.rs`)

Built with Axum for REST API and WebSocket support.

#### HTTP Endpoints

| Route | Method | Description |
|-------|--------|-------------|
| `/api/stats` | GET | Mining statistics |
| `/api/info` | GET | Miner information |
| `/api/control` | POST | Start/stop/pause mining |
| `/ws` | WebSocket | Real-time updates |
| `/` | GET | Static dashboard files |

#### WebSocket Protocol

```rust
pub struct StatsUpdate {
    pub hashrate: f64,
    pub hashes: u64,
    pub shares_accepted: u64,
    pub shares_rejected: u64,
    pub blocks_found: u64,
    pub block_height: u64,
    pub uptime: u64,
    pub is_mining: bool,
    pub idle_active: bool,
}
```

Real-time updates broadcast to all connected clients:

```rust
// Server-side broadcast
let (stats_tx, _) = broadcast::channel::<StatsUpdate>(100);

// Periodic stats update
loop {
    let stats = collect_stats();
    let _ = stats_tx.send(stats);
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

### 11. Web Notifications (`web/js/notifications.js`)

Browser notifications for important mining events.

#### Notification Types

| Event | Default | Description |
|-------|---------|-------------|
| New Block | Enabled | Network found new block, job updated |
| Found Block | Enabled | YOU found a block! (with sound) |
| Share Accepted | Disabled | Pool accepted share |
| Share Rejected | Enabled | Pool rejected share |
| Error | Enabled | Mining error occurred |
| Connection Lost | Enabled | Lost connection to miner |
| Idle Mining Start | Disabled | Mining started due to idle |

```javascript
function notifyFoundBlock(height, hash) {
    // Play celebratory sound
    const audio = new Audio('...');
    audio.play();

    showNotification('BLOCK FOUND!', {
        body: `You found block ${height}!`,
        requireInteraction: true,  // Don't auto-dismiss
    });
}
```

## Mining Modes

### 1. Pool Mining (Default)

```text
Miner ←─Stratum─→ Pool ←──→ Bitcoin Network
```

- Connect to pool via Stratum V1 or V2
- Submit shares to pool
- Pool handles block submission

### 2. True Solo Mining (`preset = "none"`)

```text
Miner ←─RPC─→ Bitcoin Core ←──→ Bitcoin Network
```

- Get block templates from local Bitcoin Core
- Mine blocks directly
- Submit found blocks to network
- Requires fully synced node (or pruned node)

### 3. Stratum V2 via SRI

```text
Miner ←─SV2─→ SRI Proxy ←─IPC─→ Bitcoin Core
```

- Connect to SRI (Stratum Reference Implementation) proxy
- SRI handles Bitcoin Core IPC communication
- More efficient than V1 protocol

## Data Flow

### Block Header Mining

```text
1. Receive job from pool/bitcoind
   │
   ▼
2. Parse job parameters
   │
   ├─ version (4 bytes)
   ├─ prev_hash (32 bytes)
   ├─ merkle_root (computed)
   ├─ ntime (4 bytes)
   ├─ nbits (4 bytes)
   └─ nonce (4 bytes)
   │
   ▼
3. Build 80-byte header
   │
   ├─► CPU Workers (SIMD-optimized)
   │     └─ SHA-NI / AVX2 / AVX512
   │
   └─► GPU Workers (OpenCL kernel)
         └─ Midstate optimization
   │
   ▼
4. Workers iterate nonces:
   │
   │  for nonce in range:
   │      hash = sha256d(header)
   │      if hash <= target:
   │          submit_share(nonce)
   │
   ▼
5. On new job, restart from step 2
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

### CPU Mining

1. **SIMD Instructions** - Hardware SHA-256 (SHA-NI) provides ~3x speedup
2. **Thread-per-worker model** - Each worker owns its nonce range
3. **Atomic statistics** - Lock-free hash counting
4. **Watch channels** - Efficient job broadcasting
5. **Batch hashing** - Process nonce ranges before checking job updates

### GPU Mining

1. **Midstate optimization** - Pre-compute first 64 bytes of SHA-256
2. **Coalesced memory access** - Aligned buffer reads
3. **Local memory** - Message schedule in fast local memory
4. **Atomic result collection** - Lock-free result buffer

### Real-time Updates

1. **WebSocket broadcast** - Single channel to all clients
2. **Selective notifications** - User-configurable event filters
3. **Efficient serialization** - Minimal JSON payload

## Testing

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# With coverage
./scripts/run_coverage.sh
```

### Test Coverage

| Module | Tests |
|--------|-------|
| `config` | Pool presets, validation |
| `mining::job` | Hash comparison |
| `mining::hasher` | SHA-256d correctness |
| `mining::sha256_simd` | SIMD detection, output verification |
| `gpu` | Device enumeration (when feature enabled) |
| `stats` | Hashrate calculation |

## Build Options

```bash
# Standard build
cargo build --release

# With GPU support
cargo build --release --features opencl
cargo build --release --features cuda

# With all features
cargo build --release --features "opencl cuda"
```

## References

- [Bitcoin Block Header](https://developer.bitcoin.org/reference/block_chain.html#block-headers)
- [Stratum V1 Protocol](https://braiins.com/stratum-v1/docs)
- [Stratum V2 Specification](https://stratumprotocol.org/)
- [SRI (Stratum Reference Implementation)](https://github.com/stratum-mining/stratum)
- [Bitcoin Core RPC](https://developer.bitcoin.org/reference/rpc/)
- [Intel SHA Extensions](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sha-extensions.html)
- [OpenCL Programming Guide](https://www.khronos.org/registry/OpenCL/specs/3.0-unified/html/OpenCL_API.html)
- [Web Notifications API](https://developer.mozilla.org/en-US/docs/Web/API/Notifications_API)
