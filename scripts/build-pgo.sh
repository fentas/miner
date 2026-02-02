#!/bin/bash
# Profile-Guided Optimization (PGO) build script
# Expected improvement: 10-15% hashrate increase
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
PGO_DIR="/tmp/pgo-data"

echo "=== Profile-Guided Optimization Build ==="
echo ""

# Find llvm-profdata (try rustup first, then system)
LLVM_PROFDATA=""
RUSTUP_LLVM="$(find ~/.rustup -name 'llvm-profdata' 2>/dev/null | head -1)"
if [ -n "$RUSTUP_LLVM" ]; then
    LLVM_PROFDATA="$RUSTUP_LLVM"
elif command -v llvm-profdata &> /dev/null; then
    LLVM_PROFDATA="llvm-profdata"
else
    echo "ERROR: llvm-profdata not found"
    echo "Install with: rustup component add llvm-tools-preview"
    echo "         or: sudo apt install llvm"
    exit 1
fi
echo "Using: $LLVM_PROFDATA"

cd "$PROJECT_DIR"

# Clean previous PGO data
rm -rf "$PGO_DIR"
mkdir -p "$PGO_DIR"

echo "Step 1: Building instrumented binary..."
RUSTFLAGS="-Cprofile-generate=$PGO_DIR" cargo build --release

echo ""
echo "Step 2: Running training workload (60 seconds)..."
echo "This generates profile data for optimization."
timeout 60 ./target/release/btcminer --benchmark --threads 1 || true

echo ""
echo "Step 3: Merging profile data..."
"$LLVM_PROFDATA" merge -o "$PGO_DIR/merged.profdata" "$PGO_DIR"

if [ ! -f "$PGO_DIR/merged.profdata" ]; then
    echo "ERROR: Failed to create merged profile data"
    exit 1
fi

echo ""
echo "Step 4: Building optimized binary with PGO..."
RUSTFLAGS="-Cprofile-use=$PGO_DIR/merged.profdata" cargo build --release

echo ""
echo "=== PGO Build Complete ==="
echo ""
echo "Optimized binary: target/release/btcminer"
echo "Profile data: $PGO_DIR/merged.profdata"
echo ""
echo "Run benchmark to compare:"
echo "  cargo test --release --test research_v11_test test_research_v11_baseline -- --nocapture"
