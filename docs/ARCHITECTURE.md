# Architecture

This document describes the internal architecture of BTC Miner.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         main.cpp                            │
│  - CLI argument parsing                                     │
│  - Config file loading                                      │
│  - Signal handling                                          │
│  - Stats reporting                                          │
└───────┬─────────────────┬─────────────────┬─────────────────┘
        │                 │                 │
        ▼                 ▼                 ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│ Stratum Client│ │  Miner Core   │ │  Web Server   │
│ (stratum.cpp) │◄│  (miner.cpp)  │ │(webserver.hpp)│
└───────┬───────┘ └───────┬───────┘ └───────┬───────┘
        │                 │                 │
        │                 ▼                 ▼
        │         ┌───────────────┐ ┌───────────────┐
        │         │Worker Threads │ │  Idle Monitor │
        │         │  (N threads)  │ │(idle_monitor) │
        │         └───────┬───────┘ └───────────────┘
        │                 │
        │                 ▼
        │         ┌───────────────┐
        ▼         │    SHA-256    │
   ┌─────────┐    │ (sha256.hpp)  │
   │  Pool   │    └───────────────┘
   │(ckpool) │
   └─────────┘
```

## Components

### 1. SHA-256 Engine (`sha256.hpp`)

Header-only implementation with compile-time feature detection.

#### Implementations

| Function | Description | Performance |
|----------|-------------|-------------|
| `transform_shani()` | SHA-NI hardware acceleration | ~800 MH/s |
| `transform_generic()` | Portable C++ implementation | ~20 MH/s |

#### Key Functions

```cpp
// Full 80-byte header double-hash
void sha256d_80(const uint8_t header[80], uint8_t hash[32]);

// Pre-compute midstate for first 64 bytes
void compute_midstate(const uint8_t header[80], Midstate& ms);

// Hash with midstate (fast path for mining)
void sha256d_midstate(const Midstate& ms, uint32_t nonce, uint8_t hash[32]);
```

#### SHA-NI Instructions Used

- `_mm_sha256rnds2_epu32`: 2 rounds of SHA-256 compression
- `_mm_sha256msg1_epu32`: First part of message schedule
- `_mm_sha256msg2_epu32`: Second part of message schedule

### 2. Stratum Client (`stratum.hpp/cpp`)

Implements the Stratum mining protocol for pool communication.

#### Protocol Messages

| Message | Direction | Purpose |
|---------|-----------|---------|
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

### 3. Miner Core (`miner.hpp/cpp`)

Multi-threaded mining engine with work distribution.

#### Threading Model

```
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

#### Statistics

```cpp
struct Stats {
    std::atomic<uint64_t> hashes{0};
    std::atomic<uint64_t> shares_found{0};
    std::atomic<uint64_t> shares_accepted{0};
    std::atomic<uint64_t> shares_rejected{0};
};
```

### 4. Configuration (`config.hpp`)

Simple key-value config parser.

```cpp
struct Config {
    std::string pool_host;
    uint16_t pool_port;
    std::string payout_address;
    std::string password;
    unsigned threads;
    unsigned stats_interval;
    uint16_t web_port;
    std::string web_dir;
    unsigned idle_timeout;        // Screensaver mode timeout
    bool idle_mining_enabled;
    bool colors;                  // ANSI color output
    std::string log_file;         // File logging (empty = disabled)
    bool system_bell;             // Audio notification on share
    unsigned reconnect_delay;     // Initial reconnect delay (seconds)
    unsigned max_reconnect_delay; // Max backoff delay (seconds)
};
```

### 5. Web Server (`webserver.hpp`)

Header-only HTTP/WebSocket server for the dashboard.

#### Features

- Static file serving (HTML, CSS, JS)
- REST API endpoints for stats, settings, and control
- WebSocket for real-time stats broadcast
- No external dependencies (uses raw sockets)

#### API Routes

| Route | Method | Description |
|-------|--------|-------------|
| `/api/stats` | GET | Mining statistics |
| `/api/settings` | GET/POST | Configuration |
| `/api/control/*` | POST | Miner control (start/stop/pause) |
| `/api/benchmark` | GET/POST | Benchmark mode |
| `/api/idle` | GET/POST | Idle mining control |
| `/ws` | WebSocket | Real-time updates |

### 6. Idle Monitor (`idle_monitor.hpp`)

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
|-------------|--------|-----|
| Native Linux | xprintidle / xssstate | X11 screen saver extension |
| WSL2 | PowerShell | Windows GetLastInputInfo |

#### Callback Flow

```
Idle Monitor Thread
        │
        ▼
┌──────────────────┐
│ Check idle time  │◄─── Every 5 seconds
└────────┬─────────┘
         │
         ▼
    ┌────────────┐
    │ idle_time  │
    │ > timeout? │
    └─────┬──────┘
          │
    ┌─────┴─────┐
    │Yes        │No
    ▼           ▼
┌────────┐  ┌────────┐
│ Start  │  │ Stop   │
│ Mining │  │ Mining │
└────────┘  └────────┘
```

## Data Flow

### Mining Loop

```
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

```
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

### 3. Nonce Chunking

Workers grab large nonce ranges atomically to minimize synchronization:

```cpp
uint32_t nonce_start = nonce_base_.fetch_add(NONCE_CHUNK);
// Mine 1M nonces without touching atomics
```

### 4. Target Pre-computation

nbits to 256-bit target conversion done once per job, not per hash.

## Recent Features

- **Pool Reconnection**: Auto-reconnects with exponential backoff
- **File Logging**: Timestamped stats to configurable log file
- **Block Height Parsing**: Extracts from coinb1 (BIP34 format)
- **Estimated Time to Share**: Based on hashrate vs difficulty
- **Random Nonce Start**: Avoids duplicate work after restart
- **System Bell**: Terminal beep on share found
- **Pool Difficulty Display**: Shows in UI and stats output

## Future Improvements

1. **AVX2/AVX512 Multi-hashing**: Process 4-8 hashes in parallel
2. **Stratum V2**: More efficient binary protocol (config placeholder ready)
3. **GPU Support**: OpenCL/CUDA for massive parallelism (config placeholder ready)
4. **Wayland Support**: Idle detection via D-Bus idle portal
5. **macOS Support**: IOKit idle detection
6. **Power Management**: Reduce threads when on battery
7. **Multiple Pool Support**: Failover to backup pools

## References

- [Bitcoin Block Header](https://developer.bitcoin.org/reference/block_chain.html#block-headers)
- [Stratum Protocol](https://braiins.com/stratum-v1/docs)
- [Intel SHA Extensions](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sha-extensions.html)
- [SHA-256 Specification (FIPS 180-4)](https://csrc.nist.gov/publications/detail/fips/180/4/final)
