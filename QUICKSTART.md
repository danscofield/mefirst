# Quick Start Guide

## TL;DR

```bash
# On macOS
make install-musl    # One-time setup
make build-all       # Build everything

# Deploy to Linux
scp target/x86_64-unknown-linux-musl/release/imdsinterposer user@host:/usr/local/bin/
```

## Build Commands

| Command | Description |
|---------|-------------|
| `make help` | Show all available targets |
| `make install-musl` | Install musl toolchain (macOS, one-time) |
| `make build` | Build debug binary |
| `make build-release` | Build release binary (recommended) |
| `make build-ebpf` | Build eBPF program (Docker/Finch) |
| `make build-all` | Build everything |
| `make check` | Run cargo check |
| `make test` | Run tests |
| `make clean` | Clean build artifacts |

## File Locations

After building:

```
target/x86_64-unknown-linux-musl/release/imdsinterposer  # Main binary
ebpf/target/bpfel-unknown-none/release/imdsinterposer-ebpf  # eBPF program
```

## Running

```bash
# On Linux server
sudo /usr/local/bin/imdsinterposer \
  --redirect-mode iptables \
  --config-file /etc/imdsinterposer/config.toml
```

## Configuration Example

```toml
# /etc/imdsinterposer/config.toml
redirect_mode = "iptables"
bind_port = 8080

[[plugins]]
pattern = "/latest/meta-data/instance-id"
pattern_type = "exact"
status_code = 200

[plugins.response_source]
type = "command"
command = "/usr/bin/hostname"
args = []
```

See [BUILD.md](BUILD.md) for detailed instructions.
