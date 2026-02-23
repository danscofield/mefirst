#!/bin/bash
set -e

echo "Building mefirst proxy (separate artifacts)..."
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_TYPE="${1:---release}"

# Detect platform
OS_TYPE="$(uname -s)"
echo "Platform: $OS_TYPE"
echo ""

# Detect container runtime
if command -v finch >/dev/null 2>&1; then
    CONTAINER_RUNTIME="finch"
elif command -v docker >/dev/null 2>&1; then
    CONTAINER_RUNTIME="docker"
else
    echo "Error: Neither finch nor docker found"
    exit 1
fi

echo "Container runtime: $CONTAINER_RUNTIME"
echo ""

# Step 1: Build eBPF program
echo "Step 1/2: Building eBPF program..."
echo "=========================================="
"${SCRIPT_DIR}/build-ebpf.sh" "${CONTAINER_RUNTIME}"

# Step 2: Build main binary
echo ""
echo "Step 2/2: Building main binary..."
echo "=========================================="
cd "${PROJECT_ROOT}"

if [[ "$OS_TYPE" == "Linux" ]]; then
    # Native Linux build
    if [ "${BUILD_TYPE}" = "--release" ]; then
        cargo build --release --target x86_64-unknown-linux-musl --features ebpf
        BINARY_PATH="target/x86_64-unknown-linux-musl/release/mefirst"
    else
        cargo build --target x86_64-unknown-linux-musl --features ebpf
        BINARY_PATH="target/x86_64-unknown-linux-musl/debug/mefirst"
    fi
else
    # Container build for cross-compilation
    echo "Building in Linux container..."
    $CONTAINER_RUNTIME build -f Dockerfile.ebpf -t mefirst-ebpf-builder . -q
    
    if [ "${BUILD_TYPE}" = "--release" ]; then
        $CONTAINER_RUNTIME run --rm \
            -v "$(pwd)":/workspace \
            -w /workspace \
            mefirst-ebpf-builder \
            cargo build --release --target x86_64-unknown-linux-musl --features ebpf
        BINARY_PATH="target/x86_64-unknown-linux-musl/release/mefirst"
    else
        $CONTAINER_RUNTIME run --rm \
            -v "$(pwd)":/workspace \
            -w /workspace \
            mefirst-ebpf-builder \
            cargo build --target x86_64-unknown-linux-musl --features ebpf
        BINARY_PATH="target/x86_64-unknown-linux-musl/debug/mefirst"
    fi
fi

echo ""
echo "✓ Main binary built: ${BINARY_PATH}"
ls -lh "${BINARY_PATH}"

echo ""
echo "=========================================="
echo "Build Complete!"
echo "=========================================="
echo ""
echo "Artifacts:"
echo "  Main binary: ${BINARY_PATH}"
echo "  Cgroup eBPF program: target/bpfel-unknown-none/release/mefirst-ebpf"
echo "  LSM eBPF program: target/bpfel-unknown-none/release/mefirst-lsm"
echo ""
echo "Deploy to Linux:"
echo "  scp ${BINARY_PATH} user@host:/usr/local/bin/mefirst"
echo "  scp target/bpfel-unknown-none/release/mefirst-ebpf user@host:/usr/local/lib/"
echo "  scp target/bpfel-unknown-none/release/mefirst-lsm user@host:/usr/local/lib/"
echo ""
