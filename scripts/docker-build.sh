#!/bin/bash
# Build btcminer using Docker
# Usage: ./scripts/docker-build.sh [windows|linux|all]

set -e

TARGET=${1:-all}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

build_windows() {
    echo "Building Windows binary..."

    # Use the simple Dockerfile with cargo-xwin
    docker build \
        -f Dockerfile.simple \
        --target output \
        --output type=local,dest=./dist/windows \
        .

    echo "Windows binary built: dist/windows/btcminer.exe"
}

build_linux() {
    echo "Building Linux binary..."

    docker build \
        -f Dockerfile \
        --target linux-runtime \
        -t btcminer:linux \
        .

    # Extract binary
    mkdir -p dist/linux
    docker create --name btcminer-extract btcminer:linux
    docker cp btcminer-extract:/usr/local/bin/btcminer dist/linux/
    docker rm btcminer-extract

    echo "Linux binary built: dist/linux/btcminer"
}

build_linux_opencl() {
    echo "Building Linux binary with OpenCL..."

    docker build \
        -f Dockerfile \
        --target linux-runtime \
        -t btcminer:linux-opencl \
        .

    echo "Linux OpenCL image built: btcminer:linux-opencl"
}

case "$TARGET" in
    windows)
        build_windows
        ;;
    linux)
        build_linux
        ;;
    linux-opencl)
        build_linux_opencl
        ;;
    all)
        build_windows
        build_linux
        ;;
    *)
        echo "Usage: $0 [windows|linux|linux-opencl|all]"
        exit 1
        ;;
esac

echo "Build complete!"
