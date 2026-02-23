#[cfg(all(target_os = "linux", feature = "ebpf"))]
use tracing::{info, warn};

#[cfg(all(target_os = "linux", feature = "ebpf"))]
use capctl::caps::{CapState, Cap};

use crate::error::{InterposerError, Result};

/// Status of all capabilities required for eBPF and process metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CapabilityStatus {
    // eBPF operation capabilities
    pub has_bpf: bool,
    pub has_sys_admin: bool,
    pub has_net_admin: bool,
    
    // Process metadata capabilities
    pub has_sys_ptrace: bool,
    pub has_dac_read_search: bool,
}

/// Check for all required capabilities before loading eBPF programs
/// This should be called before any eBPF operations
#[allow(dead_code)]
pub fn check_all_capabilities() -> Result<CapabilityStatus> {
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    {
        check_all_capabilities_linux()
    }

    #[cfg(not(all(target_os = "linux", feature = "ebpf")))]
    {
        Err(InterposerError::EbpfNotSupported(
            "eBPF is only supported on Linux with the 'ebpf' feature enabled".to_string()
        ))
    }
}

#[cfg(all(target_os = "linux", feature = "ebpf"))]
fn check_all_capabilities_linux() -> Result<CapabilityStatus> {
    info!("Checking all required capabilities for eBPF and process metadata");

    // Get current capability state
    let caps = CapState::get_current().map_err(|e| {
        InterposerError::Ebpf(format!("Failed to get current capabilities: {}", e))
    })?;

    // Check eBPF operation capabilities
    let has_bpf = caps.effective.has(Cap::BPF);
    let has_sys_admin = caps.effective.has(Cap::SYS_ADMIN);
    let has_net_admin = caps.effective.has(Cap::NET_ADMIN);
    
    // Check process metadata capabilities
    let has_sys_ptrace = caps.effective.has(Cap::SYS_PTRACE);
    let has_dac_read_search = caps.effective.has(Cap::DAC_READ_SEARCH);

    let status = CapabilityStatus {
        has_bpf,
        has_sys_admin,
        has_net_admin,
        has_sys_ptrace,
        has_dac_read_search,
    };

    // Check eBPF capabilities (required)
    // CAP_BPF is required for loading BPF programs (kernel 5.8+)
    // CAP_SYS_ADMIN is required for loading LSM BPF programs and as fallback for CAP_BPF
    // CAP_NET_ADMIN is required for attaching cgroup socket programs
    
    // For full functionality (cgroup + LSM), we need CAP_SYS_ADMIN
    // For cgroup-only functionality, CAP_BPF is sufficient
    if !has_bpf && !has_sys_admin {
        return Err(InterposerError::EbpfNotSupported(
            "Missing required eBPF capability: CAP_BPF or CAP_SYS_ADMIN. Run with: sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst".to_string()
        ));
    } else if has_bpf && has_sys_admin {
        info!("✓ CAP_BPF and CAP_SYS_ADMIN present (full LSM support available)");
    } else if has_sys_admin {
        info!("✓ CAP_SYS_ADMIN present (fallback for CAP_BPF, LSM support available)");
    } else if has_bpf {
        info!("✓ CAP_BPF present");
        warn!("⚠ CAP_SYS_ADMIN is missing - LSM BPF programs cannot be loaded");
        warn!("  LSM hook provides defense-in-depth ptrace restrictions");
        warn!("  To enable LSM support: sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst");
    }

    if !has_net_admin {
        return Err(InterposerError::EbpfNotSupported(
            "Missing required capability: CAP_NET_ADMIN (required for attaching cgroup socket programs). Run with: sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst".to_string()
        ));
    } else {
        info!("✓ CAP_NET_ADMIN present");
    }

    // Check process metadata capabilities (warnings only)
    if has_sys_ptrace && has_dac_read_search {
        info!("✓ All process metadata capabilities present (CAP_SYS_PTRACE, CAP_DAC_READ_SEARCH)");
    } else {
        if !has_sys_ptrace {
            warn!("⚠ CAP_SYS_PTRACE is missing - process metadata retrieval may fail for other users' processes");
        }
        if !has_dac_read_search {
            warn!("⚠ CAP_DAC_READ_SEARCH is missing - process metadata access may be restricted");
        }
        warn!("  To enable all capabilities: sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst");
        info!("Process metadata retrieval will continue with available permissions");
    }

    Ok(status)
}
