# Build Instructions

This document provides detailed build instructions for the IMDS Interposer project.

## Overview

The project consists of two components:
1. **Main binary**: Rust application (can be cross-compiled to musl for static linking)
2. **eBPF program**: Kernel-space program (requires Linux to build)

## Building on macOS

### Prerequisites

1. **Rust toolchain:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. **Container runtime (for eBPF):**
- Install [Finch](https://github.com/runfinch/finch) (recommended for macOS)
- Or install [Docker Desktop](https://www.docker.com/products/docker-desktop/)

Note: The main binary builds directly on macOS. When building with eBPF support (`--features ebpf`), the build script automatically uses Docker/Finch to compile the eBPF program in a Linux container.

### Build Commands

#### Build Main Binary Only (No eBPF)

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

Output: `target/release/imdsinterposer`

#### Build with Embedded eBPF (Recommended)

```bash
# Using the helper script (handles everything automatically)
./scripts/build-cross-platform.sh
```

This will:
1. Detect you're on macOS
2. Check for Docker/Finch
3. Build eBPF program in container
4. Embed bytecode in main binary
5. Produce a single binary ready for Linux deployment

Output: `target/release/imdsinterposer` (with embedded eBPF)

#### Build eBPF Program Separately

```bash
# Using Makefile
make build-ebpf

# Or using script directly
./scripts/build-ebpf.sh finch  # or docker
```

Output: `ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf`

### How It Works

**Main Binary:**
- Builds natively on macOS (no cross-compilation needed)
- Pure Rust dependencies
- No C compiler required

**eBPF Program (with `--features ebpf`):**
- Build script (`build.rs`) detects non-Linux platform
- Automatically uses Docker/Finch to compile eBPF in container
- Uses native architecture (ARM64 on Apple Silicon, x86_64 on Intel)
- Produces architecture-independent eBPF bytecode
- Embeds bytecode in main binary using `include_bytes!`
- Docker image is cached - only rebuilds when Dockerfile changes

**Result:** Single binary that runs on any Linux system with eBPF support!

## Building on Linux

### Prerequisites

```bash
# Ubuntu/Debian
sudo apt-get install -y \
    build-essential \
    llvm \
    clang \
    libbpf-dev \
    linux-headers-$(uname -r) \
    pkg-config

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add eBPF target and rust-src
rustup target add bpfel-unknown-none
rustup component add rust-src
```

### Build Commands

#### Standard Build (No eBPF)

```bash
# Build main binary
cargo build --release
```

#### Build with Embedded eBPF

```bash
# Using the helper script (recommended)
./scripts/build-linux-native.sh

# Or manually
rustup target add bpfel-unknown-none
rustup component add rust-src
cargo +nightly build --release --features ebpf
```

The build script will:
1. Check for required Rust components
2. Install nightly toolchain if needed
3. Compile the eBPF program
4. Embed the eBPF bytecode in the main binary
5. Build the final binary with eBPF support

Output: `target/release/imdsinterposer` (with embedded eBPF)

#### Build eBPF Program Separately

```bash
cd ebpf
cargo +nightly build --release -Z build-std=core --target bpfel-unknown-none
```

Output: `ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf`

## Deployment

### eBPF Deployment Options

The service supports two eBPF deployment modes:

#### 1. Embedded eBPF (Recommended)

Build with the `ebpf` feature to embed the eBPF bytecode in the binary:

```bash
# On any platform (uses Docker/Finch on macOS)
./scripts/build-cross-platform.sh

# Or manually
cargo +nightly build --release --features ebpf
```

**Advantages:**
- Single binary deployment
- No separate eBPF file to manage
- Simpler configuration

**Usage:**
```bash
# Copy single binary
scp target/release/imdsinterposer user@host:/usr/local/bin/

# Run with eBPF enabled
./imdsinterposer --enable-ebpf
```

#### 2. External eBPF File (Requires allow-external-ebpf Feature)

**Security Note:** Loading external eBPF programs is disabled by default. To enable this feature, you must compile with the `allow-external-ebpf` feature flag.

Build with the `allow-external-ebpf` feature to load eBPF programs from files:

```bash
# Build main binary with external eBPF support
cargo build --release --features ebpf,allow-external-ebpf

# Build eBPF program separately
cd ebpf && cargo +nightly build --release -Z build-std=core --target bpfel-unknown-none
```

**Advantages:**
- Can update eBPF program without recompiling main binary
- Useful for testing different eBPF programs

**Security Warning:**
- Only enable `allow-external-ebpf` if you need to load custom eBPF programs
- External eBPF programs run in kernel space and can compromise system security
- Only load eBPF programs from trusted sources

**Usage:**
```bash
# Copy both files
scp target/release/imdsinterposer user@host:/usr/local/bin/
scp ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf user@host:/usr/local/lib/

# Run with external eBPF file (only works if compiled with allow-external-ebpf)
./imdsinterposer --enable-ebpf --ebpf-program-path /usr/local/lib/imdsinterposer-ebpf
```

### Copy to Linux Server

```bash
# Main binary
scp target/x86_64-unknown-linux-musl/release/imdsinterposer user@host:/usr/local/bin/

# eBPF program
scp ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf user@host:/usr/local/lib/
```

### Verify Binary

```bash
# Check it's statically linked
file target/x86_64-unknown-linux-musl/release/imdsinterposer
# Should show: "statically linked"

# Check size
ls -lh target/x86_64-unknown-linux-musl/release/imdsinterposer
```

## Troubleshooting

### Finch/Docker not working

```bash
# Check Finch
finch version

# Check Docker
docker version

# If neither works, install one:
# Finch: https://github.com/runfinch/finch#installation
# Docker: https://www.docker.com/products/docker-desktop/
```

### eBPF build fails

The eBPF build uses native architecture (no emulation), so it should work reliably on both ARM64 and x86_64 hosts. If you encounter issues:

```bash
# Clean and rebuild the Docker image
finch rmi imdsinterposer-ebpf-builder
./scripts/build-ebpf.sh finch

# Or manually build and run
finch build -f Dockerfile.ebpf -t imdsinterposer-ebpf-builder .
finch run --rm -v $(pwd):/workspace -w /workspace/ebpf \
    imdsinterposer-ebpf-builder \
    cargo +nightly build --release -Z build-std=core --target bpfel-unknown-none
```

Note: The eBPF bytecode is architecture-independent and will work on any Linux system regardless of where it was built.

### Linking errors on macOS

Make sure you're using the musl target:
```bash
cargo build --target x86_64-unknown-linux-musl --release
```

## Development Workflow

### Quick iteration (no eBPF)

```bash
# Check code
make check

# Run tests (native, not musl)
make test

# Build for Linux
make build-release
```

### Full build (with eBPF)

```bash
# Build everything
make build-all

# Deploy to test server
scp target/x86_64-unknown-linux-musl/release/imdsinterposer test-server:/tmp/
ssh test-server 'sudo /tmp/imdsinterposer --help'
```

## Build Targets

| Target | Platform | Output |
|--------|----------|--------|
| `x86_64-unknown-linux-musl` | Linux x86_64 | Static binary |
| `aarch64-unknown-linux-musl` | Linux ARM64 | Static binary |
| `bpfel-unknown-none` | eBPF (little-endian) | eBPF bytecode |

## CI/CD Integration

The build scripts are designed to work in CI/CD pipelines:

```yaml
# Example GitHub Actions
- name: Add musl target
  run: rustup target add x86_64-unknown-linux-musl

- name: Build
  run: make build-all

- name: Upload artifacts
  uses: actions/upload-artifact@v3
  with:
    name: binaries
    path: |
      target/x86_64-unknown-linux-musl/release/imdsinterposer
      ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf
```
