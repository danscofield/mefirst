#!/bin/bash
set -e

echo "Building mefirst proxy with eBPF support..."
echo ""

# Check if rustup is installed
if ! command -v rustup &> /dev/null; then
    echo "Error: rustup is not installed"
    echo "Please install rustup from https://rustup.rs/"
    exit 1
fi

# Detect platform
OS_TYPE="$(uname -s)"
echo "Detected platform: $OS_TYPE"
echo ""

if [[ "$OS_TYPE" == "Linux" ]]; then
    echo "Building natively on Linux..."
    echo ""
    
    # Install required components
    echo "Checking required Rust components..."
    
    # Install nightly toolchain if not present
    if ! rustup toolchain list | grep -q nightly; then
        echo "Installing nightly toolchain..."
        rustup toolchain install nightly
    fi
    
    # Install bpfel-unknown-none target
    echo "Installing eBPF target..."
    rustup target add bpfel-unknown-none
    
    # Install rust-src for building std
    echo "Installing rust-src component..."
    rustup component add rust-src
    
    echo ""
    echo "Building eBPF program and main binary..."
    echo ""
    
    # Build with eBPF feature
    cargo +nightly build --release --features ebpf
    
    echo ""
    echo "Build complete!"
    echo "Binary location: target/release/mefirst"
    echo ""
    echo "To run with eBPF enabled:"
    echo "  sudo setcap 'cap_bpf,cap_net_admin=+ep' target/release/mefirst"
    echo "  ./target/release/mefirst --enable-ebpf"
    
else
    echo "Error: Embedded eBPF builds are only supported on Linux"
    echo ""
    echo "You are running on $OS_TYPE. For cross-platform builds, use:"
    echo "  ./scripts/build-cross-platform.sh"
    echo ""
    echo "This will create:"
    echo "  - Main binary: target/x86_64-unknown-linux-musl/release/mefirst"
    echo "  - eBPF program: target/bpfel-unknown-none/release/mefirst-ebpf"
    echo ""
    echo "Deploy both files to your Linux server and use --ebpf-program-path"
    echo "to load the eBPF program from the external file."
    echo ""
    exit 1
fi
