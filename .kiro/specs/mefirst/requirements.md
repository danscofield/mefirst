# Requirements: mefirst - BPF-enabled Intercepting HTTP Proxy

## Overview

mefirst is a general-purpose HTTP proxy that uses eBPF to transparently intercept connections to a configured target service and optionally replace responses for specific paths with custom content. This allows flexible HTTP response customization through a configuration-driven plugin system.

## Architecture

### eBPF-based Transparent Redirection
```
Application → eBPF cgroup/connect4 → Proxy (port 8080) → Target Service
              (transparent redirect)        ↓
                                    Plugin Interception
                                    (configurable responses)
```

The eBPF approach provides:
- Transparent connection redirection at the kernel level
- No iptables rules or management needed
- No source port pool management required
- No special port range configuration
- No `route_localnet` sysctl setting required
- Better performance with kernel-level redirection

## User Stories

### US1: As a developer, I want to run mefirst as a transparent HTTP proxy
So that I can intercept and customize HTTP responses for various use cases.

**Acceptance Criteria:**
- AC1.1: The proxy server listens on a configurable port (default: 8080) on localhost
- AC1.2: Applications can make HTTP requests to the proxy as if it were the target service
- AC1.3: The proxy forwards non-intercepted requests to the configured target service
- AC1.4: The proxy supports standard HTTP methods (GET, POST, PUT, DELETE, etc.)

### US2: As a developer, I want custom responses to be served for specific paths
So that I can customize HTTP behavior without modifying the proxy code.

**Acceptance Criteria:**
- AC2.1: Requests matching configured path patterns are intercepted
- AC2.2: Custom responses from template files are returned for matched paths
- AC2.3: Response format matches standard HTTP response format
- AC2.4: Unmatched requests are forwarded to target service

### US3: As a developer, I want transparent connection redirection using eBPF
So that I don't need to manage iptables rules or source port pools.

**Acceptance Criteria:**
- AC3.1: eBPF program redirects connections from configured target address:port to configured proxy address:port
- AC3.2: eBPF program attaches to system cgroup or specific container cgroups
- AC3.3: Proxy can connect to target service normally without special port handling
- AC3.4: Cgroup path is configurable via command-line or config file
- AC3.5: Proxy bind address and port are configurable and passed to eBPF program via BPF map
- AC3.6: Target address and port (what to intercept) are configurable and passed to eBPF program via BPF map
- AC3.7: Clear error messages if eBPF is unavailable (kernel version, permissions, etc.)

### US4: As a developer, I want a modular interception plugin system
So that I can configure custom responses for specific paths without writing code.

**Acceptance Criteria:**
- AC4.1: Interception plugin interface is defined as a Rust trait
- AC4.2: Plugins can be configured via command-line or config file
- AC4.3: Plugin configuration includes path patterns and response sources
- AC4.4: Response sources support static files (JSON/YAML/text) or command execution
- AC4.5: Command execution plugins run shell commands and return stdout as response
- AC4.6: Multiple plugins can be registered for different path patterns
- AC4.7: Unmatched requests are passed through to target service

### US5: As a developer, I want flexible configuration options
So that I can adapt the proxy to different environments and use cases.

**Acceptance Criteria:**
- AC5.1: Configuration can be provided via command-line arguments
- AC5.2: Configuration can be provided via TOML/YAML files
- AC5.3: Environment variables can override configuration values
- AC5.4: Configurable parameters include:
  - Cgroup path for eBPF attachment
  - Interception plugin configurations (path patterns, response templates)
  - Target service address and port
  - Bind port
- AC5.5: Configuration is validated on startup with clear error messages

### US6: As an operator, I want observability into proxy operations
So that I can monitor performance and troubleshoot issues.

**Acceptance Criteria:**
- AC6.1: All requests are logged with method, path, and status
- AC6.2: Plugin interception events are logged
- AC6.3: Errors are logged with context
- AC6.4: Prometheus-compatible metrics endpoint is available
- AC6.5: Metrics track request counts, latency, error rates
- AC6.6: Metrics track plugin hit/miss rates

### US7: As an operator, I want graceful shutdown
So that in-flight requests complete before the proxy terminates.

**Acceptance Criteria:**
- AC7.1: SIGTERM and SIGINT signals are handled
- AC7.2: In-flight requests are drained before shutdown
- AC7.3: Resources are cleaned up properly (eBPF programs, connections)
- AC7.4: Shutdown timeout is configurable

## Functional Requirements

### FR1: HTTP Proxy Server
- Listen on configurable port (default: 8080) on localhost
- Accept HTTP requests matching configured patterns
- Forward non-intercepted requests to target service
- Support standard HTTP methods

### FR2: Request Interception
- Intercept requests matching configured path patterns
- Return configured responses for matched paths
- Support static response files (JSON/YAML/text)
- Support command execution for dynamic responses
- Forward unmatched requests to target service

### FR3: eBPF-based Connection Redirection
- Use `BPF_PROG_TYPE_CGROUP_SOCK_ADDR` with `cgroup/connect4` attachment
- Transparently redirect `connect()` calls to configured target address/port → configured proxy address/port
- Configurable via CLI/config file:
  - Target: `--target-address`, `--target-port` (what to intercept)
  - Proxy: `--bind-address`, `--bind-port` (where to redirect to)
- Pass both target and proxy configuration to eBPF program via BPF maps
- Attach to system cgroup or specific container cgroups
- No iptables rules or source port management needed
- Proxy connects to target service using standard HTTP client
- PID filtering to prevent infinite loops

### FR4: Interception Plugin System
- Pluggable interception interface via traits
- Optional request interception (disabled = pure proxy mode)
- Plugin configuration:
  - **Path patterns**: Glob or regex patterns to match request paths
  - **Response source**: Static file or command execution
  - **Static files**: JSON/YAML/text files with response content
  - **Command execution**: Shell command to execute, stdout becomes response
  - **Status codes**: HTTP status code for the response
  - **Headers**: Custom headers to include in response
  - **Timeout**: Command execution timeout (for command-based plugins)
- Plugin registry for managing multiple plugins
- First-match routing (plugins evaluated in order)

### FR5: Configuration
- Command-line argument parsing
- Configurable parameters:
  - Cgroup path for eBPF attachment (default: system root cgroup)
  - Interception plugins: list of plugin configurations
  - Plugin path patterns (glob or regex)
  - Plugin response template files
  - Target service address and port
  - Bind port (default: 8080)

## Non-Functional Requirements

### NFR1: Performance
- Handle concurrent requests efficiently
- Minimize latency overhead vs direct target access
- Efficient eBPF-based redirection with minimal overhead

### NFR2: Reliability
- Graceful error handling
- Proper connection cleanup
- Plugin configuration validation
- Clear error messages when eBPF is unavailable

### NFR3: Security
- Prevent SSRF attacks by restricting proxy destinations
- Validate plugin response template files on startup

### NFR4: Observability
- Log all requests with method, path, and status
- Log plugin interception events
- Log errors with context

## Features

### F1: Enhanced Error Handling
- Structured error types with context
- Better error messages for debugging
- Graceful degradation on plugin failures

### F2: Metrics & Monitoring
- Prometheus-compatible metrics endpoint
- Track request counts, latency, error rates
- Plugin hit/miss metrics

### F3: Configuration File Support
- Support TOML/YAML configuration files
- Environment variable overrides
- Configuration validation on startup

### F4: Graceful Shutdown
- Handle SIGTERM/SIGINT signals
- Drain in-flight requests
- Clean up resources properly

### F5: Modular Interception Plugin System
- Trait-based interception plugin interface
- Optional request interception (can run as pure proxy)
- Configuration-driven plugin system:
  - Path pattern matching (glob or regex)
  - Static response files (JSON/YAML/text)
  - Command execution for dynamic responses
  - Custom status codes and headers
  - First-match routing
  - Command timeout configuration
- No code changes needed to add new interceptions

### F6: eBPF-based Connection Redirection
- Use `BPF_PROG_TYPE_CGROUP_SOCK_ADDR` for transparent connection redirection
- Eliminates iptables dependency and source port management complexity
- Attach to specific cgroups for per-container/per-process control
- Better performance (kernel-level redirection)
- Requires kernel 4.17+ with BPF support and appropriate permissions

## Implementation Status

**All requirements have been implemented and are working in production.**

Key implementation decisions:
- eBPF framework: **Aya** (pure Rust, no C dependencies)
- Path patterns: Support for **Exact, Glob, and Regex** patterns
- Plugin response sources: **File and Command** execution
- Metrics: Separate endpoint on port 9090 (configurable)
- Feature flag: `allow-external-ebpf` for loading external eBPF programs (disabled by default for security)
