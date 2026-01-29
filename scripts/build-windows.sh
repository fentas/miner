#!/bin/bash
# Sync project to Windows and build with OpenCL
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WIN_DIR="/mnt/c/Users/fentas/miner"

echo "Syncing project to Windows..."
rsync -av --delete \
    --exclude 'target' \
    --exclude '.git' \
    --exclude 'dist' \
    "$PROJECT_DIR/" "$WIN_DIR/"

echo "Building on Windows with OpenCL..."
cmd.exe /c "C:\\Users\\fentas\\build_opencl.bat"

echo "Done! Binary at: C:\\Users\\fentas\\miner\\target\\release\\btcminer.exe"
