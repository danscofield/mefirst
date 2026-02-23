# Build Instructions

This document provides detailed build instructions for the mefirst proxy project.

## Overview

The project consists of three components:
1. **Main binary**: Rust application (7.2M, includes embedded eBPF programs)
2. **Cgroup eBPF program**: Kernel-space program for connection redirection (2.6K)
3. **LSM eBPF program**: Kernel-space program for ptrace restrictions (4.2K)

The eBPF programs are embedded in the main binary, so you only need to deploy a single file.

## Prerequisites

### All Platforms

1. **Rust toolchain:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

2. **Container runtime (for eBPF compilation):**
- Install [Finch](https://github.com/runfinch/finch) (recommended for macOS)
- Or install [Docker Desktop](https://www.docker.com/products/docker-desktop/)

### Linux Only (for native builds)

```bash
# Ubuntu/Debian
sudo apt-get install -y \
    build-essential \
    llvm \
    clang \
    libbpf-dev \
    linux-headers-$(uname -r) \
    pkg-config

# Add eBPF target and rust-src
rustup target add bpfel-unknown-none
rustup component add rust-src
rustup toolchain install nightly
```

## Building on macOS

### Quick Build (Recommended)

```bash
# Build everything (main binary + eBPF programs)
./scripts/build-cross-platform.sh
```

This will:
1. Detect you're on macOS
2. Check for Docker/Finch
3. Build both eBPF programs in a Linux container
4. Build the main binary with embedded eBPF bytecode
5. Produce a single binary ready for Linux deployment

Output:
- `target/x86_64-unknown-linux-musl/release/mefirst` (7.2M, includes embedded eBPF)
- `target/bpfel-unknown-none/release/mefirst-ebpf` (2.6K)
- `target/bpfel-unknown-none/release/mefirst-lsm` (4.2K)

### Build eBPF Programs Only

```bash
# Using script (recommended)
./scripts/build-ebpf.sh finch  # or docker

# Or manually
finch build -f Dockerfile.ebpf -t mefirst-ebpf-builder .
finch run --rm -v $(pwd):/workspace -w /workspace/ebpf \
    mefirst-ebpf-builder \
    bash -c '
        cargo +nightly build --bin mefirst-ebpf --release -Z build-std=core --target bpfel-unknown-none
        cargo +nightly build --bin mefirst-lsm --release -Z build-std=core --target bpfel-unknown-none
    '
```

### How It Works

**Main Binary:**
- Builds natively on macOS using musl target
- Cross-compiles to Linux x86_64
- Embeds eBPF bytecode using `include_bytes_aligned!`
- No C compiler required

**eBPF Programs:**
- Build script detects non-Linux platform
- Automatically uses Docker/Finch to compile in Linux container
- Produces architecture-independent eBPF bytecode
- Docker image is cached - only rebuilds when Dockerfile changes

**Result:** Single binary that runs on any Linux system with eBPF support!

## Building on Linux

### Quick Build (Recommended)

```bash
# Build everything natively
./scripts/build-linux-native.sh
```

This will:
1. Check for required Rust components
2. Install nightly toolchain if needed
3. Compile both eBPF programs
4. Embed the eBPF bytecode in the main binary
5. Build the final binary

Output: `target/x86_64-unknown-linux-musl/release/mefirst` (7.2M, includes embedded eBPF)

### Manual Build

```bash
# Add required components
rustup target add x86_64-unknown-linux-musl
rustup target add bpfel-unknown-none
rustup component add rust-src
rustup toolchain install nightly

# Build eBPF programs
cd ebpf
cargo +nightly build --bin mefirst-ebpf --release -Z build-std=core --target bpfel-unknown-none
cargo +nightly build --bin mefirst-lsm --release -Z build-std=core --target bpfel-unknown-none
cd ..

# Build main binary (embeds eBPF bytecode)
cargo build --release --target x86_64-unknown-linux-musl
```

### Build Without eBPF (Not Recommended)

```bash
# Build main binary only (no eBPF support)
cargo build --release --target x86_64-unknown-linux-musl --no-default-features
```

Note: eBPF is mandatory for the proxy to function. This is only useful for testing the build process.

## Deployment

### Single Binary Deployment (Recommended)

The main binary includes embedded eBPF programs, so you only need to deploy one file:

```bash
# Copy to Linux server
scp target/x86_64-unknown-linux-musl/release/mefirst user@host:/usr/local/bin/

# Grant capabilities
ssh user@host 'sudo setcap "cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep" /usr/local/bin/mefirst'

# Run
ssh user@host '/usr/local/bin/mefirst --target-address 169.254.169.254 --target-port 80 --bind-port 8080'
```

### Verify Binary

```bash
# Check it's statically linked
file target/x86_64-unknown-linux-musl/release/mefirst
# Should show: "statically linked"

# Check size
ls -lh target/x86_64-unknown-linux-musl/release/mefirst
# Should show: ~7.2M

# Check eBPF programs are embedded
strings target/x86_64-unknown-linux-musl/release/mefirst | grep -i "redirect_connect"
# Should show eBPF program names
```

## Capability Requirements

The proxy requires specific Linux capabilities to function:

### Required Capabilities

- **CAP_BPF** - Load BPF programs (kernel 5.8+)
- **CAP_NET_ADMIN** - Attach cgroup socket programs
- **CAP_SYS_ADMIN** - Load LSM BPF programs (optional, for ptrace restrictions)

### Optional Capabilities

- **CAP_SYS_PTRACE** - Read process metadata from /proc for other users
- **CAP_DAC_READ_SEARCH** - Bypass /proc permission checks

### Grant Capabilities

```bash
# Full functionality (with LSM support)
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst

# Minimal (cgroup redirection only, no LSM)
sudo setcap 'cap_bpf,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst
```

See [docs/CAPABILITY_CHECKS.md](docs/CAPABILITY_CHECKS.md) for detailed information about capabilities.

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
finch rmi mefirst-ebpf-builder
./scripts/build-ebpf.sh finch

# Or manually build and run
finch build -f Dockerfile.ebpf -t mefirst-ebpf-builder .
finch run --rm -v $(pwd):/workspace -w /workspace/ebpf \
    mefirst-ebpf-builder \
    bash -c '
        cargo +nightly build --bin mefirst-ebpf --release -Z build-std=core --target bpfel-unknown-none
        cargo +nightly build --bin mefirst-lsm --release -Z build-std=core --target bpfel-unknown-none
    '
```

Note: The eBPF bytecode is architecture-independent and will work on any Linux system regardless of where it was built.

### Linking errors on macOS

Make sure you're using the musl target:
```bash
cargo build --target x86_64-unknown-linux-musl --release
```

### Missing nightly toolchain

```bash
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

### Missing bpfel-unknown-none target

```bash
rustup target add bpfel-unknown-none
```

## Development Workflow

### Quick iteration (no eBPF changes)

```bash
# Check code
cargo check

# Run tests
cargo test

# Build for Linux
cargo build --release --target x86_64-unknown-linux-musl
```

### Full build (with eBPF changes)

```bash
# Build eBPF programs
./scripts/build-ebpf.sh

# Build main binary (embeds eBPF)
cargo build --release --target x86_64-unknown-linux-musl

# Or use the all-in-one script
./scripts/build-cross-platform.sh
```

### Testing on Linux

```bash
# Deploy to test server
scp target/x86_64-unknown-linux-musl/release/mefirst test-server:/tmp/

# Grant capabilities
ssh test-server 'sudo setcap "cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep" /tmp/mefirst'

# Run
ssh test-server '/tmp/mefirst --target-address 169.254.169.254 --target-port 80 --bind-port 8080'
```

## Build Targets

| Target | Platform | Output |
|--------|----------|--------|
| `x86_64-unknown-linux-musl` | Linux x86_64 | Static binary (7.2M) |
| `bpfel-unknown-none` | eBPF (little-endian) | eBPF bytecode (2.6K + 4.2K) |

## CI/CD Integration

The build scripts are designed to work in CI/CD pipelines:

```yaml
# Example GitHub Actions
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: x86_64-unknown-linux-musl
          
      - name: Install Docker
        run: |
          sudo apt-get update
          sudo apt-get install -y docker.io
          
      - name: Build
        run: ./scripts/build-cross-platform.sh
        
      - name: Run tests
        run: cargo test
        
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: binaries
          path: |
            target/x86_64-unknown-linux-musl/release/mefirst
            target/bpfel-unknown-none/release/mefirst-ebpf
            target/bpfel-unknown-none/release/mefirst-lsm
```

## Build Artifacts

After a successful build, you'll have:

```
target/x86_64-unknown-linux-musl/release/mefirst  # Main binary (7.2M)
  ├─ Embedded: mefirst-ebpf (2.6K)                # Cgroup eBPF program
  └─ Embedded: mefirst-lsm (4.2K)                 # LSM eBPF program

target/bpfel-unknown-none/release/
  ├─ mefirst-ebpf                                 # Cgroup eBPF program (standalone)
  └─ mefirst-lsm                                  # LSM eBPF program (standalone)
```

The standalone eBPF programs are provided for reference and debugging. The main binary includes them embedded, so you only need to deploy the main binary.

## Performance

The build process is optimized for speed:

- **eBPF programs**: ~3-5 seconds (cached Docker image)
- **Main binary**: ~45-60 seconds (release build)
- **Total**: ~50-65 seconds for a full build

Incremental builds (no eBPF changes) take ~5-10 seconds.

## Next Steps

- Read [QUICKSTART.md](QUICKSTART.md) for quick start guide
- Read [docs/CAPABILITY_CHECKS.md](docs/CAPABILITY_CHECKS.md) for capability requirements
- Read [docs/EBPF_IMPLEMENTATION.md](docs/EBPF_IMPLEMENTATION.md) for eBPF implementation details
