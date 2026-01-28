# BTC Miner

A high-performance CPU Bitcoin miner written in C++17 with SHA-NI hardware acceleration support.

## Features

- **SHA-NI Hardware Acceleration**: Utilizes Intel SHA Extensions for ~10x faster hashing
- **Multi-threaded Mining**: Scales across all CPU cores
- **Stratum Protocol**: Compatible with solo.ckpool.org and other stratum pools
- **Midstate Optimization**: Pre-computes first 64 bytes of block header
- **Web Dashboard**: Real-time stats, hashrate chart, miner controls, settings UI
- **WebSocket Live Updates**: Stats broadcast to browser in real-time
- **Miner Controls**: Start/stop, pause/resume, benchmark mode via web UI
- **Idle Mining (Screensaver Mode)**: Auto-start mining when user is idle, stop when active
- **WSL2 Support**: Idle detection works on both native Linux and WSL2
- **Config File Support**: Easy configuration via `btcminer.conf`
- **Comprehensive Test Suite**: Verify correctness with `make test` and `make test-web`

## Performance

| CPU Feature | Single-Thread Hashrate |
|-------------|----------------------|
| SHA-NI (Intel/AMD) | ~800-900 MH/s |
| AVX2 (no SHA-NI) | ~50-100 MH/s |
| Generic | ~10-20 MH/s |

## Quick Start

```bash
# Build
make

# Run tests
make test

# Create config file
cp btcminer.conf.example btcminer.conf
# Edit btcminer.conf with your Bitcoin address

# Start mining
./btcminer
```

## Build Options

```bash
make              # Native build (optimized for your CPU)
make portable     # Portable build (SSE4.1 only)
make debug        # Debug build with symbols
make test         # Build and run SHA-256 test suite
make test-web     # Test web server and API endpoints
make test-all     # Run all tests
make clean        # Remove built files
```

## Configuration

### Config File (btcminer.conf)

```ini
# Pool connection
Pool = solo.ckpool.org:3333

# Your Bitcoin address for payouts (REQUIRED)
PayoutAddress = bc1q...

# Pool password (optional)
Password = x

# Number of mining threads (0 = auto-detect)
Threads = 0

# Stats reporting interval in seconds
StatsInterval = 10

# Web interface port (0 to disable)
WebPort = 8080

# Web files directory
WebDir = web

# Idle mining - screensaver mode (seconds, 0 = disabled)
# Starts mining when user is idle for this many seconds
IdleTimeout = 0
```

### Command Line Options

```
Usage: ./btcminer [options]

Options:
  -c, --config <file>      Config file (default: btcminer.conf)
  -o, --pool <host:port>   Pool address (default: solo.ckpool.org:3333)
  -u, --user <address>     Bitcoin address for mining rewards
  -p, --pass <password>    Pool password (default: x)
  -t, --threads <n>        Number of mining threads (default: all cores)
  -w, --web <port>         Web interface port (default: 8080, 0 to disable)
  -h, --help               Show this help message
```

Command line options override config file settings.

## Requirements

- C++17 compiler (GCC 7+ or Clang 5+)
- x86-64 CPU (Intel Skylake+ or AMD Zen+ recommended for SHA-NI)
- Linux (tested on Ubuntu 20.04+)

### CPU Feature Detection

The Makefile automatically detects your CPU's capabilities:

```bash
make
# Output shows:
# CPU features detected:
#   - SHA-NI: enabled/disabled
#   - AVX2: enabled/disabled
#   - SSE4.1: enabled/disabled
```

## Project Structure

```
.
├── btcminer.conf      # Configuration file
├── Makefile           # Build system
├── CMakeLists.txt     # Alternative CMake build
├── README.md          # This file
├── test_web.sh        # Web server test script
├── docs/              # Documentation
│   └── ARCHITECTURE.md
├── web/               # Web interface
│   └── index.html     # Dashboard UI
└── src/
    ├── main.cpp       # Entry point and CLI
    ├── config.hpp     # Configuration parser
    ├── sha256.hpp     # SHA-256 with SHA-NI/SIMD
    ├── stratum.hpp    # Stratum protocol client
    ├── stratum.cpp
    ├── miner.hpp      # Multi-threaded miner core
    ├── miner.cpp
    ├── webserver.hpp  # Built-in HTTP/WebSocket server
    ├── idle_monitor.hpp  # Idle detection (Linux/WSL2)
    └── test.cpp       # Test suite
```

## Web Interface

The miner includes a built-in web dashboard accessible at `http://localhost:8080`.

**Dashboard Features:**

- **Real-time Stats**: Hashrate, total hashes, shares, uptime
- **Live Hashrate Chart**: Visual history of mining performance
- **Miner Controls**: Start/stop, pause/resume mining with one click
- **Benchmark Mode**: Test your hashrate without connecting to pool
- **Idle Mining**: Enable screensaver-style mining with visual "zzz" animation
- **Settings Panel**: View and configure mining settings
- **Log Viewer**: Real-time mining log output
- **WebSocket Updates**: Stats update automatically without page refresh

**API Endpoints:**

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/stats` | GET | Current mining statistics |
| `/api/settings` | GET | Current configuration |
| `/api/settings` | POST | Update configuration |
| `/api/control/start` | POST | Start mining |
| `/api/control/stop` | POST | Stop mining |
| `/api/control/pause` | POST | Pause mining |
| `/api/control/resume` | POST | Resume mining |
| `/api/control/benchmark` | POST | Start benchmark (query: duration) |
| `/api/control/benchmark/stop` | POST | Stop benchmark |
| `/api/benchmark` | GET | Get benchmark results |
| `/api/idle` | GET | Get idle mining status |
| `/api/idle/enable` | POST | Enable idle mining (query: timeout) |
| `/api/idle/disable` | POST | Disable idle mining |
| `/ws` | WebSocket | Real-time stats stream |

**Disabling:** Set `WebPort = 0` in config or use `-w 0` on command line.

## Idle Mining (Screensaver Mode)

The miner can automatically start when your computer is idle and stop when you return.

**Supported Environments:**

- **Native Linux (X11)**: Uses `xprintidle` or `xssstate` for idle detection
- **WSL2**: Queries Windows idle time via PowerShell

**Configuration:**

```ini
# In btcminer.conf
IdleTimeout = 900  # Start mining after 15 minutes of inactivity
```

**Or enable via web UI:**
1. Go to the Dashboard
2. Toggle "Enable idle mining" in the Idle Mining section
3. Set your preferred timeout (in minutes)

When enabled, the web UI shows a "zzz" animation while waiting for idle, and switches to a mining indicator when active.

## How It Works

1. **Connect to Pool**: Establishes stratum connection and subscribes to mining jobs
2. **Receive Job**: Pool sends block template (prevhash, merkle root, difficulty target)
3. **Build Header**: Constructs 80-byte block header, computes midstate for first 64 bytes
4. **Mine**: Worker threads iterate through nonces, computing SHA-256d hashes
5. **Submit Share**: When hash meets target difficulty, submit to pool

### SHA-256 Optimization

Bitcoin mining requires computing `SHA256(SHA256(block_header))` billions of times.

**Midstate Optimization**: The 80-byte block header is processed in two 64-byte chunks. Since only the nonce (last 4 bytes) changes, we pre-compute the SHA-256 state after the first chunk (midstate) and reuse it.

**SHA-NI**: Intel's SHA Extensions provide hardware instructions for SHA-256:
- `SHA256RNDS2`: Two rounds of SHA-256
- `SHA256MSG1/2`: Message schedule computation

This eliminates the need for table lookups and reduces rounds to ~64 instructions.

## Testing

```bash
make test
```

The test suite verifies:
- SHA-256 correctness (NIST test vectors)
- SHA-256d (double hash)
- Bitcoin block header hashing
- Midstate computation
- Target comparison logic
- Mining simulation with low difficulty

## Disclaimer

**Solo mining Bitcoin with a CPU is extremely unlikely to find a block.** The current network difficulty requires approximately 10^23 hashes per block. Even at 1 GH/s, you would need billions of years on average to find a block.

This miner is provided for:
- Educational purposes
- Testing and development
- "Lottery ticket" mining (you never know!)

## License

MIT License

## Acknowledgments

- Inspired by [rieMiner](https://github.com/RiecoinTeam/rieMiner) architecture
- SHA-NI implementation based on Intel's reference code
- Stratum protocol based on [slush's specification](https://slushpool.com/help/stratum-protocol)
