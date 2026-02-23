#!/usr/bin/env bash
set -euo pipefail

# Build eBPF program using Docker or Finch
# Usage: ./build-ebpf.sh [docker|finch]

CONTAINER_RUNTIME="${1:-finch}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "Building eBPF program using ${CONTAINER_RUNTIME}..."

# Check if container runtime is available
if ! command -v "${CONTAINER_RUNTIME}" >/dev/null 2>&1; then
    echo "Error: ${CONTAINER_RUNTIME} not found"
    exit 1
fi

# Build the Docker image (will use cache if Dockerfile hasn't changed)
echo "Ensuring eBPF builder image is up to date..."
"${CONTAINER_RUNTIME}" build \
    -f "${PROJECT_ROOT}/Dockerfile.ebpf" \
    -t mefirst-ebpf-builder \
    "${PROJECT_ROOT}"

# Build the eBPF program using the image
echo "Building eBPF programs..."
"${CONTAINER_RUNTIME}" run --rm \
    -v "${PROJECT_ROOT}:/workspace" \
    -w /workspace/ebpf \
    mefirst-ebpf-builder \
    bash -c '
        set -euo pipefail
        
        echo "Building cgroup eBPF program..."
        cargo +nightly build --bin mefirst-ebpf --release -Z build-std=core --target bpfel-unknown-none
        
        echo ""
        echo "Building LSM eBPF program..."
        cargo +nightly build --bin mefirst-lsm --release -Z build-std=core --target bpfel-unknown-none
        
        echo ""
        echo "Build artifacts:"
        ls -lh /workspace/target/bpfel-unknown-none/release/mefirst-ebpf
        ls -lh /workspace/target/bpfel-unknown-none/release/mefirst-lsm
    '

echo ""
echo "eBPF binaries built successfully!"
echo "Cgroup program: ${PROJECT_ROOT}/target/bpfel-unknown-none/release/mefirst-ebpf"
echo "LSM program: ${PROJECT_ROOT}/target/bpfel-unknown-none/release/mefirst-lsm"
ls -lh "${PROJECT_ROOT}/target/bpfel-unknown-none/release/mefirst-ebpf"
ls -lh "${PROJECT_ROOT}/target/bpfel-unknown-none/release/mefirst-lsm"
