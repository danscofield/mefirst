# mefirst - BPF-enabled Intercepting HTTP Proxy

A transparent, high-performance HTTP proxy with eBPF-based connection redirection and plugin-based request interception. Intercept and customize HTTP traffic to any destination with zero application changes.

## Features

- **Transparent Redirection**: eBPF-based connection interception at the kernel level
- **Dual-Stack Support**: Automatic IPv4 and IPv6 interception with separate listeners
- **Process-Aware Routing**: Route requests based on process identity (uid, username, executable, cmdline)
- **Configurable Targets**: Intercept traffic to any IP:port combination or all traffic on a port
- **Path-Based Interception**: Configure custom responses for specific URL paths
- **Host Header Filtering**: Route based on HTTP Host header for multi-domain interception
- **Static & Dynamic Responses**: Support for both file-based and command execution responses
- **Zero Application Changes**: Applications connect to original destination, traffic is transparently redirected
- **High Performance**: Kernel-level redirection with minimal overhead
- **Metrics**: Prometheus-compatible metrics endpoint
- **Graceful Shutdown**: Clean resource cleanup on termination

## Use Cases

- **AWS IMDS Interception**: Customize EC2 instance metadata responses
- **Multi-Domain Interception**: Intercept traffic to multiple domains on the same port
- **Process-Based Access Control**: Route requests based on which process made them
- **API Mocking**: Mock external API responses for testing
- **Service Virtualization**: Simulate backend services in development
- **Traffic Analysis**: Inspect and log HTTP traffic with process metadata
- **A/B Testing**: Route traffic to different backends based on rules
- **Development**: Test applications against custom responses

## Quick Start

### Prerequisites

- Rust 1.70+ (for building)
- Linux kernel 4.17+ (for eBPF support)
- Root/sudo access (for eBPF attachment)
- CAP_BPF and CAP_NET_ADMIN capabilities

### Build

```bash
# Cross-compile for Linux (from macOS or Linux)
./scripts/build-cross-platform.sh

# Or build natively on Linux
./scripts/build-linux-native.sh
```

### Run

#### Basic Proxy Mode (IPv4 + IPv6)

```bash
sudo ./target/release/mefirst \
  --enable-ebpf \
  --target-address 169.254.169.254 \
  --target-port 80 \
  --bind-port 8080
```

The proxy automatically binds to both `127.0.0.1:8080` (IPv4) and `[::1]:8080` (IPv6) for complete dual-stack support.

#### With Configuration File

```bash
sudo ./target/release/mefirst --config-file config.toml
```

## Configuration

### Command-Line Options

```
--config-file <PATH>          Configuration file path (TOML or YAML)
--enable-ebpf                 Enable eBPF-based transparent redirection
--cgroup-path <PATH>          Cgroup path for eBPF attachment [default: /sys/fs/cgroup]
--target-address <IP>         Target address to intercept [default: 169.254.169.254]
--target-port <PORT>          Target port to intercept [default: 80]
--bind-port <PORT>            Proxy bind port [default: 8080]
--enable-metrics <BOOL>       Enable metrics endpoint [default: true]
--metrics-port <PORT>         Metrics port [default: 9090]
```

### Configuration File

Create a `config.toml` file to configure interception:

```toml
# Enable eBPF-based transparent redirection
enable_ebpf = true

# Cgroup path for eBPF attachment
cgroup_path = "/sys/fs/cgroup"

# Connection interception configuration
[interception]
port = 80
# ip = "169.254.169.254"  # Optional: omit for IP-agnostic mode

# Proxy bind port (always binds to 127.0.0.1 and ::1)
bind_port = 8080

# Metrics configuration
enable_metrics = true
metrics_port = 9090

# Interception plugins
[[plugins]]
pattern = "/api/*"
pattern_type = "glob"
response_source = { type = "file", path = "responses/api-response.json" }
status_code = 200
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

### IP-Agnostic vs IP-Specific Interception

**IP-Agnostic Mode** (omit `ip` field):
```toml
[interception]
port = 80
```
Intercepts all connections to port 80 regardless of destination IP. Use with `host_pattern` to filter by domain.

**IP-Specific Mode** (specify `ip` field):
```toml
[interception]
ip = "169.254.169.254"
port = 80
```
Intercepts only connections to the specific (IP, port) pair.

## Architecture

### Dual-Stack Proxy

The proxy automatically creates two separate listeners:
- **IPv4**: `127.0.0.1:8080` - handles IPv4 connections
- **IPv6**: `[::1]:8080` - handles IPv6 connections

Both listeners run concurrently, ensuring full dual-stack support without exposing the proxy to remote connections.

### eBPF Redirection Flow

```
Application → eBPF Hook → Proxy (localhost) → Upstream
                ↓
        Plugin Interception
        (custom responses)
```

**IPv4 connections:**
- eBPF `connect4` hook intercepts
- Redirects to `127.0.0.1:8080`

**IPv6 connections:**
- eBPF `connect6` hook intercepts
- Redirects to `::1:8080`

### Process Metadata Capture

When eBPF is enabled, the proxy captures process metadata for each connection:
- User ID (uid)
- Username
- Process ID (pid)
- Executable path
- Command line arguments

This metadata can be used for routing decisions and is included in logs.

## Interception Plugins

Plugins allow you to intercept specific paths and return custom responses.

### Response Sources

1. **Static Files**: Return content from a file
2. **Command Execution**: Execute a shell command and return its stdout

### Pattern Types

- **exact**: Exact path match
- **glob**: Glob pattern matching (supports `*` wildcard)
- **regex**: Regular expression matching

### Process-Aware Routing

Filter requests based on process identity (requires `enable_ebpf = true`):

```toml
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "admin.json" }
status_code = 200
uid = 0  # Only root
executable_pattern = { pattern = "/usr/bin/curl", pattern_type = "exact" }
host_pattern = { pattern = "admin.example.com", pattern_type = "exact" }
```

### Host Header Filtering

Route based on HTTP Host header:

```toml
[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "api-response.json" }
status_code = 200
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

## Testing

```bash
# Test IPv4
curl -4 google.com:80

# Test IPv6
curl -6 google.com:80

# Test auto (tries IPv6 first)
curl google.com:80

# Run unit tests
cargo test
```

## Security

### Localhost-Only Binding

The proxy always binds to localhost addresses (`127.0.0.1` and `::1`), ensuring it never accepts remote connections. This is hardcoded for security.

### eBPF Program Loading

By default, only the embedded eBPF program is loaded. External eBPF loading is disabled unless compiled with the `allow-external-ebpf` feature.

### Command Execution

Plugins can execute commands. Ensure:
- Command paths are absolute and trusted
- Commands are owned by root and not writable by others
- Timeout values are set to prevent hanging processes

## License

See LICENSE file for details.
