#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]
#![cfg_attr(target_os = "none", allow(unused_unsafe))]

#[cfg(target_os = "none")]
use aya_ebpf::{
    macros::{cgroup_sock_addr, map},
    programs::SockAddrContext,
    maps::HashMap,
    helpers::bpf_get_current_pid_tgid,
};

/// Map to store the proxy's PID to exclude it from redirection
#[cfg(target_os = "none")]
#[map]
static PROXY_PID: HashMap<u32, u32> = HashMap::with_max_entries(1, 0);

/// Proxy configuration: stores the proxy bind port
/// Key 0 = proxy port (u16 stored as u32)
#[cfg(target_os = "none")]
#[map]
static PROXY_CONFIG: HashMap<u32, u32> = HashMap::with_max_entries(1, 0);

/// Target configuration: stores the target address and port to intercept
/// Key 0 = target IP (u32 in network byte order) - default 169.254.169.254, or 0 if not specified
/// Key 1 = target port (u16 stored as u32)
/// When Key 0 is 0, intercept all connections to the specified port regardless of destination IP
#[cfg(target_os = "none")]
#[map]
static TARGET_CONFIG: HashMap<u32, u32> = HashMap::with_max_entries(2, 0);

/// eBPF program that redirects IPv4 connections to a configured target address:port
/// to our local proxy (127.0.0.1:bind_port)
/// 
/// Excludes the proxy's own PID to prevent infinite loops
#[cfg(target_os = "none")]
#[cgroup_sock_addr(connect4)]
pub fn redirect_connect(ctx: SockAddrContext) -> i32 {
    match try_redirect_connect(ctx) {
        Ok(ret) => ret,
        Err(_) => 1, // Allow connection on error
    }
}

/// eBPF program that redirects IPv6 connections to our local proxy (::1:bind_port)
/// 
/// Excludes the proxy's own PID to prevent infinite loops
#[cfg(target_os = "none")]
#[cgroup_sock_addr(connect6)]
pub fn redirect_connect6(ctx: SockAddrContext) -> i32 {
    match try_redirect_connect6(ctx) {
        Ok(ret) => ret,
        Err(_) => 1, // Allow connection on error
    }
}

#[cfg(target_os = "none")]
fn try_redirect_connect(ctx: SockAddrContext) -> Result<i32, ()> {
    // Get current process PID (upper 32 bits of pid_tgid)
    let pid_tgid = unsafe { bpf_get_current_pid_tgid() };
    let pid = (pid_tgid >> 32) as u32;

    // Check if this is the proxy's own PID - if so, don't redirect
    let key = 0u32;
    if let Some(proxy_pid) = unsafe { PROXY_PID.get(&key) } {
        if pid == *proxy_pid {
            return Ok(1);
        }
    }

    // Get target configuration
    let target_ip = unsafe { TARGET_CONFIG.get(&0u32) }
        .copied()
        .unwrap_or(0xFEA9FEA9);

    let target_port_u32 = unsafe { TARGET_CONFIG.get(&1u32) }
        .copied()
        .unwrap_or(80u32);
    let target_port = target_port_u32 as u16;

    let user_ip = unsafe { (*ctx.sock_addr).user_ip4 };
    let user_port = unsafe { (*ctx.sock_addr).user_port };

    // Check if this is a connection to the configured target
    // If target_ip is 0, intercept all connections to the specified port (IP-agnostic mode)
    // Otherwise, intercept only connections to the specific (ip, port) pair
    let should_intercept = if target_ip == 0 {
        // IP-agnostic mode: intercept all connections to the target port
        user_port as u16 == target_port.to_be()
    } else {
        // IP-specific mode: intercept only connections to the specific (ip, port) pair
        user_ip == target_ip && user_port as u16 == target_port.to_be()
    };

    if should_intercept {
        // Get proxy port from config
        let proxy_port_u32 = unsafe { PROXY_CONFIG.get(&0u32) }
            .copied()
            .unwrap_or(8080u32);
        let proxy_port = proxy_port_u32 as u16;
        
        // Redirect to 127.0.0.1:proxy_port
        unsafe {
            (*ctx.sock_addr).user_ip4 = 0x0100007F; // 127.0.0.1 in little-endian
            (*ctx.sock_addr).user_port = proxy_port.to_be() as u32;
        }
    }

    Ok(1)
}

/// Handle IPv6 connection redirection
#[cfg(target_os = "none")]
fn try_redirect_connect6(ctx: SockAddrContext) -> Result<i32, ()> {
    // Get current process PID (upper 32 bits of pid_tgid)
    let pid_tgid = unsafe { bpf_get_current_pid_tgid() };
    let pid = (pid_tgid >> 32) as u32;

    // Check if this is the proxy's own PID - if so, don't redirect
    let key = 0u32;
    if let Some(proxy_pid) = unsafe { PROXY_PID.get(&key) } {
        if pid == *proxy_pid {
            return Ok(1);
        }
    }

    // Get target configuration
    let target_port_u32 = unsafe { TARGET_CONFIG.get(&1u32) }
        .copied()
        .unwrap_or(80u32);
    let target_port = target_port_u32 as u16;

    let user_port = unsafe { (*ctx.sock_addr).user_port };

    // For IPv6, we only support IP-agnostic mode (intercept all IPs on the port)
    // Check if this is a connection to the configured target port
    let should_intercept = user_port as u16 == target_port.to_be();

    if should_intercept {
        // Get proxy port from config
        let proxy_port_u32 = unsafe { PROXY_CONFIG.get(&0u32) }
            .copied()
            .unwrap_or(8080u32);
        let proxy_port = proxy_port_u32 as u16;
        
        // Redirect to ::1:proxy_port
        unsafe {
            // Set IPv6 address to ::1 (IPv6 loopback)
            // ::1 = 0000:0000:0000:0000:0000:0000:0000:0001
            (*ctx.sock_addr).user_ip6[0] = 0;
            (*ctx.sock_addr).user_ip6[1] = 0;
            (*ctx.sock_addr).user_ip6[2] = 0;
            (*ctx.sock_addr).user_ip6[3] = 1u32.to_be(); // 0x00000001 in network byte order
            (*ctx.sock_addr).user_port = proxy_port.to_be() as u32;
        }
    }

    Ok(1)
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}

// Stub for non-eBPF targets (macOS, etc.)
#[cfg(not(target_os = "none"))]
fn main() {
    println!("This is an eBPF program and cannot be run directly.");
    println!("Build it with: cargo +nightly build --release -Z build-std=core --target bpfel-unknown-none");
}
