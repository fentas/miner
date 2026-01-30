<h3 align="center">
	<img width="80" alt="BTC Miner" src="https://bitcoin.org/img/icons/opengraph.png">
</h3>

<h1 align="center">BTC Miner</h1>

<p align="center">
	<em>High-performance Bitcoin solo miner written in Rust</em>
</p>

<p align="center">
	<a href="https://github.com/fentas/miner/stargazers">
		<img alt="Stargazers" src="https://img.shields.io/github/stars/fentas/miner?style=for-the-badge&logo=starship&color=C9CBFF&logoColor=D9E0EE&labelColor=302D41"></a>
	<a href="https://github.com/fentas/miner/releases/latest">
		<img alt="Releases" src="https://img.shields.io/github/release/fentas/miner.svg?style=for-the-badge&logo=github&color=F2CDCD&logoColor=D9E0EE&labelColor=302D41"/></a>
	<a href="https://github.com/fentas/miner/issues">
		<img alt="Issues" src="https://img.shields.io/github/issues/fentas/miner?style=for-the-badge&logo=gitbook&color=B5E8E0&logoColor=D9E0EE&labelColor=302D41"></a>
</p>

&nbsp;

## Features

- **TUI Dashboard** - k9s-style terminal UI with hashrate graph, stats, logs, and shortcuts
- **Multi-threaded CPU Mining** - Worker threads with optimized nonce distribution
- **SIMD Optimizations** - SHA-NI, AVX2, and AVX512 hardware acceleration
- **GPU Mining** - OpenCL and CUDA support for additional hashrate
- **Stratum V1/V2** - Pool protocol support with automatic reconnection
- **True Solo Mining** - Direct Bitcoin Core integration with managed mode
- **Fast Sync** - AssumeUTXO support for quick initial sync (minutes vs days)
- **Idle Mining** - Screensaver-style mining (X11/WSL2) - mine when you're away
- **Web Dashboard** - Real-time stats with WebSocket updates
- **Web Notifications** - Browser notifications for blocks, shares, errors
- **TOML Configuration** - SRI-compatible configuration format

&nbsp;

## Quick Start

### Linux / macOS

```bash
# Build
cargo build --release

# Configure
cp btcminer.example.toml btcminer.toml
# Edit btcminer.toml with your Bitcoin address

# Run
./target/release/btcminer
```

### Windows

```powershell
# Build
cargo build --release

# Configure
copy btcminer.example.toml btcminer.toml
# Edit btcminer.toml with your Bitcoin address

# Run
.\target\release\btcminer.exe
```

### One-Line Install

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/fentas/miner/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/fentas/miner/main/install.ps1 | iex
```

### Pre-built Binaries

Download from [Releases](https://github.com/fentas/miner/releases):

| Platform    | Standard                        | GPU (OpenCL)                          |
| ----------- | ------------------------------- | ------------------------------------- |
| Linux x64   | `btcminer-*-linux-x64.tar.gz`   | `btcminer-*-linux-x64-opencl.tar.gz`  |
| Windows x64 | `btcminer-*-windows-x64.zip`    | `btcminer-*-windows-x64-opencl.zip`   |
| macOS Intel | `btcminer-*-macos-x64.tar.gz`   | -                                     |
| macOS ARM   | `btcminer-*-macos-arm64.tar.gz` | -                                     |

&nbsp;

## Configuration

TOML-based configuration for consistency with SRI ecosystem:

```toml
# btcminer.toml

[pool]
preset = "ckpool"              # ckpool, braiins, ocean, none, custom
payout_address = "bc1q..."     # Your Bitcoin address (required)

[mining]
enable_cpu = true
threads = 0                    # 0 = auto-detect
enable_gpu = false             # Enable OpenCL/CUDA
gpu_devices = []               # Specific GPU IDs or empty for all
gpu_intensity = 100            # GPU intensity (1-100)
gpu_workgroup_size = 0         # 0 = auto, or 64-1024
gpu_batch_exp = 25             # Batch size: 2^25 = 32M nonces

[bitcoind]
enabled = false
managed = false                # Auto-manage Bitcoin Core
prune = 10000                  # Pruned node size in MB

[sv2]
enabled = false
proxy_host = "127.0.0.1"
proxy_port = 34255

[web]
port = 8080

[idle]
enabled = false
timeout = 900                  # Idle timeout in seconds (15 min)
pause_on_active = true         # Pause mining when user returns

[logging]
colors = true
stats_interval = 10
system_bell = true             # Bell on share accepted
```

**Pool Presets:**

| Preset    | Pool                      | Protocol    | Fee   | Description          |
|-----------|---------------------------|-------------|-------|----------------------|
| `ckpool`  | solo.ckpool.org           | Stratum V1  | 2%    | Reliable solo pool   |
| `braiins` | solo.stratum.braiins.com  | Stratum V1  | 0.5%  | Low fee option       |
| `ocean`   | mine.ocean.xyz:3334       | Stratum V1  | 0%    | DATUM support        |
| `none`    | Your Bitcoin Core         | Direct      | 0%    | **True solo mining** |

&nbsp;

## True Solo Mining (No Pool)

Mine directly to the Bitcoin network with **0% fees**:

```toml
[pool]
preset = "none"
payout_address = "bc1q...your_address"

[bitcoind]
enabled = true
managed = true         # Auto-manages Bitcoin Core
prune = 10000          # ~10GB pruned node
dbcache = 0            # 0 = auto (uses 50% of available RAM)
assumeutxo = true      # Fast sync in minutes instead of days
```

**Performance Tips:**

- `dbcache=0` auto-calculates based on your RAM (~4-8GB recommended for fast sync)
- `assumeutxo=true` downloads a ~7GB UTXO snapshot for instant usability
- With AssumeUTXO, you can start mining immediately while the node validates history in background

**How it works:**

```text
┌─────────────┐    getblocktemplate    ┌───────────────┐
│             │ ◄───────────────────── │               │
│  btcminer   │                        │  Bitcoin Core │
│             │ ─────────────────────► │               │
└─────────────┘     submitblock        └───────┬───────┘
                                               │
                                               ▼
                                        Bitcoin Network
```

> **Warning:** Solo mining is extremely unlikely to find a block. This is for educational/lottery purposes.

&nbsp;

## GPU Mining

Enable GPU acceleration for additional hashrate:

```toml
[mining]
enable_cpu = true
enable_gpu = true
gpu_devices = []           # Empty = all GPUs, or [0, 1] for specific devices
gpu_intensity = 100        # 1-100, higher = more GPU usage
gpu_workgroup_size = 0     # 0 = auto, 64-1024 for manual tuning
gpu_batch_exp = 25         # Batch size 2^n: 24=16M, 25=32M, 26=64M, 27=128M
```

**GPU Tuning:**

- `gpu_intensity` - Controls work size, 100 = maximum utilization
- `gpu_workgroup_size` - Work group size for OpenCL (0 = auto-detect optimal)
- `gpu_batch_exp` - Nonces per batch: higher = better utilization, more latency

**Performance Tips (AMD RDNA2+):**

- Use `gpu_workgroup_size = 1024` for modern AMD GPUs
- Use `gpu_batch_exp = 26` or higher for dedicated GPUs
- SHA256 on GPUs is fundamentally limited - ASICs are 100,000x faster

**Install dependencies:**

```bash
# Ubuntu/Debian - OpenCL
sudo apt install ocl-icd-opencl-dev

# Ubuntu/Debian - NVIDIA (includes OpenCL)
sudo apt install nvidia-cuda-toolkit

# Ubuntu/Debian - AMD
sudo apt install mesa-opencl-icd

# WSL2 - Just need the ICD loader (uses Windows GPU driver)
sudo apt install ocl-icd-opencl-dev
```

**Build with GPU support:**

```bash
# OpenCL support
cargo build --release --features opencl

# CUDA support (requires CUDA toolkit)
cargo build --release --features cuda
```

&nbsp;

## Desktop Overlay

A transparent floating window showing hashrate and controls:

```bash
# Build the overlay
cargo build --release --features overlay

# Run (requires btcminer running with web API on port 8080)
./target/release/btcminer-overlay
```

**Configuration:**

```toml
[overlay]
auto_open = true  # Auto-launch overlay on startup
```

**Features:**
- Transparent, borderless, always-on-top window
- Draggable by clicking anywhere
- Shows: hashrate, status (Mining/Paused/Idle), shares count
- Start/Pause button and close button

&nbsp;

## Idle Mining (Screensaver Mode)

Automatically mine when your computer is idle:

```toml
[idle]
enabled = true
timeout = 900          # Start mining after 15 minutes idle
pause_on_active = true # Pause when user returns
```

**Supported environments:**
- Linux (X11) - Uses `xprintidle` or `xssstate`
- WSL2 - Queries Windows idle time via PowerShell
- macOS - IOKit HIDIdleTime (planned)

&nbsp;

## Stratum V2 with SRI

For Stratum V2, connect through an [SRI (Stratum Reference Implementation)](https://stratumprotocol.org/) proxy:

```toml
[sv2]
enabled = true
proxy_host = "127.0.0.1"
proxy_port = 34255
```

**Benefits of SV2:**

- Binary protocol (more efficient than JSON)
- Job negotiation for custom block templates
- Optional encryption

&nbsp;

## TUI Keyboard Shortcuts (k9s-style)

| Key       | Action                                  |
| --------- | --------------------------------------- |
| `?`       | Help overlay (shows all shortcuts)      |
| `:`       | Command mode (`:q` quit, `:clear`)      |
| `l`       | Focus logs (full screen)                |
| `Shift+L` | Toggle Bitcoin Core logs                |
| `h`       | Toggle hashrate display (Total/CPU/GPU) |
| `Shift+H` | Hashrate history chart (expanded)       |
| `p`       | Pause/Resume mining                     |
| `j/k`     | Scroll logs (vim-style)                 |
| `0-6`     | Time filter (tail, 1m-1h, all)          |

See [TUI Reference](docs/TUI.md) for all shortcuts.

&nbsp;

## Command Line

```
btcminer [OPTIONS]

Options:
  -c, --config <FILE>    Config file [default: btcminer.toml]
  -t, --threads <N>      Override thread count
  -d, --debug            Enable debug logging
      --no-color         Disable colored output
      --no-hud           Disable TUI, use continuous log output
      --log-format       Log format: text or json
  -h, --help             Show help
```

&nbsp;

## Web Dashboard

Dashboard at `http://localhost:8080`:

- Real-time hashrate chart
- Share statistics (accepted/rejected)
- Miner controls (start/stop/pause)
- Benchmark mode
- Idle mining toggle
- Settings management
- Web notifications (new block, found block, errors)

&nbsp;

## Project Structure

```
src/
├── main.rs           # Entry point, colored output
├── config.rs         # TOML configuration
├── mining/           # Mining core
│   ├── mod.rs        # Coordinator, stats
│   ├── worker.rs     # Worker threads
│   ├── job.rs        # Job structure
│   ├── hasher.rs     # SHA-256d
│   └── sha256_simd.rs # SIMD optimizations
├── gpu/              # GPU mining
│   ├── manager.rs    # Device management
│   └── opencl.rs     # OpenCL implementation
├── stratum/          # Pool protocols
│   ├── v1.rs         # Stratum V1
│   └── v2.rs         # Stratum V2
├── bitcoin/          # Bitcoin Core
│   ├── rpc.rs        # RPC client
│   └── manager.rs    # Lifecycle management
├── idle/             # Idle monitor
│   └── mod.rs        # X11/WSL2 detection
├── web/              # Web interface
│   └── mod.rs        # Axum + WebSocket
└── stats/            # Statistics
    └── mod.rs        # Hashrate calculation

web/
├── index.html        # Dashboard HTML
├── css/
│   └── styles.css    # Dashboard styles
└── js/
    ├── app.js        # Main application
    ├── chart.js      # Hashrate chart
    ├── websocket.js  # Real-time updates
    ├── idle.js       # Idle monitoring
    └── notifications.js # Web notifications
```

&nbsp;

## Testing

```bash
# Run unit tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Specific test
cargo test test_sha256d

# E2E integration tests (mock RPC server)
cargo test --test e2e_mining_tests

# Binary E2E tests (spawns actual btcminer binary)
cargo test --test binary_e2e_tests --release
```

### Test Categories

| Test Suite          | Description                                  |
| ------------------- | -------------------------------------------- |
| Unit tests          | Fast tests for individual functions          |
| `e2e_mining_tests`  | Integration tests with mock Bitcoin RPC      |
| `binary_e2e_tests`  | True E2E tests spawning the compiled binary  |

&nbsp;

## Profiling

CPU profiling support for performance analysis:

```bash
# Run profiler (default 10 seconds)
cargo run --release -- --profile

# Custom duration
cargo run --release -- --profile --profile-duration 30

# Output to specific file
cargo run --release -- --profile --profile-output results.json
```

**Output formats:**

- JSON file with per-thread stats
- Console summary with hashrate breakdown

See [Profiling Documentation](docs/profiling.md) for detailed usage and visualization.

&nbsp;

## Disclaimer

**Solo mining Bitcoin with a CPU is extremely unlikely to find a block.** Current network difficulty requires ~10^23 hashes per block. This miner is for:

- Educational purposes
- Testing and development
- Lottery-style mining

&nbsp;

## License

`btcminer` is released under the MIT license.

&nbsp;

## Acknowledgments

- [SRI (Stratum Reference Implementation)](https://github.com/stratum-mining/stratum) for protocol specifications
- Stratum V1 based on slush's specification
- Stratum V2 based on the [SV2 specification](https://stratumprotocol.org/)

&nbsp;

<p align="center">Copyright &copy; 2024-present <a href="https://github.com/fentas" target="_blank">fentas</a>
<p align="center"><a href="https://github.com/fentas/miner/blob/main/LICENSE"><img src="https://img.shields.io/static/v1.svg?style=for-the-badge&label=License&message=MIT&logoColor=d9e0ee&colorA=302d41&colorB=b7bdf8"/></a></p>
