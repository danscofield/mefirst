# Design: mefirst - BPF-enabled Intercepting HTTP Proxy

## Technology Stack

- **Language**: Rust (stable)
- **HTTP Server**: `axum` or `hyper`
- **HTTP Client**: `reqwest` (standard HTTP client)
- **eBPF**: `aya` for eBPF program loading and management
- **CLI Parsing**: `clap` v4
- **Config**: `config` crate with TOML/YAML support
- **Async Runtime**: `tokio`
- **Serialization**: `serde` with `serde_json`
- **Logging**: `tracing` + `tracing-subscriber`
- **Metrics**: `prometheus` crate

### eBPF Framework Choice

**Aya** (Selected):
- Pure Rust, no C dependencies
- CO-RE (Compile Once, Run Everywhere) support
- Better Rust integration and ergonomics
- Active development and community
- Currently implemented and working in production

## Module Structure

```
src/
├── main.rs                 # Entry point, CLI, server setup
├── config.rs              # Configuration structures and loading
├── redirect/
│   ├── mod.rs            # eBPF redirection abstraction
│   └── ebpf.rs           # eBPF-based redirection with Aya
├── upstream/
│   ├── mod.rs            # Upstream module exports
│   ├── client.rs         # HTTP client for upstream service
│   └── proxy.rs          # Proxy server logic and request handler
├── plugin/
│   ├── mod.rs            # Plugin trait and exports
│   ├── config.rs         # Plugin configuration structures
│   ├── factory.rs        # Plugin factory for creating plugins
│   ├── file.rs           # File-based response plugin
│   └── command.rs        # Command execution plugin
├── metrics.rs            # Prometheus metrics (separate port)
└── error.rs              # Error types

# eBPF program (separate compilation)
ebpf/
├── Cargo.toml            # eBPF program dependencies
└── src/
    └── main.rs           # cgroup/connect4 BPF program with PID filtering

# Build scripts
scripts/
├── build-ebpf.sh         # Build eBPF program only
├── build-linux-native.sh # Build for Linux with embedded eBPF
└── build-cross-platform.sh # Build using Docker for cross-platform
```

## Key Data Structures

### PluginConfig
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub pattern: String,           // Exact, Glob or regex pattern
    pub pattern_type: PatternType, // Exact, Glob or Regex
    pub response_source: ResponseSource, // File or Command
    pub status_code: u16,          // HTTP status code (default: 200)
    pub timeout_secs: Option<u64>, // Command timeout (for command source)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    Exact,  // Exact path match
    Glob,   // Glob pattern matching
    Regex,  // Regex pattern matching
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ResponseSource {
    File { path: PathBuf },
    Command { command: String, args: Vec<String> },
}
```

### InterceptionPlugin Trait
```rust
#[async_trait]
pub trait InterceptionPlugin: Send + Sync {
    /// Check if this plugin matches the given path
    fn matches(&self, path: &str) -> bool;
    
    /// Get the response for a matched request
    async fn get_response(&self) -> Result<PluginResponse>;
}

pub struct PluginResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}
```

### RedirectMode
```rust
pub struct EbpfRedirector {
    cgroup_path: PathBuf,
    // eBPF program handle
}

pub trait Redirector {
    async fn setup(&self) -> Result<()>;
    async fn teardown(&self) -> Result<()>;
}
```

### ProxyState
```rust
pub struct ProxyState {
    redirector: Arc<EbpfRedirector>,
    plugins: Arc<Vec<Box<dyn InterceptionPlugin>>>,
    config: Arc<Config>,
    metrics: Arc<Metrics>,
    upstream_client: Arc<UpstreamClient>,
}
```

**Note**: Metrics are served on a separate port (default: 9090) to avoid conflicts with the main proxy port.

### eBPF Program Structure
```rust
// ebpf/src/main.rs

/// Map to store the proxy's PID to exclude it from redirection (prevents infinite loops)
#[map]
static PROXY_PID: HashMap<u32, u32> = HashMap::with_max_entries(1, 0);

/// Proxy configuration: stores the proxy bind address and port
/// Key 0 = proxy IP (u32 in little-endian)
/// Key 1 = proxy port (u16 stored as u32)
#[map]
static PROXY_CONFIG: HashMap<u32, u32> = HashMap::with_max_entries(2, 0);

/// Target configuration: stores the target address and port to intercept
/// Key 0 = target IP (u32 in little-endian)
/// Key 1 = target port (u16 stored as u32)
#[map]
static TARGET_CONFIG: HashMap<u32, u32> = HashMap::with_max_entries(2, 0);

#[cgroup_sock_addr(connect4)]
pub fn redirect_connect(ctx: SockAddrContext) -> i32 {
    // Get current process PID
    let pid = (bpf_get_current_pid_tgid() >> 32) as u32;
    
    // Check if this is the proxy's own PID - if so, don't redirect
    if let Some(proxy_pid) = PROXY_PID.get(&0u32) {
        if pid == *proxy_pid {
            return 1; // Allow direct connection to target
        }
    }
    
    // Get target configuration from map
    let target_ip = TARGET_CONFIG.get(&0u32).unwrap_or(0xFEA9FEA9); // Default: 169.254.169.254
    let target_port = TARGET_CONFIG.get(&1u32).unwrap_or(80);
    
    // Get proxy configuration from map
    let proxy_ip = PROXY_CONFIG.get(&0u32).unwrap_or(0x0100007F); // Default: 127.0.0.1
    let proxy_port = PROXY_CONFIG.get(&1u32).unwrap_or(8080);
    
    // Check if destination matches configured target
    if ctx.user_ip4() == target_ip && ctx.user_port() == target_port {
        // Redirect to configured proxy address:port
        ctx.set_user_ip4(proxy_ip);
        ctx.set_user_port(proxy_port);
    }
    1 // Allow connection
}
```

**Key Features**:
- **PID Filtering**: Prevents infinite loops by excluding the proxy's own connections
- **Configurable Target**: Target address and port are passed via TARGET_CONFIG BPF map
- **Configurable Proxy**: Proxy bind address and port are passed via PROXY_CONFIG BPF map
- **Default Values**: Falls back to 169.254.169.254:80 → 127.0.0.1:8080 if not configured
- **Flexible Use Cases**: Can intercept any TCP service, not just AWS IMDS

## Component Design

### 1. Configuration Module (`config.rs`)

Handles configuration from multiple sources with precedence:
1. Default values
2. Configuration file (TOML/YAML)
3. Environment variables
4. Command-line arguments

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub cgroup_path: PathBuf,
    pub plugins: Vec<PluginConfig>,
    pub target_address: String,
    pub target_port: u16,
    pub bind_port: u16,
}
```

### 2. Redirect Module (`redirect/`)

Manages eBPF-based connection redirection:

**eBPF Mode** (`ebpf.rs`):
- Load and attach eBPF program to cgroup
- Handle program lifecycle
- Graceful detachment on shutdown
- Error handling for missing eBPF support

### 3. Plugin Module (`plugin/`)

Implements the interception plugin system:

**Plugin Configuration** (`config.rs`):
- Load plugin configurations from file
- Parse path patterns and response templates
- Validate plugin configurations

**Path Matching** (`matcher.rs`):
- Glob pattern matching
- Regex pattern matching
- First-match routing logic

**Plugin Trait** (`mod.rs`):
- Define InterceptionPlugin trait
- Plugin registry for managing multiple plugins
- Plugin factory for creating plugins from config

### 4. Proxy Module (`proxy/`)

HTTP server and request handling:

**Request Flow**:
1. Receive request from application
2. Check if request matches any plugin pattern
3. If plugin matches:
   - Return plugin response
4. Otherwise:
   - Forward request to target service
   - Return response

### 5. Upstream Client Module (`upstream/`)

HTTP client for communicating with target service:
- Standard HTTP client using reqwest
- Request/response handling

### 6. Metrics Module (`metrics.rs`)

Prometheus metrics:
- Request counters by path and status
- Request latency histograms
- Plugin hit/miss counters

## Design Decisions

1. **No TLS Support**: Keeps implementation simple for local proxying
2. **Pluggable Plugins**: Request interception is optional via trait-based plugin system
3. **Configuration-Driven**: Plugins configured via files, no code changes needed
4. **First-Match Routing**: Plugins evaluated in order, first match wins
5. **eBPF-Only**: Simplified architecture with eBPF-only redirection
6. **Cgroup-Based**: eBPF attachment to cgroups enables per-container/process control

## Error Handling Strategy

Use `thiserror` for structured error types:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Upstream client error: {0}")]
    UpstreamClient(#[from] reqwest::Error),
    
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    #[error("Plugin configuration error: {0}")]
    PluginConfig(String),
    
    #[error("Pattern matching error: {0}")]
    PatternMatch(String),
    
    #[error("eBPF error: {0}")]
    Ebpf(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

## Concurrency Model

- **Async Runtime**: Tokio for async I/O
- **Thread Safety**: Arc + Mutex/RwLock for shared state
- **Background Tasks**: Tokio tasks for background operations
- **Graceful Shutdown**: Tokio CancellationToken for coordinated shutdown

## Performance Considerations

1. **Connection Pooling**: Reuse HTTP connections to target service
2. **Efficient Pattern Matching**: Optimize glob/regex matching for plugin routing
3. **Zero-Copy**: Use `Bytes` for efficient buffer management
4. **eBPF Overhead**: Minimal kernel-level redirection overhead

## Security Considerations

1. **SSRF Prevention**: Restrict proxy destinations to configured target only
2. **Command Execution**: Validate and sanitize command execution in plugins
3. **Input Validation**: Validate all configuration inputs
4. **Least Privilege**: Run with minimal required permissions (CAP_BPF, CAP_NET_ADMIN)
5. **External eBPF Loading**: 
   - By default, only embedded eBPF bytecode is used (compiled into the binary)
   - External eBPF loading is disabled by default and requires the `allow-external-ebpf` feature flag
   - This prevents loading of untrusted eBPF programs that could compromise system security
   - When enabled, the `--ebpf-program-path` flag allows loading external eBPF object files
   - Use external loading only in trusted environments with verified eBPF programs

## Deployment Considerations

1. **eBPF Requirements**: Kernel 4.17+ with BPF support
2. **Permissions**: Requires CAP_BPF and CAP_NET_ADMIN capabilities
3. **Container Support**: Works in containers with proper cgroup access
4. **Systemd Integration**: Provide systemd service file
5. **Health Checks**: Kubernetes/Docker health check support
