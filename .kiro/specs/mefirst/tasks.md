# Implementation Tasks: mefirst

## Overview

This document outlines the implementation tasks for mefirst, a BPF-enabled intercepting HTTP proxy. The project implements a transparent proxy for HTTP services with a flexible plugin-based interception system.

### Key Features
- **Transparent Redirection**: eBPF-based connection interception
- **Plugin-Based Interception**: Configure custom responses for specific paths
- **Static & Dynamic Responses**: Support for both file-based and command execution responses
- **Observability**: Prometheus metrics and structured logging

## Implementation Status

All phases completed ✅

---

## Phase 1: Core Infrastructure ✅

- [x] 1.1 Project setup with Cargo workspace
  - Workspace configured with main crate and ebpf crate

- [x] 1.2 Configuration module with CLI and file support
  - CLI parsing with clap
  - TOML/YAML config file support
  - Environment variable overrides
  - Plugin configuration structures

- [x] 1.3 Error types and Result aliases
  - Structured error types with thiserror
  - InterposerError enum with context

- [x] 1.4 Logging setup with tracing
  - Configurable log levels and formats
  - Structured logging with tracing

- [x] 1.5 Basic HTTP server with axum
  - Async HTTP server with tokio
  - Request handler infrastructure

## Phase 2: eBPF Redirection ✅

- [x] 2.1 eBPF program for cgroup/connect4 redirection
  - eBPF program written in ebpf/src/main.rs
  - Redirects configured target address:port → configured proxy address:port

- [x] 2.2 eBPF program compilation and embedding
  - Compile eBPF program to bytecode
  - Embed bytecode in main binary using include_bytes_aligned!
  - Build script for automatic compilation

- [x] 2.3 eBPF loader and cgroup attachment
  - Load eBPF program using aya
  - Attach to cgroup path from config
  - Store program handle for cleanup

- [x] 2.4 Graceful eBPF program detachment
  - Detach program on shutdown
  - Clean up resources properly

- [x] 2.5 Error handling for missing eBPF support
  - Check kernel version (4.17+ required)
  - Check BPF filesystem availability
  - Check permissions (CAP_BPF, CAP_NET_ADMIN)
  - Clear error messages for users

- [x] 2.6 Configurable proxy target in eBPF program
  - Added PROXY_CONFIG BPF map to store proxy IP and port
  - Pass bind_address and bind_port from config to eBPF program
  - eBPF program reads proxy target from map (defaults to 127.0.0.1:8080)
  - IPv4 address parsing helper for little-endian conversion

## Phase 3: Upstream Client ✅

- [x] 3.1 HTTP client (standard reqwest client)
  - Standard HTTP client without custom port binding
  - Connection pooling and reuse

- [x] 3.2 Generic request proxying
  - Support for all HTTP methods (GET, PUT, POST, DELETE, etc.)
  - Header forwarding with hop-by-hop filtering
  - Binary request/response body support
  - Full test coverage

## Phase 4: Interception Plugin System ✅

- [x] 4.1 InterceptionPlugin trait definition
  - Trait with matches() and get_response() methods
  - PluginResponse structure
  - Async trait support

- [x] 4.2 Plugin configuration structure
  - [x] 4.2.1 ResponseSource enum (File and Command variants)
  - [x] 4.2.2 PatternType enum (Exact, Glob, Regex)
  - [x] 4.2.3 PluginConfig struct with validation
  - Configuration validation on startup

- [x] 4.3 Static file response loading
  - FilePlugin implementation
  - Load JSON/YAML/text files
  - Custom status codes and headers
  - Comprehensive tests

- [x] 4.4 Command execution response handler
  - [x] 4.4.1 Command execution with timeout
  - [x] 4.4.2 Stdout capture and error handling
  - [x] 4.4.3 Command argument parsing
  - CommandPlugin implementation
  - Configurable timeout
  - Comprehensive tests

- [x] 4.5 Path matching and routing logic
  - [x] 4.5.1 Exact path matching
  - [x] 4.5.2 Glob pattern matching
  - [x] 4.5.3 Regex pattern matching
  - PathMatcher implementation
  - First-match routing in registry
  - Comprehensive tests

- [x] 4.6 Plugin registry and factory
  - PluginRegistry for managing multiple plugins
  - PluginFactory for creating plugins from config
  - First-match evaluation order
  - Comprehensive tests

## Phase 5: Proxy Logic ✅

- [x] 5.1 Request routing and filtering
  - Basic request handler implemented
  - Extract method, path, headers, body

- [x] 5.2 Plugin-based request interception
  - Check if path matches any plugin pattern
  - Return plugin response if matched
  - Log plugin hits

- [x] 5.3 Passthrough mode for non-intercepted requests
  - Forward unmatched requests to target service
  - Use proxy_request_full for all HTTP methods

- [x] 5.4 Header manipulation (hop-by-hop removal)
  - Automatic hop-by-hop header filtering
  - Implemented in UpstreamClient

- [x] 5.5 Response forwarding
  - Forward status code, headers, and body
  - Binary body support

- [x] 5.6 Error handling and status codes
  - Proper error responses
  - Gateway errors for upstream failures

## Phase 6: Observability & Operations ✅

- [x] 6.1 Prometheus metrics endpoint
  - [x] 6.1.1 Metrics structure defined (requests_total, request_duration, etc.)
  - [x] 6.1.2 Integrate metrics into request handler
  - [x] 6.1.3 Add plugin hit/miss counters
  - [x] 6.1.4 Expose /metrics endpoint

- [x] 6.2 Graceful shutdown handling
  - [x] 6.2.1 Signal handling (SIGTERM/SIGINT) implemented
  - [x] 6.2.2 In-flight request draining (axum handles this)
  - [x] 6.2.3 Complete resource cleanup (eBPF programs, connections)

## Phase 7: Testing & Documentation ✅

- [x] 7.1 Unit tests for all modules
  - [x] 7.1.1 Configuration parsing and validation tests
  - [x] 7.1.2 Plugin pattern matching tests
  - [x] 7.1.3 Command execution tests
  - [x] 7.1.4 Error handling tests
  - Over 100 unit tests passing

- [x] 7.2 Integration tests
  - [x] 7.2.1 Config file loading tests
  - [x] 7.2.2 Proxy request tests

- [x] 7.3 Build scripts
  - [x] 7.3.1 Makefile with build targets
  - [x] 7.3.2 eBPF build script
  - [x] 7.3.3 Docker build for eBPF compilation

- [x] 7.4 README with usage examples
  - [x] 7.4.1 Quick start guide
  - [x] 7.4.2 Configuration examples (file and command plugins)
  - [x] 7.4.3 Troubleshooting guide (eBPF requirements, permissions)
  - [x] 7.4.4 Architecture diagrams

- [x] 7.5 Configuration examples
  - [x] 7.5.1 Basic passthrough config
  - [x] 7.5.2 Static file plugin config
  - [x] 7.5.3 Command execution plugin config
  - [x] 7.5.4 External API plugin config

- [x] 7.6 Additional documentation
  - [x] Build instructions
  - [x] Quick start guide
  - [x] Logging documentation

---

## File Locations

**Core Implementation:**
- Configuration: `src/config.rs`
- Error types: `src/error.rs`
- Logging: `src/logging.rs`
- Main entry: `src/main.rs`

**Upstream Client:**
- Client: `src/upstream/client.rs`
- Tests: `src/upstream/*.test.rs`

**Plugin System:**
- Trait: `src/plugin/mod.rs`
- Config: `src/plugin/config.rs`
- Matcher: `src/plugin/matcher.rs`
- File plugin: `src/plugin/file.rs`
- Command plugin: `src/plugin/command.rs`
- Factory: `src/plugin/factory.rs`

**Proxy:**
- Server: `src/proxy/mod.rs`
- Handler: `src/proxy/handler.rs`

**Redirection:**
- eBPF: `src/redirect/ebpf.rs`
- Mode enum: `src/redirect/mod.rs`
- eBPF program: `ebpf/src/main.rs`

**Tests:**
- Integration: `tests/*.rs`
- Unit: `src/**/*.rs` (inline)

**Documentation:**
- README: `README.md`
- Build: `BUILD.md`
- Quick start: `QUICKSTART.md`

**Examples:**
- Basic config: `examples/config.toml`
- With plugins: `examples/config-with-plugins.toml`
- Response files: `examples/responses/*.json`

## Build Commands

```bash
# Build main application
cargo build --release

# Build eBPF program
cd ebpf && cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Run with config
cargo run -- --config-file examples/config-with-plugins.toml
```

## Test Commands

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test '*'

# Specific module
cargo test --lib plugin::

# With output
cargo test -- --nocapture

# Single test
cargo test test_name
```
