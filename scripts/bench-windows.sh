#!/bin/bash
# Run benchmarks on Windows native to compare with WSL2
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="/mnt/c/Users/fentas/miner"

# Default benchmark filter
BENCH_FILTER="${1:-hasher_comparison}"

echo "Syncing project to Windows..."
rsync -av --delete \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'dist' \
    --exclude '*.txt' \
    "$PROJECT_DIR/" "$WIN_DIR/"

echo "Cleaning and rebuilding with LTO..."
powershell.exe -Command "
cd C:\Users\fentas\miner
cargo clean -p btcminer 2>&1
"

echo "Running benchmarks on Windows (filter: $BENCH_FILTER)..."
powershell.exe -Command "
cd C:\Users\fentas\miner
cargo bench -- $BENCH_FILTER 2>&1
"

echo "Done!"
