# BTC Miner

A high-performance CPU Bitcoin miner written in C++17 with SHA-NI hardware acceleration support.

## Features

- **SHA-NI Hardware Acceleration**: Utilizes Intel SHA Extensions for ~10x faster hashing
- **Multi-threaded Mining**: Scales across all CPU cores
- **Stratum Protocol**: Compatible with solo.ckpool.org and other stratum pools
- **Midstate Optimization**: Pre-computes first 64 bytes of block header
- **Config File Support**: Easy configuration via `btcminer.conf`
- **Comprehensive Test Suite**: Verify correctness with `make test`

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
make test         # Build and run test suite
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
├── docs/              # Documentation
│   └── ARCHITECTURE.md
└── src/
    ├── main.cpp       # Entry point and CLI
    ├── config.hpp     # Configuration parser
    ├── sha256.hpp     # SHA-256 with SHA-NI/SIMD
    ├── stratum.hpp    # Stratum protocol client
    ├── stratum.cpp
    ├── miner.hpp      # Multi-threaded miner core
    ├── miner.cpp
    └── test.cpp       # Test suite
```

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
