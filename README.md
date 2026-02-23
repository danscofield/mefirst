# mefirst - eBPF-Powered Process-Aware HTTP Proxy

A transparent, high-performance HTTP proxy with eBPF-based connection redirection and process-aware routing. Intercept and customize HTTP traffic with zero application changes, route based on process identity, and enforce security policies at the kernel level.

## Features

- **Transparent eBPF Redirection**: Kernel-level connection interception with cgroup socket hooks
- **Process-Aware Routing**: Route requests based on process identity (uid, username, executable, cmdline)
- **Dual-Stack Support**: Automatic IPv4 and IPv6 interception with separate listeners
- **Host Header Filtering**: Route based on HTTP Host header for multi-domain interception
- **Flexible Interception**: Intercept specific IP:port pairs or all traffic on a port
- **Static & Dynamic Responses**: File-based and command execution response sources
- **Process Metadata Injection**: Forward process information to command-based handlers
- **LSM Security Hooks**: Restrict proxy to read-only ptrace operations (defense-in-depth)
- **Zero Application Changes**: Applications connect to original destination, traffic is transparently redirected
- **High Performance**: Kernel-level redirection with minimal overhead
- **Prometheus Metrics**: Built-in metrics endpoint for monitoring
- **Graceful Shutdown**: Clean resource cleanup on termination

## Use Cases

- **AWS IMDS Interception**: Customize EC2 instance metadata responses per process
- **Process-Based Access Control**: Different responses based on which process made the request
- **Multi-Domain Interception**: Intercept traffic to multiple domains on the same port
- **API Mocking**: Mock external API responses for testing with process context
- **Service Virtualization**: Simulate backend services with process-aware routing
- **Traffic Analysis**: Inspect and log HTTP traffic with full process metadata
- **Security Enforcement**: Restrict API access based on process identity
- **Development**: Test applications against custom responses with process filtering

## Quick Start

### Prerequisites

- **Linux kernel 4.17+** (for cgroup eBPF support)
- **Linux kernel 5.7+** (recommended, for LSM eBPF support)
- **Rust 1.70+** (for building)
- **Linux capabilities**:
  - `CAP_BPF` - Load BPF programs (kernel 5.8+)
  - `CAP_SYS_ADMIN` - Load LSM BPF programs (optional, for ptrace restrictions)
  - `CAP_NET_ADMIN` - Attach cgroup socket programs
  - `CAP_SYS_PTRACE` - Read process metadata from /proc (optional)
  - `CAP_DAC_READ_SEARCH` - Bypass /proc permission checks (optional)

### Build

```bash
# Cross-compile for Linux (from macOS or Linux)
./scripts/build-cross-platform.sh

# Or build natively on Linux
./scripts/build-linux-native.sh
```

This produces:
- Main binary: `target/x86_64-unknown-linux-musl/release/mefirst` (7.2M, includes embedded eBPF)
- Cgroup eBPF program: `target/bpfel-unknown-none/release/mefirst-ebpf` (2.6K)
- LSM eBPF program: `target/bpfel-unknown-none/release/mefirst-lsm` (4.2K)

### Grant Capabilities

```bash
# Full functionality (with LSM support)
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst

# Minimal (cgroup redirection only, no LSM)
sudo setcap 'cap_bpf,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

### Run

#### Basic Proxy Mode (IPv4 + IPv6)

```bash
./mefirst \
  --target-address 169.254.169.254 \
  --target-port 80 \
  --bind-port 8080
```

The proxy automatically binds to both `127.0.0.1:8080` (IPv4) and `[::1]:8080` (IPv6) for complete dual-stack support.

#### With Configuration File

```bash
./mefirst --config-file config.toml
```

## Configuration

### Command-Line Options

```
-c, --config-file <PATH>           Configuration file path (TOML or YAML)
    --cgroup-path <PATH>            Cgroup path for eBPF attachment [default: /sys/fs/cgroup]
-t, --target-address <IP>          Target address to intercept [default: 169.254.169.254]
-T, --target-port <PORT>           Target port to intercept [default: 80]
-p, --bind-port <PORT>             Proxy bind port [default: 8080]
    --enable-metrics <BOOL>         Enable metrics endpoint [default: true]
    --metrics-port <PORT>           Metrics port [default: 9090]
    --inject-process-headers <BOOL> Inject process metadata headers into all upstream requests [default: false]
```

### Configuration File

Create a `config.toml` file to configure interception:

```toml
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
        (process-aware routing)
```

**IPv4 connections:**
- eBPF `connect4` hook intercepts
- Redirects to `127.0.0.1:8080`

**IPv6 connections:**
- eBPF `connect6` hook intercepts
- Redirects to `::1:8080`

### Process Metadata Capture

The proxy captures process metadata for each connection by reading the `/proc` filesystem:
- User ID (uid)
- Username
- Process ID (pid)
- Executable path
- Command line arguments

This metadata is used for routing decisions and included in logs.

### LSM Security Hook

When `CAP_SYS_ADMIN` is available, the proxy loads an LSM (Linux Security Module) eBPF program that restricts the proxy process to read-only ptrace operations. This provides defense-in-depth security by preventing the proxy from attaching to or modifying other processes, even though it has `CAP_SYS_PTRACE` for reading `/proc`.

## Process-Aware Routing

Route requests based on process identity with flexible pattern matching.

### Filter by User

```toml
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "admin.json" }
status_code = 200
uid = 0  # Only root
```

### Filter by Executable

```toml
[[plugins]]
pattern = "/api/*"
pattern_type = "glob"
response_source = { type = "file", path = "api-response.json" }
status_code = 200
executable_pattern = { pattern = "/usr/bin/curl", pattern_type = "exact" }
```

### Filter by Command Line

```toml
[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "test-response.json" }
status_code = 200
cmdline_pattern = { pattern = "*--test*", pattern_type = "glob" }
```

### Filter by Host Header

```toml
[[plugins]]
pattern = "/*"
pattern_type = "glob"
response_source = { type = "file", path = "api-response.json" }
status_code = 200
host_pattern = { pattern = "api.example.com", pattern_type = "exact" }
```

### Combine Multiple Filters

All specified filters must match (AND logic):

```toml
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "admin.json" }
status_code = 200
uid = 0
executable_pattern = { pattern = "/usr/bin/curl", pattern_type = "exact" }
host_pattern = { pattern = "admin.example.com", pattern_type = "exact" }
```

## Response Sources

### Static Files

Return content from a file:

```toml
[[plugins]]
pattern = "/latest/meta-data/instance-id"
pattern_type = "exact"
response_source = { type = "file", path = "responses/instance-id.txt" }
status_code = 200
```

### Command Execution

Execute a shell command and return its stdout:

```toml
[[plugins]]
pattern = "/latest/meta-data/hostname"
pattern_type = "exact"
response_source = { type = "command", command = "/usr/bin/hostname", args = [] }
status_code = 200
```

### Process Metadata Injection

There are two ways to inject process metadata headers into requests:

#### 1. Global Header Injection (All Requests)

Enable `inject_process_headers` to add process metadata headers to ALL upstream requests, including those that don't match any plugin:

```toml
# Inject process headers into all upstream requests
inject_process_headers = true

[interception]
port = 80
```

Or via CLI:
```bash
./mefirst --inject-process-headers true --target-port 80
```

When enabled, the proxy adds these headers to every upstream request:
- `X-Forwarded-Uid`: User ID
- `X-Forwarded-Username`: Username
- `X-Forwarded-Pid`: Process ID
- `X-Forwarded-Process-Name`: Executable path
- `X-Forwarded-Process-Args`: Command line arguments

This is useful when your upstream service needs to know which process made each request, regardless of the URL path.

#### 2. Plugin-Specific Injection (Matched Requests Only)

Use `proxy_request_stdin = true` in a plugin to forward the full HTTP request with injected headers to a command handler:

```toml
[[plugins]]
pattern = "/api/*"
pattern_type = "glob"
response_source = { type = "command", command = "/usr/local/bin/api-handler.sh", args = [] }
status_code = 200
proxy_request_stdin = true  # Inject headers for this plugin only
```

When `proxy_request_stdin = true`, the same headers are added, but only for requests matching this plugin pattern. The serialized HTTP request (with headers) is sent to the command's stdin.

#### Comparison

| Feature | `inject_process_headers` | `proxy_request_stdin` |
|---------|-------------------------|----------------------|
| Scope | All upstream requests | Plugin-matched requests only |
| Destination | Upstream service | Command handler stdin |
| Use case | Upstream needs process context | Custom command-based routing |

## Pattern Types

- **exact**: Exact string match
- **glob**: Glob pattern matching (supports `*` wildcard)
- **regex**: Regular expression matching

## Testing

```bash
# Test IPv4
curl -4 http://169.254.169.254/latest/meta-data/instance-id

# Test IPv6
curl -6 http://[::1]:80/latest/meta-data/instance-id

# Run unit tests
cargo test

# Run integration tests
cargo test --test '*'
```

## Security

### Localhost-Only Binding

The proxy always binds to localhost addresses (`127.0.0.1` and `::1`), ensuring it never accepts remote connections. This is hardcoded for security.

### Capability Management

The proxy checks for required capabilities at startup:
- **CAP_BPF** or **CAP_SYS_ADMIN**: Required for loading eBPF programs
- **CAP_NET_ADMIN**: Required for attaching cgroup socket programs
- **CAP_SYS_ADMIN**: Required for loading LSM eBPF programs (optional)
- **CAP_SYS_PTRACE**: Optional, for reading process metadata from other users
- **CAP_DAC_READ_SEARCH**: Optional, for bypassing /proc permission checks

After loading eBPF programs, the proxy drops `CAP_BPF` and `CAP_SYS_ADMIN` to minimize privilege.

### LSM Hook

When `CAP_SYS_ADMIN` is available, the proxy loads an LSM eBPF program that restricts the proxy process to read-only ptrace operations. This prevents the proxy from:
- Attaching to other processes with `PTRACE_ATTACH`
- Modifying memory or registers of other processes
- Using ptrace for anything other than reading `/proc`

The LSM hook provides defense-in-depth security and is optional (proxy continues without it if unavailable).

### eBPF Program Loading

By default, only the embedded eBPF programs are loaded. External eBPF loading is disabled unless compiled with the `allow-external-ebpf` feature.

### Command Execution

Plugins can execute commands. Ensure:
- Command paths are absolute and trusted
- Commands are owned by root and not writable by others
- Timeout values are set to prevent hanging processes

## Metrics

Prometheus-compatible metrics are exposed on `http://localhost:9090/metrics` (configurable):

- `http_requests_total`: Total HTTP requests processed
- `http_request_duration_seconds`: Request duration histogram
- `ebpf_redirections_total`: Total eBPF redirections
- `process_metadata_retrievals_total`: Process metadata retrieval attempts
- `process_metadata_failures_total`: Failed process metadata retrievals

## Troubleshooting

### eBPF program load failed

```
Error: eBPF setup failed: Missing required capability: CAP_NET_ADMIN
```

Grant the required capabilities:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

### LSM hook not loading

```
INFO CAP_SYS_ADMIN not present - LSM BPF programs require CAP_SYS_ADMIN
```

This is informational. The proxy continues without LSM support. To enable LSM:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

Also ensure your kernel has LSM BPF enabled:
```bash
cat /sys/kernel/security/lsm | grep bpf
```

If not present, add `lsm=...,bpf` to kernel boot parameters.

### Process metadata not available

```
WARN ⚠ CAP_SYS_PTRACE is missing - process metadata retrieval may fail
```

Grant `CAP_SYS_PTRACE` and `CAP_DAC_READ_SEARCH`:
```bash
sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' ./mefirst
```

## Examples

See the `examples/` directory for complete configuration examples:
- `config.toml` - Basic configuration
- `config-process-aware.toml` - Process-aware routing
- `config-ip-specific.toml` - IP-specific interception
- `config-optional-ip.toml` - IP-agnostic interception with host filtering
- `config-proxy-request-stdin.toml` - Plugin-specific process metadata injection
- `config-inject-headers.toml` - Global process metadata injection

## Documentation

- [BUILD.md](BUILD.md) - Detailed build instructions
- [QUICKSTART.md](QUICKSTART.md) - Quick start guide
- [docs/CAPABILITY_CHECKS.md](docs/CAPABILITY_CHECKS.md) - Capability requirements and LSM hooks
- [docs/EBPF_IMPLEMENTATION.md](docs/EBPF_IMPLEMENTATION.md) - eBPF implementation details
- [docs/LOGGING.md](docs/LOGGING.md) - Logging configuration
- [docs/plugin-interception.md](docs/plugin-interception.md) - Plugin system details
- [docs/proxy-request-api.md](docs/proxy-request-api.md) - Process metadata injection API

## License

See LICENSE file for details.
