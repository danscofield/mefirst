# Configuration Examples

This directory contains example configuration files and response templates for the IMDS Interposer.

## Configuration Files

### config.toml
Basic configuration file demonstrating all available options with comments.

**Use case:** Starting point for creating your own configuration.

**Features:**
- All configuration options documented
- eBPF disabled by default (runs as standard proxy)
- No plugins configured (all requests proxied to real IMDS)

**Run:**
```bash
./imdsinterposer --config-file examples/config.toml
```

### config.yaml
Same as config.toml but in YAML format.

**Use case:** If you prefer YAML over TOML.

**Run:**
```bash
./imdsinterposer --config-file examples/config.yaml
```

### config-with-plugins.toml
Configuration with example interception plugins enabled.

**Use case:** Demonstrates how to intercept specific IMDS paths and return custom responses.

**Features:**
- Static file responses for IAM credentials and instance ID
- Command execution for dynamic hostname
- Shows different pattern types (exact, glob)
- Shows different response sources (file, command)

**Run:**
```bash
./imdsinterposer --config-file examples/config-with-plugins.toml
```

### config-process-aware.toml
Configuration demonstrating process-aware routing features.

**Use case:** Route requests based on process identity (uid, username, executable, cmdline).

**Features:**
- UID-based routing (only specific users)
- Username-based routing
- Executable pattern matching (exact, glob, regex)
- Command line pattern matching
- Host header pattern matching
- Multiple filter combinations

**Run:**
```bash
sudo ./mefirst --config examples/config-process-aware.toml
```

### config-proxy-request-stdin.toml
Configuration demonstrating the proxy_request_stdin feature.

**Use case:** Forward complete HTTP requests with process metadata headers to command stdin.

**Features:**
- HTTP request forwarding to command stdin
- Process metadata header injection (X-Forwarded-Uid, X-Forwarded-Username, etc.)
- HTTP response parsing from command stdout
- Fallback to configured status_code if command doesn't output HTTP format

**Run:**
```bash
./mefirst --config examples/config-proxy-request-stdin.toml
```

### config-optional-ip.toml
Configuration demonstrating IP-agnostic interception mode.

**Use case:** Intercept all outbound traffic on a specific port and filter by Host header.

**Features:**
- Optional IP field (omitted to enable IP-agnostic mode)
- Intercepts all connections to port 80 regardless of destination IP
- Host header pattern matching for routing decisions
- Useful for intercepting traffic to multiple domains
- **Dual-stack support**: Automatically intercepts both IPv4 and IPv6 connections

**Run:**
```bash
sudo ./mefirst --config examples/config-optional-ip.toml
```

**Test:**
```bash
curl -4 google.com:80  # IPv4
curl -6 google.com:80  # IPv6
curl google.com:80     # Auto (tries IPv6 first, falls back to IPv4)
```

### config-ip-specific.toml
Configuration demonstrating IP-specific interception mode.

**Use case:** Intercept only connections to a specific (IP, port) pair.

**Features:**
- Explicit IP field for targeted interception
- Only intercepts connections to 169.254.169.254:80
- Traditional IMDS interception use case

**Run:**
```bash
sudo ./mefirst --config examples/config-ip-specific.toml
```

## Response Templates

The `responses/` directory contains example response files used by the plugin configurations.

### responses/credentials.json
Example IAM credentials response in AWS format.

**Used by:** Plugin intercepting `/latest/meta-data/iam/security-credentials/*`

### responses/instance-id.txt
Example instance ID as plain text.

**Used by:** Plugin intercepting `/latest/meta-data/instance-id`

### responses/role-list.json
Example IAM role list response.

**Used by:** Plugin intercepting `/latest/meta-data/iam/security-credentials/`

## Code Examples

### logging_demo.rs
Demonstrates the logging configuration options.

**Run:**
```bash
# Default (INFO level, text format)
cargo run --example logging_demo

# Debug level
RUST_LOG=debug cargo run --example logging_demo

# JSON format
LOG_FORMAT=json cargo run --example logging_demo

# Trace level with JSON
RUST_LOG=trace LOG_FORMAT=json cargo run --example logging_demo
```

## Configuration Options Reference

### Core Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enable_ebpf` | bool | `false` | Enable eBPF-based transparent redirection |
| `ebpf_program_path` | string | (embedded) | Path to external eBPF program (requires `allow-external-ebpf` feature) |
| `cgroup_path` | string | `/sys/fs/cgroup` | Cgroup path for eBPF attachment |
| `target_address` | string | `169.254.169.254` | Target address to intercept (legacy, use `interception.ip` instead) |
| `target_port` | number | `80` | Target port to intercept (legacy, use `interception.port` instead) |
| `bind_port` | number | `8080` | Proxy bind port (always binds to 127.0.0.1 and ::1) |
| `enable_metrics` | bool | `true` | Enable Prometheus metrics endpoint |
| `metrics_port` | number | `9090` | Metrics endpoint port |

### Interception Configuration

The `[interception]` section configures which connections to intercept:

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `ip` | string | No | Target IP address to intercept. If omitted, intercepts all IPs on the specified port |
| `port` | number | Yes | Target port to intercept |

**IP-Agnostic Mode (omit `ip` field):**
```toml
[interception]
port = 80
```
Intercepts all connections to port 80 regardless of destination IP. Useful for:
- Intercepting traffic to multiple domains
- Filtering by Host header instead of IP
- Capturing all HTTP traffic on a specific port

**IP-Specific Mode (specify `ip` field):**
```toml
[interception]
ip = "169.254.169.254"
port = 80
```
Intercepts only connections to the specific (IP, port) pair. Useful for:
- Targeted interception of specific services (e.g., AWS IMDS)
- Minimizing performance impact by filtering at eBPF level

### Plugin Options

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `pattern` | string | Yes | Path pattern to match |
| `pattern_type` | enum | Yes | Pattern type: `exact`, `glob`, or `regex` |
| `response_source` | object | Yes | Response source configuration |
| `status_code` | number | No (200) | HTTP status code to return |
| `timeout_secs` | number | No | Timeout for command execution |

### Process-Aware Routing Options

Plugins can filter requests based on process identity (requires `enable_ebpf = true`):

| Option | Type | Description |
|--------|------|-------------|
| `uid` | number | Match only requests from processes with this user ID |
| `username` | string | Match only requests from processes with this username |
| `executable_pattern` | object | Match executable path (requires `pattern` and `pattern_type`) |
| `cmdline_pattern` | object | Match command line arguments (requires `pattern` and `pattern_type`) |
| `host_pattern` | object | Match HTTP Host header (requires `pattern` and `pattern_type`) |
| `proxy_request_stdin` | bool | Forward complete HTTP request to command stdin with process metadata headers |

**Multiple filters use AND logic** - all specified filters must match for the plugin to apply.

**Example:**
```toml
[[plugins]]
pattern = "/admin/*"
pattern_type = "glob"
response_source = { type = "file", path = "admin-response.json" }
status_code = 200
uid = 0  # Only root
executable_pattern = { pattern = "/usr/bin/curl", pattern_type = "exact" }
host_pattern = { pattern = "admin.example.com", pattern_type = "exact" }
```

This plugin only applies when:
- Path matches `/admin/*` AND
- Request is from uid 0 (root) AND
- Executable is `/usr/bin/curl` AND
- Host header is `admin.example.com`

### Response Source Types

**File:**
```toml
response_source = { type = "file", path = "path/to/file.json" }
```

**Command:**
```toml
response_source = { type = "command", command = "/usr/bin/hostname", args = [] }
```

## Security Notes

### External eBPF Programs

The `ebpf_program_path` option is **disabled by default** for security reasons. To enable it:

1. Compile with the `allow-external-ebpf` feature:
   ```bash
   cargo build --release --features ebpf,allow-external-ebpf
   ```

2. Only load eBPF programs from trusted sources
3. Verify eBPF program integrity before loading

### Command Execution

Plugins can execute arbitrary commands. Ensure:
- Command paths are absolute and trusted
- Commands are owned by root and not writable by others
- Timeout values are set to prevent hanging processes

## Creating Your Own Configuration

1. Start with `config.toml` as a template
2. Enable eBPF if you need transparent redirection
3. Add plugins for paths you want to intercept
4. Create response files in a secure location
5. Test with `--config-file` flag before deploying

## Testing Configurations

Test your configuration without eBPF:
```bash
./imdsinterposer --config-file your-config.toml
```

Test with eBPF (requires Linux, root, and proper capabilities):
```bash
sudo ./imdsinterposer --config-file your-config.toml --enable-ebpf
```

Check configuration is valid:
```bash
./imdsinterposer --config-file your-config.toml --help
```
