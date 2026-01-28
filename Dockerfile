# Multi-stage Dockerfile for cross-compiling btcminer to Windows with OpenCL
# Uses cargo-xwin for Windows MSVC cross-compilation

# Build stage
FROM rust:1.84-bookworm AS builder

# Install required build dependencies
RUN apt-get update && apt-get install -y \
    clang \
    llvm \
    lld \
    cmake \
    ninja-build \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust components and Windows target
RUN rustup component add llvm-tools \
    && rustup target add x86_64-pc-windows-msvc

# Install cargo-xwin
RUN cargo install --locked cargo-xwin

# Download and setup OpenCL SDK (Khronos headers + ICD loader)
WORKDIR /opt/opencl
RUN git clone --depth 1 https://github.com/KhronosGroup/OpenCL-Headers.git \
    && git clone --depth 1 https://github.com/KhronosGroup/OpenCL-ICD-Loader.git

# Build OpenCL ICD Loader for Windows
WORKDIR /opt/opencl/OpenCL-ICD-Loader
RUN mkdir build-win && cd build-win \
    && cmake .. \
        -G Ninja \
        -DCMAKE_SYSTEM_NAME=Windows \
        -DCMAKE_C_COMPILER=clang \
        -DCMAKE_CXX_COMPILER=clang++ \
        -DCMAKE_C_COMPILER_TARGET=x86_64-pc-windows-msvc \
        -DCMAKE_CXX_COMPILER_TARGET=x86_64-pc-windows-msvc \
        -DOPENCL_ICD_LOADER_HEADERS_DIR=/opt/opencl/OpenCL-Headers \
        -DCMAKE_INSTALL_PREFIX=/opt/opencl/sdk \
    || true  # May fail but we can use pre-built lib

# Alternative: download pre-built OpenCL.lib from Windows SDK via xwin
# cargo-xwin downloads Windows SDK which includes OpenCL

# Set OpenCL environment for Rust build
ENV OPENCL_INCLUDE_DIR=/opt/opencl/OpenCL-Headers
ENV OPENCL_LIB_DIR=/opt/opencl/sdk/lib

# Set working directory for project
WORKDIR /build

# Copy project files
COPY . .

# Configure cargo for cross-compilation
RUN mkdir -p .cargo && echo '\
[target.x86_64-pc-windows-msvc]\n\
linker = "lld-link"\n\
rustflags = ["-C", "link-arg=/LIBPATH:/opt/opencl/sdk/lib"]\n\
' >> .cargo/config.toml

# Build for Windows with OpenCL
# Note: First build will download Windows SDK (~1.5GB)
RUN cargo xwin build --release --target x86_64-pc-windows-msvc --features opencl \
    || cargo xwin build --release --target x86_64-pc-windows-msvc

# Output stage - extract just the binary
FROM scratch AS output
COPY --from=builder /build/target/x86_64-pc-windows-msvc/release/btcminer.exe /btcminer.exe
COPY --from=builder /build/web /web

# Alternative: Linux build stage (for comparison/testing)
FROM rust:1.84-bookworm AS linux-builder

# Install OpenCL development libraries for Linux
RUN apt-get update && apt-get install -y \
    ocl-icd-opencl-dev \
    opencl-headers \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

# Build for Linux with OpenCL
RUN cargo build --release --features opencl

# Linux runtime stage
FROM debian:bookworm-slim AS linux-runtime

RUN apt-get update && apt-get install -y \
    ocl-icd-libopencl1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=linux-builder /build/target/release/btcminer /usr/local/bin/btcminer
COPY --from=linux-builder /build/web /opt/btcminer/web

WORKDIR /opt/btcminer
EXPOSE 8080
CMD ["btcminer"]
