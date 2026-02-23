#![cfg_attr(target_os = "none", no_std)]

// This library is intentionally minimal.
// The eBPF program doesn't need to share complex data structures with userspace.
// Process metadata is retrieved by userspace from /proc/net/tcp and /proc/net/tcp6.
