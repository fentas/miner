<h3 align="center">
	<img width="80" alt="BTC Miner" src="https://bitcoin.org/img/icons/opengraph.png">
</h3>

<h1 align="center">BTC Miner</h1>

<p align="center">
	<em>High-performance Bitcoin miner with SHA-NI, AVX2/AVX512, and GPU support</em>
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

<p align="left">

A C++17 Bitcoin miner optimized for modern hardware. Leverages CPU SIMD instructions (SHA-NI, AVX2, AVX512) and GPU acceleration (CUDA, OpenCL) for maximum hashrate. Features Stratum V1/V2 protocol support, web dashboard, and idle mining mode.

</p>

&nbsp;

### ⚡ Performance

| Implementation | Throughput |
|----------------|------------|
| SHA-NI | ~800-900 MH/s |
| AVX512 (16-way) | ~400-500 MH/s |
| AVX2 (8-way) | ~200-300 MH/s |
| Generic | ~10-20 MH/s |
| GPU | 100-1000+ MH/s |

&nbsp;

### 🚀 Quick Start

```bash
# Build
mkdir build && cd build
cmake .. -DNATIVE_BUILD=ON
make -j$(nproc)

# Configure
cp btcminer.conf.example btcminer.conf
# Edit with your Bitcoin address

# Run
./btcminer
```

&nbsp;

### 🔧 Build Options

```bash
cmake .. [OPTIONS]

# CPU Optimization
-DNATIVE_BUILD=ON       # Optimize for local CPU (default)

# GPU Support
-DENABLE_CUDA=ON        # NVIDIA CUDA
-DENABLE_OPENCL=ON      # AMD/Intel OpenCL

# Testing
-DBUILD_TESTS=ON        # Build test suite
-DENABLE_COVERAGE=ON    # Code coverage
```

&nbsp;

### 📝 Configuration

```ini
# btcminer.conf

# Solo Pool Preset (recommended)
SoloPool = ckpool         # ckpool or braiins (both use Stratum V1)

# Or manual configuration
# Pool = solo.ckpool.org:3333
# Stratum = v1

PayoutAddress = bc1q...   # Your Bitcoin address (required)

# Mining mode - at least one must be enabled
EnableCPU = true          # CPU mining (default: on)
EnableGPU = false         # GPU mining (default: off)
Threads = 0               # CPU threads (0 = auto-detect)

WebPort = 8080
```

**Pool Presets:**

| Preset    | Pool                      | Protocol         | Fee  | Description          |
|-----------|---------------------------|------------------|------|----------------------|
| `ckpool`  | solo.ckpool.org           | Stratum V1       | 2%   | Reliable solo pool   |
| `braiins` | solo.stratum.braiins.com  | Stratum V1       | 0.5% | Low fee option       |
| `ocean`   | mine.ocean.xyz:3334       | Stratum V1       | 0%*  | DATUM support        |
| `none`    | Your Bitcoin Core         | getblocktemplate | 0%   | **True solo mining** |

> **Note on Fees:**
>
> - **Solo.ckpool**: 2% service fee on successful block rewards
> - **Braiins Solo**: 0.5% fee on found blocks (paid to CKPool code authors)
> - **Ocean Pool**: 0% pool fee (miners pay only Bitcoin transaction fees via DATUM)
> - **True Solo (`none`)**: 0% fees - you keep 100% of the block reward!
>
> **Stratum V2 Pools** (require special setup):
>
> - **DMND Pool** (dmnd.work): 0% fee for solo mining - community project for network decentralization. Requires SV2 proxy setup.

**Mining Modes:**

| Mode      | EnableCPU | EnableGPU |
|-----------|-----------|-----------|
| CPU only  | true      | false     |
| GPU only  | false     | true      |
| CPU + GPU | true      | true      |

&nbsp;

### 🎯 True Solo Mining (No Pool)

For maximum decentralization with **0% fees**, mine directly to the Bitcoin network using your own node.

```ini
# btcminer.conf - True Solo Mining
SoloPool = none
PayoutAddress = bc1q...your_address
BitcoindEnabled = true
BitcoindManaged = true       # Auto-manages Bitcoin Core
BitcoindPrune = 10000        # ~10GB pruned node
```

**How it works:**

```text
┌─────────────┐    getblocktemplate    ┌───────────────┐
│             │ ◄───────────────────── │               │
│  btcminer   │                        │  Bitcoin Core │
│             │ ────────────────────► │               │
└─────────────┘     submitblock        └───────┬───────┘
                                               │
                                               ▼
                                        Bitcoin Network
```

1. Gets block template from your local Bitcoin Core node
2. Mines with full transaction selection control
3. Submits found blocks directly to the network
4. **You keep 100% of the block reward** (3.125 BTC + fees)

**vs Pool Mining:**

| Feature        | Pool Mining      | True Solo         |
|----------------|------------------|-------------------|
| Fees           | 0.5-2%           | **0%**            |
| Block template | Pool decides     | **You decide**    |
| Censorship     | Pool can censor  | **You control**   |
| Requirements   | Internet         | Bitcoin Core node |

> ⚠️ **Warning:** Solo mining is extremely unlikely to find a block. This is for educational/lottery purposes.

&nbsp;

### 🔗 Bitcoin Core Integration

Run your own Bitcoin Core node for custom block templates. Required for true solo mining (`SoloPool = none`).

#### Option 1: Managed Mode (Recommended)

Let btcminer handle everything automatically:

```ini
# btcminer.conf
BitcoindEnabled = true
BitcoindManaged = true
BitcoindPrune = 10000        # ~10GB pruned node
```

This will:
- Download Bitcoin Core v28.0 from bitcoincore.org
- Install to `~/.local/share/bitcoin-core`
- Generate `bitcoin.conf` with optimal settings
- Start/stop bitcoind with the miner

#### Option 2: Manual Setup

If you prefer to manage Bitcoin Core yourself:

```ini
# btcminer.conf
BitcoindEnabled = true
BitcoindManaged = false
BitcoindHost = 127.0.0.1
BitcoindRpcPort = 8332
BitcoindRpcUser = your_username
BitcoindRpcPassword = your_password
BitcoindPrune = 10000
```

Configure your `bitcoin.conf`:

```ini
server=1
rpcuser=your_username
rpcpassword=your_password
prune=10000
```

#### Stratum V2

Bitcoin Core v30+ supports an IPC Mining Interface for external Stratum V2 software. To use SV2:

1. Install the [SRI (Stratum V2 Reference Implementation)](https://stratumprotocol.org/)
2. Configure SRI proxy to connect to your Bitcoin Core node
3. Enable in btcminer:

```ini
BitcoindSv2 = true
```

**Benefits:**

- 🎯 Full transaction selection control
- 🔐 No pool template dependency
- 📦 Choose which transactions to include
- ⚡ Stratum V2 support (when Bitcoin Core v30+ releases)

**Requirements:**

- ~10GB disk space (pruned) or 600GB+ (full node)
- ~4GB RAM
- SSD recommended
- Initial sync: ~2-4 hours (pruned)

&nbsp;

### 💻 Command Line

```
./btcminer [options]

  -c, --config <file>      Config file
  -o, --pool <host:port>   Pool address
  -u, --user <address>     Bitcoin address
  -t, --threads <n>        CPU threads
  -w, --web <port>         Web dashboard port
  -h, --help               Show help
```

&nbsp;

### 🌐 Web Dashboard

Dashboard at `http://localhost:8080`:

- Real-time hashrate and statistics
- Live hashrate chart
- Mining controls (start/stop/pause)
- Benchmark mode
- Idle mining toggle

&nbsp;

### 🧪 Testing

```bash
./btcminer_test              # Run all tests
./btcminer_test --sha256     # SHA-256 tests
./btcminer_test --avx        # AVX tests
./btcminer_test --stratum    # Protocol tests
./btcminer_test --config     # Config validation tests

# Coverage report
./scripts/run_coverage.sh
```

&nbsp;

### 📁 Project Structure

```
src/
├── sha256.hpp          # SHA-256 (generic + SHA-NI)
├── sha256_avx.hpp      # AVX2/AVX512 multi-hashing
├── stratum.hpp/cpp     # Stratum V1
├── stratum_v2.hpp/cpp  # Stratum V2
├── miner.hpp/cpp       # Mining core
├── webserver.hpp       # HTTP/WebSocket
└── gpu/                # CUDA/OpenCL kernels
```

&nbsp;

### ⚠️ Disclaimer

**Solo mining Bitcoin with a CPU is extremely unlikely to find a block.** Current network difficulty requires ~10²³ hashes per block. This miner is for:

- Educational purposes
- Testing and development
- Lottery-style mining

&nbsp;

### 📜 License

`btcminer` is released under the MIT license, which grants the following permissions:

- Commercial use
- Distribution
- Modification
- Private use

For more details, see the [LICENSE](https://github.com/fentas/miner/blob/main/LICENSE).

&nbsp;

### ❤️ Acknowledgments

- SHA-NI implementation based on Intel reference code
- Stratum V1 based on slush's specification
- Stratum V2 based on the SV2 specification

&nbsp;

<p align="center">Copyright &copy; 2024-present <a href="https://github.com/fentas" target="_blank">fentas</a>
<p align="center"><a href="https://github.com/fentas/miner/blob/main/LICENSE"><img src="https://img.shields.io/static/v1.svg?style=for-the-badge&label=License&message=MIT&logoColor=d9e0ee&colorA=302d41&colorB=b7bdf8"/></a></p>
