#[cfg(all(target_os = "linux", feature = "ebpf"))]
use crate::capability;

use crate::config::Config;
use crate::error::{InterposerError, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[cfg(all(target_os = "linux", feature = "ebpf"))]
use tracing::debug;

#[cfg(all(target_os = "linux", feature = "ebpf"))]
use aya::{Bpf, programs::{CgroupSockAddr, cgroup_sock_addr::CgroupSockAddrLinkId}};

#[cfg(all(target_os = "linux", feature = "ebpf"))]
use capctl::caps::{CapState, Cap};

pub struct EbpfRedirector {
    cgroup_path: PathBuf,
    #[cfg_attr(not(all(target_os = "linux", feature = "ebpf")), allow(dead_code))]
    config: Arc<Config>,
    #[cfg(feature = "allow-external-ebpf")]
    #[allow(dead_code)]
    program_path: Option<PathBuf>,
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    _bpf: Option<Bpf>,
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    _link_v4: Option<CgroupSockAddrLinkId>,
    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    _link_v6: Option<CgroupSockAddrLinkId>,
}

impl EbpfRedirector {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            cgroup_path: config.cgroup_path.clone(),
            config: Arc::new(config.clone()),
            #[cfg(feature = "allow-external-ebpf")]
            program_path: config.ebpf_program_path.clone(),
            #[cfg(all(target_os = "linux", feature = "ebpf"))]
            _bpf: None,
            #[cfg(all(target_os = "linux", feature = "ebpf"))]
            _link_v4: None,
            #[cfg(all(target_os = "linux", feature = "ebpf"))]
            _link_v6: None,
        })
    }

    pub async fn setup(&mut self) -> Result<()> {
        info!("Setting up eBPF redirection");
        info!("Attaching to cgroup: {:?}", self.cgroup_path);

        #[cfg(all(target_os = "linux", feature = "ebpf"))]
        {
            self.setup_linux().await
        }

        #[cfg(not(all(target_os = "linux", feature = "ebpf")))]
        {
            warn!("eBPF support is not available on this platform");
            warn!("eBPF requires Linux with the 'ebpf' feature enabled");
            Err(InterposerError::EbpfNotSupported(
                "eBPF is only supported on Linux with the 'ebpf' feature enabled".to_string()
            ))
        }
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    async fn setup_linux(&mut self) -> Result<()> {
        // Check kernel version
        self.check_kernel_version()?;

        // Check all required capabilities (eBPF + process metadata) before loading
        let _cap_status = capability::check_all_capabilities()?;

        // Check if cgroup path exists
        if !self.cgroup_path.exists() {
            return Err(InterposerError::CgroupNotFound(format!(
                "Cgroup path does not exist: {:?}",
                self.cgroup_path
            )));
        }

        // Load the eBPF program
        let mut bpf = {
            #[cfg(feature = "allow-external-ebpf")]
            {
                if let Some(ref path) = self.program_path {
                    info!("Loading eBPF program from file: {:?}", path);
                    self.load_from_file(path)?
                } else {
                    info!("Loading embedded eBPF program");
                    self.load_embedded()?
                }
            }
            #[cfg(not(feature = "allow-external-ebpf"))]
            {
                info!("Loading embedded eBPF program (external loading disabled)");
                self.load_embedded()?
            }
        };

        // Populate the PROXY_PID map with our PID to exclude ourselves from redirection
        // Do this BEFORE getting the program reference to avoid borrow checker issues
        let pid = std::process::id();
        info!("Setting proxy PID in eBPF map: {}", pid);
        
        {
            let mut proxy_pid_map: aya::maps::HashMap<_, u32, u32> = bpf
                .map_mut("PROXY_PID")
                .ok_or_else(|| InterposerError::EbpfLoad(
                    "PROXY_PID map not found in eBPF program".to_string()
                ))?
                .try_into()
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to get PROXY_PID map: {}", e
                )))?;
            
            proxy_pid_map.insert(0u32, pid, 0)
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to insert PID into PROXY_PID map: {}", e
                )))?;
        } // proxy_pid_map is dropped here, releasing the mutable borrow
        
        // Populate the TARGET_CONFIG map with target address and port to intercept
        info!("Configuring eBPF interception target: {}:{}", 
            self.config.target_address, self.config.target_port);
        
        {
            let mut target_config_map: aya::maps::HashMap<_, u32, u32> = bpf
                .map_mut("TARGET_CONFIG")
                .ok_or_else(|| InterposerError::EbpfLoad(
                    "TARGET_CONFIG map not found in eBPF program".to_string()
                ))?
                .try_into()
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to get TARGET_CONFIG map: {}", e
                )))?;
            
            // Parse target address to u32 (little-endian)
            // If target_address is empty or "0.0.0.0", use 0 to indicate IP-agnostic mode
            let target_ip = if self.config.target_address.is_empty() || self.config.target_address == "0.0.0.0" {
                info!("✓ IP-agnostic mode enabled: intercepting ALL connections to port {}", self.config.target_port);
                info!("  This will intercept connections to any IP address on port {}", self.config.target_port);
                0u32
            } else {
                info!("✓ IP-specific mode enabled: intercepting only {}:{}", 
                    self.config.target_address, self.config.target_port);
                parse_ipv4_to_u32(&self.config.target_address)?
            };
            let target_port = self.config.target_port as u32;
            
            // Key 0 = target IP (0 means intercept all IPs), Key 1 = target port
            target_config_map.insert(0u32, target_ip, 0)
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to insert target IP into TARGET_CONFIG map: {}", e
                )))?;
            
            target_config_map.insert(1u32, target_port, 0)
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to insert target port into TARGET_CONFIG map: {}", e
                )))?;
        } // target_config_map is dropped here, releasing the mutable borrow
        
        // Populate the PROXY_CONFIG map with proxy bind port
        // Always redirect to 127.0.0.1 (IPv4) and ::1 (IPv6) for the proxy
        info!("Configuring eBPF proxy target port: {}", self.config.bind_port);
        
        {
            let mut proxy_config_map: aya::maps::HashMap<_, u32, u32> = bpf
                .map_mut("PROXY_CONFIG")
                .ok_or_else(|| InterposerError::EbpfLoad(
                    "PROXY_CONFIG map not found in eBPF program".to_string()
                ))?
                .try_into()
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to get PROXY_CONFIG map: {}", e
                )))?;
            
            let proxy_port = self.config.bind_port as u32;
            
            // Key 0 = proxy port
            proxy_config_map.insert(0u32, proxy_port, 0)
                .map_err(|e| InterposerError::EbpfLoad(format!(
                    "Failed to insert proxy port into PROXY_CONFIG map: {}", e
                )))?;
        } // proxy_config_map is dropped here, releasing the mutable borrow

        // Attach IPv4 hook (connect4)
        info!("Attaching IPv4 (connect4) hook...");
        let program_v4: &mut CgroupSockAddr = bpf
            .program_mut("redirect_connect")
            .ok_or_else(|| InterposerError::EbpfLoad(
                "eBPF program 'redirect_connect' not found".to_string()
            ))?
            .try_into()
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to convert program to CgroupSockAddr: {}", e
            )))?;

        // Load the IPv4 program into the kernel
        program_v4.load().map_err(|e| InterposerError::EbpfLoad(format!(
            "Failed to load IPv4 eBPF program into kernel: {}", e
        )))?;

        // Open the cgroup for IPv4
        let cgroup_file_v4 = std::fs::File::open(&self.cgroup_path)
            .map_err(|e| InterposerError::CgroupNotFound(format!(
                "Failed to open cgroup at {:?}: {}", self.cgroup_path, e
            )))?;

        // Attach IPv4 hook to the cgroup
        let link_v4 = program_v4.attach(cgroup_file_v4)
            .map_err(|e| InterposerError::EbpfAttach(format!(
                "Failed to attach IPv4 eBPF program to cgroup: {}", e
            )))?;

        info!("✓ IPv4 hook attached successfully");

        // Attach IPv6 hook (connect6)
        info!("Attaching IPv6 (connect6) hook...");
        let program_v6: &mut CgroupSockAddr = bpf
            .program_mut("redirect_connect6")
            .ok_or_else(|| InterposerError::EbpfLoad(
                "eBPF program 'redirect_connect6' not found".to_string()
            ))?
            .try_into()
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to convert IPv6 program to CgroupSockAddr: {}", e
            )))?;

        // Load the IPv6 program into the kernel
        program_v6.load().map_err(|e| InterposerError::EbpfLoad(format!(
            "Failed to load IPv6 eBPF program into kernel: {}", e
        )))?;

        // Open the cgroup for IPv6
        let cgroup_file_v6 = std::fs::File::open(&self.cgroup_path)
            .map_err(|e| InterposerError::CgroupNotFound(format!(
                "Failed to open cgroup at {:?}: {}", self.cgroup_path, e
            )))?;

        // Attach IPv6 hook to the cgroup
        let link_v6 = program_v6.attach(cgroup_file_v6)
            .map_err(|e| InterposerError::EbpfAttach(format!(
                "Failed to attach IPv6 eBPF program to cgroup: {}", e
            )))?;

        info!("✓ IPv6 hook attached successfully");
        info!("eBPF programs (IPv4 + IPv6) attached successfully to cgroup: {:?}", self.cgroup_path);

        // Try to load and attach LSM hook for ptrace restriction (before dropping capabilities)
        // This is optional and will fail gracefully if LSM BPF is not supported
        self.setup_lsm_hook(&mut bpf, pid)?;

        // Store the BPF object and links to keep them alive
        self._bpf = Some(bpf);
        self._link_v4 = Some(link_v4);
        self._link_v6 = Some(link_v6);

        // Drop unnecessary capabilities after loading
        self.drop_capabilities()?;

        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    fn check_kernel_version(&self) -> Result<()> {
        // Read kernel version from /proc/version
        let version_str = std::fs::read_to_string("/proc/version")
            .map_err(|e| InterposerError::Ebpf(format!(
                "Failed to read kernel version: {}", e
            )))?;

        debug!("Kernel version: {}", version_str);

        // Parse kernel version (format: "Linux version X.Y.Z...")
        let version_parts: Vec<&str> = version_str
            .split_whitespace()
            .nth(2)
            .unwrap_or("0.0.0")
            .split('.')
            .collect();

        let major: u32 = version_parts.get(0)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let minor: u32 = version_parts.get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // BPF_PROG_TYPE_CGROUP_SOCK_ADDR requires kernel 4.17+
        if major < 4 || (major == 4 && minor < 17) {
            return Err(InterposerError::EbpfNotSupported(format!(
                "Kernel version {}.{} is too old. eBPF cgroup socket redirection requires kernel 4.17 or newer",
                major, minor
            )));
        }

        info!("Kernel version {}.{} supports eBPF cgroup socket redirection", major, minor);
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    fn setup_lsm_hook(&self, _bpf: &mut Bpf, proxy_pid: u32) -> Result<()> {
        info!("Attempting to load LSM hook for ptrace restriction");
        
        // Check if CAP_SYS_ADMIN is present (required for loading LSM BPF programs)
        use capctl::caps::{CapState, Cap};
        let caps = match CapState::get_current() {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to check capabilities for LSM: {}", e);
                warn!("LSM hook will not be loaded");
                return Ok(());
            }
        };
        
        if !caps.effective.has(Cap::SYS_ADMIN) {
            info!("CAP_SYS_ADMIN not present - LSM BPF programs require CAP_SYS_ADMIN");
            info!("LSM hook provides defense-in-depth ptrace restrictions");
            info!("To enable: sudo setcap 'cap_bpf,cap_sys_admin,cap_net_admin,cap_sys_ptrace,cap_dac_read_search=+ep' /path/to/mefirst");
            info!("Proxy will continue without ptrace restrictions");
            return Ok(());
        }
        
        // Check if BPF LSM is enabled in the kernel
        if let Ok(lsm_list) = std::fs::read_to_string("/sys/kernel/security/lsm") {
            if !lsm_list.contains("bpf") {
                info!("BPF LSM is not enabled in kernel (current LSMs: {})", lsm_list.trim());
                info!("To enable BPF LSM, add 'lsm=...,bpf' to kernel boot parameters");
                info!("Proxy will continue without ptrace restrictions");
                return Ok(());
            } else {
                info!("✓ BPF LSM is enabled in kernel");
            }
        } else {
            info!("Could not check LSM status - assuming LSM BPF not available");
            return Ok(());
        }
        
        // Load the separate LSM eBPF program
        let mut lsm_bpf = match self.load_lsm_program() {
            Ok(bpf) => bpf,
            Err(e) => {
                info!("LSM program not available or failed to load: {}", e);
                info!("Proxy will continue without ptrace restrictions");
                return Ok(());
            }
        };
        
        // Populate the PROXY_PID map for the LSM program
        match lsm_bpf.map_mut("PROXY_PID") {
            Some(map) => {
                let mut proxy_pid_map: aya::maps::Array<_, u32> = match map.try_into() {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("PROXY_PID map found but conversion failed: {}", e);
                        warn!("LSM hook will not be active - ptrace restrictions not enforced");
                        return Ok(());
                    }
                };
                
                match proxy_pid_map.set(0, proxy_pid, 0) {
                    Ok(_) => {
                        info!("✓ PROXY_PID map populated with proxy PID: {}", proxy_pid);
                    }
                    Err(e) => {
                        warn!("Failed to set proxy PID in PROXY_PID map: {}", e);
                        warn!("LSM hook will not be active - ptrace restrictions not enforced");
                        return Ok(());
                    }
                }
            }
            None => {
                warn!("PROXY_PID map not found in LSM program");
                warn!("LSM hook will not be active - ptrace restrictions not enforced");
                return Ok(());
            }
        }
        
        // Get and load the LSM program
        match lsm_bpf.program_mut("restrict_ptrace_access") {
            Some(program) => {
                let lsm_program: &mut aya::programs::Lsm = match program.try_into() {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("Failed to convert program to LSM type: {}", e);
                        warn!("LSM hook will not be active - ptrace restrictions not enforced");
                        return Ok(());
                    }
                };
                
                // Load BTF from /sys/kernel/btf/vmlinux
                let btf = match aya::Btf::from_sys_fs() {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("Failed to load BTF from /sys/kernel/btf/vmlinux: {}", e);
                        warn!("Kernel may not support BTF - LSM hook cannot be loaded");
                        return Ok(());
                    }
                };
                
                // Load the LSM program into the kernel
                info!("Loading LSM hook for ptrace restriction...");
                match lsm_program.load("ptrace_access_check", &btf) {
                    Ok(_) => {
                        info!("✓ LSM program loaded successfully");
                    }
                    Err(e) => {
                        warn!("Failed to load LSM program: {}", e);
                        warn!("LSM hook will not be active - ptrace restrictions not enforced");
                        return Ok(());
                    }
                }
                
                // Attach the LSM hook
                match lsm_program.attach() {
                    Ok(_) => {
                        info!("✓ LSM hook attached successfully");
                        info!("Ptrace restrictions active: all ptrace operations from proxy denied");
                        
                        // Keep the LSM BPF object alive by leaking it
                        // This ensures it stays active for the lifetime of the process
                        std::mem::forget(lsm_bpf);
                    }
                    Err(e) => {
                        warn!("Failed to attach LSM hook: {}", e);
                        warn!("LSM hook will not be active - ptrace restrictions not enforced");
                        return Ok(());
                    }
                }
            }
            None => {
                warn!("LSM program 'restrict_ptrace_access' not found in LSM binary");
                warn!("LSM hook will not be active - ptrace restrictions not enforced");
            }
        }
        
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    fn load_lsm_program(&self) -> Result<Bpf> {
        info!("Loading embedded LSM eBPF bytecode");
        
        // Embed the pre-built LSM eBPF program
        let lsm_bytecode = aya::include_bytes_aligned!(concat!(
            env!("CARGO_MANIFEST_DIR"), 
            "/target/bpfel-unknown-none/release/mefirst-lsm"
        ));
        
        info!("Embedded LSM eBPF bytecode size: {} bytes", lsm_bytecode.len());
        
        Bpf::load(lsm_bytecode)
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to load embedded LSM eBPF program: {}", e
            )))
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    fn drop_capabilities(&self) -> Result<()> {
        info!("Dropping unnecessary capabilities after eBPF program load");

        // Get current capability state
        let mut caps = CapState::get_current().map_err(|e| {
            warn!("Failed to get current capabilities for dropping: {}", e);
            return InterposerError::Ebpf(format!("Failed to get capabilities: {}", e));
        })?;

        // After loading the eBPF program, we can drop CAP_BPF and CAP_SYS_ADMIN
        // We keep CAP_NET_ADMIN as it might be needed for other network operations
        let caps_to_drop = vec![Cap::BPF, Cap::SYS_ADMIN];

        for cap in caps_to_drop {
            if caps.effective.has(cap) {
                caps.effective.drop(cap);
                caps.permitted.drop(cap);
                debug!("Marked capability {:?} for dropping", cap);
            }
        }

        // Apply the capability changes
        match caps.set_current() {
            Ok(_) => {
                info!("Successfully dropped unnecessary capabilities");
            }
            Err(e) => {
                // Not a fatal error if we can't drop capabilities
                warn!("Failed to drop capabilities: {}", e);
            }
        }

        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "ebpf", feature = "allow-external-ebpf"))]
    fn load_from_file(&self, path: &PathBuf) -> Result<Bpf> {
        let bytes = std::fs::read(path)
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to read eBPF program from {:?}: {}", path, e
            )))?;

        Bpf::load(&bytes)
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to load eBPF program from file: {}", e
            )))
    }

    #[cfg(all(target_os = "linux", feature = "ebpf"))]
    fn load_embedded(&self) -> Result<Bpf> {
        info!("Loading embedded eBPF bytecode");
        
        // Embed the pre-built eBPF program
        // Build the eBPF program first: ./scripts/build-ebpf.sh
        // Use include_bytes_aligned! for proper alignment required by eBPF ELF parsing
        let ebpf_bytecode = aya::include_bytes_aligned!(concat!(
            env!("CARGO_MANIFEST_DIR"), 
            "/target/bpfel-unknown-none/release/mefirst-ebpf"
        ));
        
        info!("Embedded eBPF bytecode size: {} bytes", ebpf_bytecode.len());
        info!("First 16 bytes: {:02x?}", &ebpf_bytecode[..16.min(ebpf_bytecode.len())]);
        
        Bpf::load(ebpf_bytecode)
            .map_err(|e| InterposerError::EbpfLoad(format!(
                "Failed to load embedded eBPF program: {}", e
            )))
    }

    pub async fn teardown(&self) -> Result<()> {
        info!("Tearing down eBPF redirection");
        // Links and BPF objects are automatically detached and cleaned up when dropped
        Ok(())
    }
}

/// Parse IPv4 address string to u32 in little-endian format for eBPF
#[cfg(all(target_os = "linux", feature = "ebpf"))]
fn parse_ipv4_to_u32(ip_str: &str) -> Result<u32> {
    let parts: Vec<&str> = ip_str.split('.').collect();
    if parts.len() != 4 {
        return Err(InterposerError::Config(format!(
            "Invalid IPv4 address: {}. Expected format: x.x.x.x", ip_str
        )));
    }
    
    let mut ip_u32: u32 = 0;
    for (i, part) in parts.iter().enumerate() {
        let octet: u8 = part.parse().map_err(|_| {
            InterposerError::Config(format!(
                "Invalid IPv4 octet in address {}: {}", ip_str, part
            ))
        })?;
        // Little-endian: first octet goes in lowest byte
        ip_u32 |= (octet as u32) << (i * 8);
    }
    
    Ok(ip_u32)
}
