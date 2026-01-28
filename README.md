# BTC Miner

A high-performance Bitcoin miner written in C++17 with hardware acceleration support for CPU (SHA-NI, AVX2, AVX512) and GPU (CUDA, OpenCL).

## Features

### Mining
- **SHA-NI Hardware Acceleration**: Intel/AMD SHA Extensions for maximum CPU performance
- **AVX2/AVX512 Multi-hashing**: Process 8-16 nonces in parallel on modern CPUs
- **GPU Mining**: CUDA (NVIDIA) and OpenCL (AMD/Intel) support
- **Multi-threaded**: Scales across all CPU cores
- **Midstate Optimization**: Pre-computes first 64 bytes of block header

### Protocol
- **Stratum V1**: JSON-based protocol (widely supported)
- **Stratum V2**: Binary protocol with Noise encryption (more efficient)
- **Auto-reconnect**: Exponential backoff on connection loss

### Interface
- **Web Dashboard**: Real-time stats, hashrate chart, controls
- **WebSocket Updates**: Live stats without page refresh
- **Idle Mining**: Screensaver mode - mine when idle, stop on activity
- **File Logging**: Timestamped stats and events

## Performance

| Implementation | Throughput (per thread) |
|----------------|------------------------|
| SHA-NI | ~800-900 MH/s |
| AVX512 (16-way) | ~400-500 MH/s |
| AVX2 (8-way) | ~200-300 MH/s |
| Generic | ~10-20 MH/s |
| GPU (varies) | 100-1000+ MH/s |

## Quick Start

```bash
# Build with CMake
mkdir build && cd build
cmake .. -DNATIVE_BUILD=ON
make -j$(nproc)

# Or use Make
make

# Create config
cp btcminer.conf.example btcminer.conf
# Edit with your Bitcoin address

# Run
./btcminer
```

## Build Options

### CMake

```bash
cmake .. [OPTIONS]

# Options:
#   -DNATIVE_BUILD=ON       Optimize for local CPU (default: ON)
#   -DENABLE_CUDA=ON        Enable CUDA GPU mining
#   -DENABLE_OPENCL=ON      Enable OpenCL GPU mining
#   -DBUILD_TESTS=ON        Build test suite (default: ON)
#   -DENABLE_COVERAGE=ON    Enable code coverage
```

### Make

```bash
make              # Native build
make portable     # Portable (SSE4.1 only)
make debug        # Debug build
make test         # Run tests
make clean        # Clean build
```

### GPU Support

```bash
# CUDA (NVIDIA)
cmake .. -DENABLE_CUDA=ON
make

# OpenCL (AMD/Intel/NVIDIA)
cmake .. -DENABLE_OPENCL=ON
make
```

## Configuration

### Config File (btcminer.conf)

```ini
# Pool connection
Pool = solo.ckpool.org:3333
PayoutAddress = bc1q...
Password = x

# Protocol: v1 (JSON) or v2 (binary)
Stratum = v1

# CPU mining threads (0 = auto)
Threads = 0

# GPU mining
EnableGPU = false

# Web interface
WebPort = 8080
WebDir = web

# Idle mining (seconds, 0 = disabled)
IdleTimeout = 0

# Logging
LogFile = miner.log
StatsInterval = 10

# Reconnection
ReconnectDelay = 5
MaxReconnectDelay = 300
```

### Command Line

```
./btcminer [options]

Options:
  -c, --config <file>      Config file (default: btcminer.conf)
  -o, --pool <host:port>   Pool address
  -u, --user <address>     Bitcoin address
  -p, --pass <password>    Pool password
  -t, --threads <n>        CPU threads (0 = all cores)
  -w, --web <port>         Web port (0 = disabled)
  --no-color               Disable colored output
  -h, --help               Show help
```

## Project Structure

```
.
├── btcminer.conf.example   # Configuration template
├── CMakeLists.txt          # CMake build
├── Makefile                # Make build
├── README.md
├── docs/
│   └── ARCHITECTURE.md     # Technical documentation
├── scripts/
│   └── run_coverage.sh     # Coverage report generator
├── tests/                  # Test suite
│   ├── test_framework.hpp
│   ├── test_main.cpp
│   ├── test_sha256.cpp
│   ├── test_sha256_avx.cpp
│   ├── test_stratum.cpp
│   ├── test_stratum_v2.cpp
│   └── test_gpu.cpp
├── web/
│   └── index.html          # Dashboard UI
└── src/
    ├── main.cpp            # Entry point
    ├── config.hpp          # Configuration
    ├── sha256.hpp          # SHA-256 (generic + SHA-NI)
    ├── sha256_avx.hpp      # AVX2/AVX512 multi-hashing
    ├── stratum.hpp/.cpp    # Stratum V1 client
    ├── stratum_v2.hpp/.cpp # Stratum V2 client
    ├── miner.hpp/.cpp      # Mining core
    ├── webserver.hpp       # HTTP/WebSocket server
    ├── idle_monitor.hpp    # Idle detection
    ├── colors.hpp          # Terminal colors
    └── gpu/
        ├── gpu.hpp/.cpp        # GPU interface
        ├── cuda_miner.cuh      # CUDA miner
        ├── opencl_miner.hpp    # OpenCL miner
        ├── sha256d.cu          # CUDA kernel
        └── sha256d.cl          # OpenCL kernel
```

## Testing

```bash
# Run all tests
./btcminer_test

# Run specific test group
./btcminer_test --sha256
./btcminer_test --avx
./btcminer_test --stratum
./btcminer_test --stratum-v2
./btcminer_test --gpu

# With coverage
./scripts/run_coverage.sh
# Open build_coverage/coverage_html/index.html
```

## Web Interface

Dashboard at `http://localhost:8080`:

- Real-time hashrate and statistics
- Live hashrate chart
- Start/stop, pause/resume controls
- Benchmark mode
- Idle mining toggle
- Settings panel
- Log viewer

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/stats` | GET | Mining statistics |
| `/api/settings` | GET/POST | Configuration |
| `/api/control/start` | POST | Start mining |
| `/api/control/stop` | POST | Stop mining |
| `/api/control/pause` | POST | Pause mining |
| `/api/control/resume` | POST | Resume mining |
| `/api/benchmark` | GET/POST | Benchmark mode |
| `/api/idle` | GET/POST | Idle mining control |
| `/ws` | WebSocket | Real-time updates |

## Requirements

- C++17 compiler (GCC 7+ or Clang 5+)
- x86-64 CPU
- Linux (Ubuntu 20.04+ tested)
- Optional: CUDA Toolkit (for NVIDIA GPU)
- Optional: OpenCL SDK (for AMD/Intel GPU)

## How It Works

1. **Connect**: Establish Stratum connection to pool
2. **Subscribe**: Receive extranonce and job notifications
3. **Build Header**: Construct 80-byte block header
4. **Compute Midstate**: SHA-256 state after first 64 bytes
5. **Mine**: Iterate nonces, compute SHA-256d
6. **Submit**: Send valid shares to pool

### Optimization Techniques

- **Midstate**: Pre-compute SHA-256 state for fixed header bytes
- **SHA-NI**: Hardware SHA-256 instructions (2 rounds/instruction)
- **AVX2/AVX512**: Process 8-16 nonces in parallel
- **GPU**: Massive parallelism with thousands of threads

## Disclaimer

**Solo mining Bitcoin with a CPU is extremely unlikely to find a block.** Current network difficulty requires ~10^23 hashes per block. This miner is for:

- Educational purposes
- Testing and development
- Lottery-style mining

## License

MIT License

## Acknowledgments

- SHA-NI implementation based on Intel reference code
- Stratum V1 based on slush's specification
- Stratum V2 based on the SV2 specification
