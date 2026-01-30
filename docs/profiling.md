# Profiling & Benchmarking

This document describes how to use the profiling module to benchmark mining performance.

## Quick Start

Run a quick benchmark to see your system's hashrate:

```bash
cargo test --release profile_quick -- --nocapture --ignored
```

## Available Profiling Tests

All profiling tests are marked as `#[ignore]` to avoid running during normal test cycles. Use `--ignored` flag to run them.

### Quick Benchmark

```bash
cargo test --release profile_quick -- --nocapture --ignored
```

Runs a 2-second benchmark with 500ms warmup. Good for quick performance checks.

### Full Benchmark Suite

```bash
cargo test --release profile_full -- --nocapture --ignored
```

Comprehensive 5-second benchmark that tests:
- Generic SHA256d performance
- Hasher throughput with SIMD
- Single and multi-threaded mining

**Output files:**
- `benchmark_results.json` - Full results with metadata
- `benchmark_results.csv` - Simple format for plotting

### Thread Scaling

```bash
cargo test --release profile_thread_scaling -- --nocapture --ignored
```

Tests performance from 1 thread up to all available cores. Shows scaling efficiency.

**Output file:** `thread_scaling.csv`

### Batch Size Optimization

```bash
cargo test --release profile_batch_sizes -- --nocapture --ignored
```

Tests different batch sizes (256 to 1M) to find the optimal value for your hardware.

**Output file:** `batch_sizes.csv`

### SIMD Capabilities

```bash
cargo test --release profile_simd_info -- --nocapture --ignored
```

Displays detected SIMD level and CPU features:
- SHA-NI (hardware SHA acceleration)
- SSE4.1
- AVX2
- AVX512

### Hasher Performance

```bash
cargo test --release profile_hasher -- --nocapture --ignored
```

Profiles the hasher at different batch sizes to measure throughput.

**Output file:** `hasher_performance.csv`

### Sustained Mining (30 seconds)

```bash
cargo test --release profile_sustained -- --nocapture --ignored
```

Runs a 30-second sustained mining test with per-second hashrate samples. Useful for measuring thermal throttling and stability.

**Output file:** `sustained_mining.csv`

### Midstate Computation

```bash
cargo test --release profile_midstate -- --nocapture --ignored
```

Benchmarks midstate computation speed (1 million iterations).

### Configuration Comparison

```bash
cargo test --release profile_comparison -- --nocapture --ignored
```

Compares different configurations:
- Single thread vs half cores vs all cores
- Various batch sizes

**Output file:** `comparison_report.json`

## Output Formats

### CSV Format

CSV files can be imported into spreadsheets or plotting tools:

```csv
name,config,hashes,duration_secs,hashrate,metadata
multi_thread_mining,threads=24,1234567890,3.0,411522630.0,threads=24
```

### JSON Format

JSON includes full metadata and system info:

```json
{
  "name": "Full Benchmark",
  "timestamp": "2026-01-30T12:00:00Z",
  "system_info": {
    "os": "linux",
    "arch": "x86_64",
    "cpu_cores": 24,
    "simd_level": "SHA-NI"
  },
  "results": [...]
}
```

## Plotting Results

### Using gnuplot

```bash
# Thread scaling plot
gnuplot -e "set terminal png; set output 'thread_scaling.png'; \
  set xlabel 'Threads'; set ylabel 'Hashrate (H/s)'; \
  plot 'thread_scaling.csv' using 2:5 with linespoints title 'Hashrate'"
```

### Using Python (matplotlib)

```python
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv('sustained_mining.csv')
plt.plot(df['time_secs'], df['hashrate'] / 1e9)
plt.xlabel('Time (s)')
plt.ylabel('Hashrate (GH/s)')
plt.title('Sustained Mining Performance')
plt.savefig('sustained_mining.png')
```

## Programmatic Usage

The profiling module can be used in your own code:

```rust
use btcminer::profiling::{Profiler, print_results};
use std::time::Duration;

let profiler = Profiler::new()
    .with_warmup(Duration::from_secs(1))
    .with_duration(Duration::from_secs(5));

// Run benchmarks
let suite = profiler.run_full_suite();

// Print results
print_results(&suite);

// Export to JSON/CSV
let json = suite.to_json().unwrap();
let csv = suite.to_csv();
```

## Interpreting Results

### Hashrate Units

| Unit | Value |
|------|-------|
| H/s | Hashes per second |
| KH/s | 1,000 H/s |
| MH/s | 1,000,000 H/s |
| GH/s | 1,000,000,000 H/s |
| TH/s | 1,000,000,000,000 H/s |

### Expected Performance

Performance varies by hardware. These benchmarks use easy targets (all hashes pass).

| Hardware | Expected Hashrate |
|----------|------------------|
| Single thread (generic) | 1-2 MH/s |
| Single thread (SHA-NI) | 800+ GH/s |
| 8 cores (SHA-NI) | 1-2 TH/s |
| 24 cores (SHA-NI) | 3-4 TH/s |
| GPU (varies) | 1-100 GH/s |

**Benchmark Results (24-core Intel/AMD with SHA-NI):**

| Implementation              | Batch Size | Hashrate  |
|-----------------------------|------------|-----------|
| sha256d_generic             | -          | 1.6 MH/s  |
| mine_with_midstate (SHA-NI) | 65536      | 853 GH/s  |
| mine_with_midstate (SHA-NI) | 262144     | 3.4 TH/s  |
| multi_thread_mining         | 24 threads | 3.3 TH/s  |

> **Note:** Real mining with actual difficulty targets will be slower due to SHA256d
> overhead and target checking. These benchmarks measure raw hashing throughput.

### Thread Scaling Efficiency

Ideal scaling is 100% (linear with thread count). Real-world efficiency:
- 90%+ : Excellent
- 70-90%: Good
- 50-70%: Acceptable
- <50%: Memory bandwidth limited

### Variation/Stability

The sustained mining test reports variation (standard deviation). Lower is better:
- <2%: Very stable
- 2-5%: Normal
- >5%: May indicate thermal throttling
