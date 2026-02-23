# IPv6 Support

## Overview

The proxy automatically supports both IPv4 and IPv6 connections with dual listeners:
- **IPv4**: `127.0.0.1:8080` - handles IPv4 connections
- **IPv6**: `[::1]:8080` - handles IPv6 connections

Both listeners run concurrently, ensuring full dual-stack support without exposing the proxy to remote connections.

## How It Works

### IPv4 Connections
```bash
curl -4 google.com:80
```
1. eBPF `connect4` hook intercepts the connection
2. Redirects to `127.0.0.1:8080`
3. IPv4 listener accepts and handles the request

### IPv6 Connections
```bash
curl -6 google.com:80
```
1. eBPF `connect6` hook intercepts the connection
2. Redirects to `::1:8080`
3. IPv6 listener accepts and handles the request

### Auto-Detection
```bash
curl google.com:80
```
Modern curl tries IPv6 first, then falls back to IPv4 if unavailable.

## Process Metadata

Process metadata (uid, username, pid, executable, cmdline) is captured for both IPv4 and IPv6 connections when eBPF is enabled.

The proxy reads from:
- `/proc/net/tcp` for IPv4 connections
- `/proc/net/tcp6` for IPv6 connections

## Testing

### Verify Both Listeners Are Running

```bash
# Check listening sockets
sudo netstat -tlnp | grep mefirst
# Should show:
# tcp   0   0 127.0.0.1:8080   0.0.0.0:*   LISTEN   <pid>/mefirst
# tcp6  0   0 ::1:8080         :::*        LISTEN   <pid>/mefirst

# Or with ss
sudo ss -tlnp | grep mefirst
```

### Test IPv4
```bash
curl -4 -v google.com:80
```

### Test IPv6
```bash
curl -6 -v google.com:80
```

### Test Auto
```bash
curl -v google.com:80
```

## Troubleshooting

### IPv6 Connections Hang

**Symptom:** `curl -6 google.com:80` hangs indefinitely

**Possible Causes:**

1. **IPv6 is disabled on the system**
   ```bash
   # Check if IPv6 is enabled
   cat /proc/sys/net/ipv6/conf/all/disable_ipv6
   # Should be 0 (enabled)
   
   # Test IPv6 loopback
   ping6 ::1
   ```

2. **IPv6 listener failed to bind**
   Check the proxy logs for errors like:
   ```
   Failed to bind to [::1]:8080: ...
   ```

3. **eBPF connect6 hook not attached**
   ```bash
   # Check eBPF programs
   sudo bpftool prog list | grep -A5 cgroup_sock_addr
   # Should show both redirect_connect and redirect_connect6
   ```

### Process Metadata Missing for IPv6

**Symptom:** IPv6 connections work but process metadata is not logged

**Possible Causes:**

1. **eBPF not enabled**
   Ensure `enable_ebpf = true` in your config

2. **Process metadata retriever failed to initialize**
   Check logs for warnings about process metadata retriever

3. **Socket not found in /proc/net/tcp6**
   This can happen if the connection closes before metadata is retrieved

### IPv4 Works But IPv6 Doesn't

**Symptom:** `curl -4` works but `curl -6` fails

**Check:**
1. Verify IPv6 listener is running (see "Verify Both Listeners Are Running" above)
2. Check if IPv6 is enabled on the system
3. Verify eBPF `connect6` hook is attached

## Security

### Localhost-Only Binding

The proxy always binds to localhost addresses:
- `127.0.0.1` for IPv4
- `::1` for IPv6

This ensures the proxy never accepts remote connections. The binding addresses are hardcoded for security and cannot be changed via configuration.

### No Firewall Rules Needed

Unlike binding to `::` (all interfaces), binding to `::1` (IPv6 loopback) does not require firewall rules to restrict access. The proxy is inherently local-only.

## Configuration

No special configuration is needed for IPv6 support. The proxy automatically creates both listeners when started.

**Example config:**
```toml
enable_ebpf = true
bind_port = 8080  # Creates listeners on 127.0.0.1:8080 and [::1]:8080

[interception]
port = 80  # Intercepts both IPv4 and IPv6 connections
```

## Logs

When debug logging is enabled (`RUST_LOG=debug`), you'll see:

```
Starting dual-stack proxy server on localhost:
  IPv4: 127.0.0.1:8080
  IPv6: [::1]:8080
✓ IPv4 listener bound successfully to 127.0.0.1:8080
✓ IPv6 listener bound successfully to [::1]:8080
[IPv4] Accept loop started
[IPv6] Accept loop started
```

When connections are accepted:
```
[IPv4] Accepted connection from 127.0.0.1:54321
[IPv6] Accepted connection from [::1]:54322
```
