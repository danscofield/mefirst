# Quick Start Guide

## TL;DR

```bash
# Build everything
./scripts/build-cross-platform.sh

# Grant capabilities
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' \
  target/x86_64-unknown-linux-musl/release/mefirst

# Run
./target/x86_64-unknown-linux-musl/release/mefirst \
  --target-address 169.254.169.254 \
  --target-port 80 \
  --bind-port 8080
```

## Build Commands

### Using Scripts (Recommended)

```bash
# Cross-platform build (works on macOS and Linux)
./scripts/build-cross-platform.sh

# Native Linux build
./scripts/build-linux-native.sh

# Build eBPF programs only
./scripts/build-ebpf.sh
```

### Using Cargo Directly

```bash
# Build main binary (debug)
cargo build

# Build main binary (release)
cargo build --release

# Run tests
cargo test

# Check code
cargo check
```

## File Locations

After building with `./scripts/build-cross-platform.sh`:

```
target/x86_64-unknown-linux-musl/release/mefirst       # Main binary (7.2M, includes embedded eBPF)
target/bpfel-unknown-none/release/mefirst-ebpf         # Cgroup eBPF program (2.6K)
target/bpfel-unknown-none/release/mefirst-lsm          # LSM eBPF program (4.2K)
```

## Deployment

### Copy to Linux Server

```bash
# Main binary only (eBPF is embedded)
scp target/x86_64-unknown-linux-musl/release/mefirst user@host:/usr/local/bin/
```

### Grant Capabilities

On the Linux server:

```bash
# Full functionality (with LSM support)
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' \
  /usr/local/bin/mefirst

# Minimal (cgroup redirection only, no LSM)
sudo setcap 'cap_bpf,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' \
  /usr/local/bin/mefirst
```

## Running

### Basic Usage

```bash
# Intercept AWS IMDS traffic
./mefirst \
  --target-address 169.254.169.254 \
  --target-port 80 \
  --bind-port 8080
```

### With Configuration File

```bash
# Create config file
cat > config.toml <<EOF
cgroup_path = "/sys/fs/cgroup"

[interception]
ip = "169.254.169.254"
port = 80

bind_port = 8080
enable_metrics = true
metrics_port = 9090

[[plugins]]
pattern = "/latest/meta-data/instance-id"
pattern_type = "exact"
response_source = { type = "file", path = "responses/instance-id.txt" }
status_code = 200
EOF

# Run with config
./mefirst --config-file config.toml
```

### IP-Agnostic Mode (Intercept All Traffic on Port)

```bash
cat > config.toml <<EOF
[interception]
port = 80  # No IP specified - intercepts all traffic to port 80

bind_port = 8080

[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "responses/api.json" }
status_code = 200
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
EOF

./mefirst --config-file config.toml
```

### Process-Aware Routing

```bash
cat > config.toml <<EOF
[interception]
ip = "169.254.169.254"
port = 80

bind_port = 8080

# Only respond to curl from root user
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "responses/admin.json" }
status_code = 200
uid = 0
executable_pattern = { pattern = "/usr/bin/curl", pattern_type = "exact" }
EOF

./mefirst --config-file config.toml
```

### Global Process Metadata Injection

```bash
# Inject process metadata headers into ALL upstream requests
./mefirst \
  --target-port 80 \
  --bind-port 8080 \
  --inject-process-headers true

# Or with config file
cat > config.toml <<EOF
[interception]
port = 80

bind_port = 8080

# Add X-Forwarded-* headers to all upstream requests
inject_process_headers = true
EOF

./mefirst --config-file config.toml

# Test it - upstream will receive X-Forwarded-Uid, X-Forwarded-Username, etc.
curl http://google.com/
```

## Testing

```bash
# Test IPv4 interception
curl -4 http://169.254.169.254/latest/meta-data/instance-id

# Test IPv6 interception
curl -6 http://[::1]:80/latest/meta-data/instance-id

# Check metrics
curl http://localhost:9090/metrics

# Run unit tests
cargo test

# Run integration tests
cargo test --test '*'
```

## Troubleshooting

### Permission Denied

```
Error: eBPF setup failed: Missing required capability: CAP_NET_ADMIN
```

**Solution**: Grant capabilities:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

### LSM Not Loading

```
INFO CAP_SYS_ADMIN not present - LSM BPF programs require CAP_SYS_ADMIN
```

**Solution**: This is informational. To enable LSM support, grant `CAP_SYS_ADMIN`:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

Also check if BPF LSM is enabled in your kernel:
```bash
cat /sys/kernel/security/lsm | grep bpf
```

### Process Metadata Not Available

```
WARN ⚠ CAP_SYS_PTRACE is missing - process metadata retrieval may fail
```

**Solution**: Grant process metadata capabilities:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

### Cgroup Not Found

```
Error: Cgroup path does not exist: "/sys/fs/cgroup"
```

**Solution**: Specify the correct cgroup path:
```bash
./mefirst --cgroup-path /sys/fs/cgroup/unified
```

Or in config file:
```toml
cgroup_path = "/sys/fs/cgroup/unified"
```

## Configuration Examples

See the `examples/` directory for complete configuration examples:
- `config.toml` - Basic configuration
- `config-process-aware.toml` - Process-aware routing
- `config-ip-specific.toml` - IP-specific interception
- `config-optional-ip.toml` - IP-agnostic interception with host filtering
- `config-proxy-request-stdin.toml` - Plugin-specific process metadata injection
- `config-inject-headers.toml` - Global process metadata injection

## Next Steps

- Read [BUILD.md](BUILD.md) for detailed build instructions
- Read [docs/CAPABILITY_CHECKS.md](docs/CAPABILITY_CHECKS.md) for capability requirements
- Read [docs/plugin-interception.md](docs/plugin-interception.md) for plugin system details
- Read [docs/proxy-request-api.md](docs/proxy-request-api.md) for process metadata injection API

## Common Use Cases

### AWS IMDS Interception

```bash
./mefirst \
  --target-address 169.254.169.254 \
  --target-port 80 \
  --bind-port 8080 \
  --config-file imds-config.toml
```

### Multi-Domain Interception

```toml
[interception]
port = 80  # Intercept all traffic to port 80

bind_port = 8080

[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "api1.json" }
status_code = 200
host_pattern = { pattern = "api1.example.com", pattern_type = "exact" }

[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "api2.json" }
status_code = 200
host_pattern = { pattern = "api2.example.com", pattern_type = "exact" }
```

### Process-Based Access Control

```toml
[interception]
ip = "169.254.169.254"
port = 80

bind_port = 8080

# Root user gets full access
[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "full-access.json" }
status_code = 200
uid = 0

# Other users get restricted access
[[plugins]]
pattern = "/latest/meta-data/instance-id"
pattern_type = "exact"
response_source = { type = "file", path = "instance-id.txt" }
status_code = 200
```
