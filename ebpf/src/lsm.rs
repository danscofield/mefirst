#![no_std]
#![no_main]

//! LSM eBPF program for restricting ptrace operations from the proxy process
//! 
//! This is a separate eBPF binary that provides an LSM hook to restrict the proxy
//! process to read-only ptrace operations, preventing it from attaching to or
//! modifying other processes.
//! 
//! Requirements:
//! - Linux kernel 5.7+ with LSM BPF support enabled
//! - CONFIG_BPF_LSM=y in kernel config
//! - "bpf" added to lsm= kernel parameter
//!
//! This program is loaded separately from the main cgroup programs and will
//! fail gracefully if LSM BPF is not supported.

use aya_ebpf::{
    macros::{lsm, map},
    maps::Array,
    programs::LsmContext,
    helpers::bpf_get_current_pid_tgid,
};
use aya_log_ebpf::warn;

/// Map to store the proxy's PID for LSM checks
#[map]
static PROXY_PID: Array<u32> = Array::with_max_entries(1, 0);

/// LSM hook for ptrace_access_check
/// 
/// This hook is called whenever a process attempts to ptrace another process.
/// We use it to restrict the proxy process to read-only ptrace operations.
/// 
/// The proxy needs /proc filesystem access (which uses PTRACE_MODE_READ),
/// but should not be able to attach to or modify other processes (PTRACE_MODE_ATTACH).
/// 
/// This hook provides defense-in-depth by blocking write-mode ptrace operations
/// while allowing read-only access needed for /proc/pid/fd spidering.
#[lsm(hook = "ptrace_access_check")]
pub fn restrict_ptrace_access(ctx: LsmContext) -> i32 {
    match try_restrict_ptrace_access(&ctx) {
        Ok(ret) => ret,
        Err(_) => 0, // Allow on error to avoid breaking system
    }
}

fn try_restrict_ptrace_access(ctx: &LsmContext) -> Result<i32, i32> {
    // Get the proxy PID from the map
    let proxy_pid = unsafe {
        PROXY_PID.get(0).ok_or(0)?
    };

    // Get current process PID (upper 32 bits of pid_tgid)
    let pid_tgid = unsafe { bpf_get_current_pid_tgid() };
    let current_pid = (pid_tgid >> 32) as u32;

    // Only restrict if this is the proxy process
    if current_pid != *proxy_pid {
        return Ok(0); // Allow - not the proxy process
    }

    // Get the ptrace mode from the LSM context
    // The mode is the second argument to ptrace_access_check
    // PTRACE_MODE_READ = 0x01 (read-only, used by /proc)
    // PTRACE_MODE_ATTACH = 0x02 (attach, allows modification)
    // PTRACE_MODE_FSCREDS = 0x04 (use filesystem credentials)
    let mode: u32 = unsafe { ctx.arg(1) };
    
    // Allow read-only ptrace (used by /proc filesystem)
    // Block attach mode (PTRACE_MODE_ATTACH = 0x02)
    const PTRACE_MODE_ATTACH: u32 = 0x02;
    
    if (mode & PTRACE_MODE_ATTACH) != 0 {
        // Block attach mode - proxy should not attach to other processes
        warn!(
            ctx,
            "Blocked ptrace ATTACH from proxy (pid={}, mode={})",
            current_pid,
            mode
        );
        // Return -EPERM to deny the operation
        return Ok(-1);
    }
    
    // Allow read-only ptrace (needed for /proc access)
    Ok(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
