#!/bin/bash
# Sync project to Windows and build
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="/mnt/c/Users/fentas/miner"

echo "Syncing project to Windows..."
rsync -av --delete \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'dist' \
    --exclude '*.txt' \
    "$PROJECT_DIR/" "$WIN_DIR/"

echo "Building on Windows..."
powershell.exe -Command "
cd C:\Users\fentas\miner
cargo clean -p btcminer 2>\$null
cargo build --release
"

echo "Done! Binary at: C:\Users\fentas\miner\target\release\btcminer.exe"
# powershell.exe -Command "cd C:\Users\fentas\miner; .\target\release\btcminer.exe --no-hud --debug"
