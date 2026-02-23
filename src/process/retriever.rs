use super::ProcessInfoWithDestination;
#[cfg(target_os = "linux")]
use super::ProcessInfo;
use crate::error::Result;
#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::fs;
use std::net::SocketAddr;
#[cfg(target_os = "linux")]
use std::sync::RwLock;

/// Retrieves process metadata by spidering /proc
pub struct ProcessMetadataRetriever {
    #[cfg(target_os = "linux")]
    cache: RwLock<HashMap<u64, ProcessInfo>>,
}

impl ProcessMetadataRetriever {
    /// Create a new ProcessMetadataRetriever
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self { 
                cache: RwLock::new(HashMap::new()),
            })
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {})
        }
    }
    
    /// Get process metadata for a connection by spidering /proc
    /// 
    /// This finds the PID by:
    /// 1. Getting the socket inode from fstat()
    /// 2. Searching /proc/*/fd/* to find which process owns the socket
    /// 3. Reading process info from /proc/<pid>/
    pub fn get_metadata_from_fd(&self, fd: i32) -> Option<ProcessInfoWithDestination> {
        #[cfg(target_os = "linux")]
        {
            let result = self.get_metadata_from_fd_impl(fd);
            if result.is_none() {
                tracing::debug!("Failed to retrieve process metadata for fd {}", fd);
            }
            result
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            let _ = fd; // Suppress unused warning
            None
        }
    }
    
    /// Get process metadata for a client connection by peer address
    /// 
    /// This searches /proc/net/tcp to find which local process has a socket
    /// connected to our server with the given peer address
    pub fn get_metadata_from_peer_addr(&self, peer_addr: &SocketAddr) -> Option<ProcessInfoWithDestination> {
        #[cfg(target_os = "linux")]
        {
            let result = self.get_metadata_from_peer_addr_impl(peer_addr);
            if result.is_none() {
                tracing::debug!("Failed to retrieve process metadata for peer {}", peer_addr);
            }
            result
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            let _ = peer_addr;
            None
        }
    }
    
    #[cfg(target_os = "linux")]
    fn get_metadata_from_fd_impl(&self, fd: i32) -> Option<ProcessInfoWithDestination> {
        use tracing::{info, debug, warn};
        
        debug!("Starting process metadata retrieval for fd {}", fd);
        
        // Get socket inode
        let inode = match get_socket_inode(fd) {
            Some(i) => {
                debug!("Found socket inode {} for fd {}", i, fd);
                i
            }
            None => {
                warn!("Failed to get socket inode for fd {}", fd);
                return None;
            }
        };
        
        // Find PID that owns this socket
        let pid = match find_pid_by_socket_inode(inode) {
            Some(p) => {
                info!("Identified process PID {} from socket inode {}", p, inode);
                p
            }
            None => {
                warn!("Failed to find PID for socket inode {}", inode);
                return None;
            }
        };
        
        // Read process metadata
        let uid = match read_proc_status_uid(pid) {
            Some(u) => u,
            None => {
                warn!("Failed to read UID for PID {}", pid);
                return None;
            }
        };
        
        let username = resolve_username(uid);
        let executable = read_proc_exe(pid);
        let cmdline = read_proc_cmdline(pid);
        
        info!(
            "Retrieved process metadata: pid={}, uid={}, username={}, executable={}, cmdline={}",
            pid, uid, username, executable, cmdline
        );
        
        let process_info = ProcessInfo::new(uid, username, pid, executable, cmdline);
        
        // Placeholder destination (we'd need the eBPF map for the original dest)
        let placeholder_dest = SocketAddr::from(([0, 0, 0, 0], 0));
        
        Some((process_info, placeholder_dest))
    }
    
    #[cfg(target_os = "linux")]
    fn get_metadata_from_peer_addr_impl(&self, peer_addr: &SocketAddr) -> Option<ProcessInfoWithDestination> {
        use tracing::{info, debug, warn};
        
        debug!("Searching for client process with peer address {}", peer_addr);
        
        // Find the socket inode for the client connection
        let inode = find_client_socket_inode(peer_addr)?;
        debug!("Found client socket inode {} for peer {}", inode, peer_addr);
        
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(cached_info) = cache.get(&inode) {
                debug!("Cache hit for inode {}", inode);
                let placeholder_dest = SocketAddr::from(([0, 0, 0, 0], 0));
                return Some((cached_info.clone(), placeholder_dest));
            }
        }
        
        debug!("Cache miss for inode {}, performing proc spider", inode);
        
        // Find PID that owns this socket
        let pid = match find_pid_by_socket_inode(inode) {
            Some(p) => {
                info!("Identified client process PID {} from socket inode {}", p, inode);
                p
            }
            None => {
                warn!("Failed to find PID for client socket inode {}", inode);
                return None;
            }
        };
        
        // Read process metadata
        let uid = match read_proc_status_uid(pid) {
            Some(u) => u,
            None => {
                warn!("Failed to read UID for PID {}", pid);
                return None;
            }
        };
        
        let username = resolve_username(uid);
        let executable = read_proc_exe(pid);
        let cmdline = read_proc_cmdline(pid);
        
        info!(
            "Retrieved client process metadata: pid={}, uid={}, username={}, executable={}, cmdline={}",
            pid, uid, username, executable, cmdline
        );
        
        let process_info = ProcessInfo::new(uid, username, pid, executable, cmdline);
        
        // Cache the result
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(inode, process_info.clone());
            debug!("Cached process info for inode {}", inode);
        }
        
        // Placeholder destination
        let placeholder_dest = SocketAddr::from(([0, 0, 0, 0], 0));
        
        Some((process_info, placeholder_dest))
    }
}

/// Get socket inode from file descriptor
#[cfg(target_os = "linux")]
fn get_socket_inode(fd: i32) -> Option<u64> {
    let path = format!("/proc/self/fd/{}", fd);
    let link_target = fs::read_link(&path).ok()?;
    let link_str = link_target.to_str()?;
    
    // Socket links look like: socket:[12345]
    if link_str.starts_with("socket:[") && link_str.ends_with(']') {
        let inode_str = &link_str[8..link_str.len()-1];
        inode_str.parse().ok()
    } else {
        None
    }
}

/// Find PID that owns a socket with the given inode
#[cfg(target_os = "linux")]
fn find_pid_by_socket_inode(inode: u64) -> Option<u32> {
    use tracing::debug;
    
    let proc_dir = match fs::read_dir("/proc") {
        Ok(dir) => dir,
        Err(e) => {
            debug!("Failed to read /proc: {}", e);
            return None;
        }
    };
    
    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let pid_str = match file_name.to_str() {
            Some(s) => s,
            None => continue,
        };
        
        // Skip non-numeric entries (like "self", "cpuinfo", etc.)
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        
        // Check this process's file descriptors
        let fd_dir_path = format!("/proc/{}/fd", pid);
        let fd_dir = match fs::read_dir(&fd_dir_path) {
            Ok(dir) => dir,
            Err(_) => continue, // Permission denied or process exited
        };
        
        for fd_entry in fd_dir.flatten() {
            if let Ok(link_target) = fs::read_link(fd_entry.path()) {
                if let Some(link_str) = link_target.to_str() {
                    let expected = format!("socket:[{}]", inode);
                    if link_str == expected {
                        debug!("Found PID {} owns socket inode {}", pid, inode);
                        return Some(pid);
                    }
                }
            }
        }
    }
    
    debug!("No process found owning socket inode {}", inode);
    None
}

/// Read UID from /proc/<pid>/status
#[cfg(target_os = "linux")]
fn read_proc_status_uid(pid: u32) -> Option<u32> {
    let status_path = format!("/proc/{}/status", pid);
    let content = fs::read_to_string(&status_path).ok()?;
    
    for line in content.lines() {
        if line.starts_with("Uid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().ok();
            }
        }
    }
    
    None
}

/// Read executable path from /proc/<pid>/exe
#[cfg_attr(not(target_os = "linux"), allow(dead_code, unused_variables))]
fn read_proc_exe(pid: u32) -> String {
    #[cfg(target_os = "linux")]
    {
        let exe_path = format!("/proc/{}/exe", pid);
        match fs::read_link(&exe_path) {
            Ok(path) => path.to_string_lossy().to_string(),
            Err(_) => format!("<unknown-{}>", pid),
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        format!("<pid-{}>", pid)
    }
}

/// Read command line from /proc/<pid>/cmdline
#[cfg_attr(not(target_os = "linux"), allow(dead_code, unused_variables))]
fn read_proc_cmdline(pid: u32) -> String {
    #[cfg(target_os = "linux")]
    {
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        match fs::read(&cmdline_path) {
            Ok(bytes) => {
                // cmdline is null-separated, convert to space-separated
                let cmdline_str = String::from_utf8_lossy(&bytes);
                cmdline_str.replace('\0', " ").trim().to_string()
            }
            Err(_) => String::new(),
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        String::new()
    }
}

/// Resolve username from UID
/// Falls back to numeric UID string if resolution fails
#[cfg_attr(not(target_os = "linux"), allow(dead_code, unused_variables))]
fn resolve_username(uid: u32) -> String {
    #[cfg(target_os = "linux")]
    {
        // Try to resolve username using libc
        unsafe {
            let pwd = libc::getpwuid(uid);
            if !pwd.is_null() {
                let name_ptr = (*pwd).pw_name;
                if !name_ptr.is_null() {
                    if let Ok(name) = std::ffi::CStr::from_ptr(name_ptr).to_str() {
                        return name.to_string();
                    }
                }
            }
        }
    }
    
    // Fallback to numeric UID
    uid.to_string()
}

/// Find the socket inode for a client connection by searching /proc/net/tcp
/// 
/// This looks for a TCP connection where the remote address matches the peer_addr.
/// In /proc/net/tcp, the "local_address" is the client's address and "rem_address" 
/// is the server's address (from the kernel's perspective of listing all connections).
#[cfg(target_os = "linux")]
fn find_client_socket_inode(peer_addr: &SocketAddr) -> Option<u64> {
    use tracing::debug;
    
    // Convert peer address to hex format used in /proc/net/tcp or /proc/net/tcp6
    let (proc_file, peer_pattern) = match peer_addr {
        SocketAddr::V4(v4) => {
            // Read /proc/net/tcp for IPv4
            let octets = v4.ip().octets();
            // /proc/net/tcp uses little-endian hex format
            let ip_hex = format!("{:02X}{:02X}{:02X}{:02X}", 
                octets[3], octets[2], octets[1], octets[0]);
            let port_hex = format!("{:04X}", v4.port());
            let pattern = format!("{}:{}", ip_hex, port_hex);
            ("/proc/net/tcp", pattern)
        }
        SocketAddr::V6(v6) => {
            // Read /proc/net/tcp6 for IPv6
            let segments = v6.ip().segments();
            // /proc/net/tcp6 stores IPv6 addresses as 4 x 32-bit words in little-endian format
            // Convert 8 x 16-bit segments to 4 x 32-bit words, then reverse bytes in each word
            let word0 = ((segments[0] as u32) << 16) | (segments[1] as u32);
            let word1 = ((segments[2] as u32) << 16) | (segments[3] as u32);
            let word2 = ((segments[4] as u32) << 16) | (segments[5] as u32);
            let word3 = ((segments[6] as u32) << 16) | (segments[7] as u32);
            
            // Format each 32-bit word in little-endian (reverse byte order)
            let ip_hex = format!("{:08X}", word0.swap_bytes()) +
                         &format!("{:08X}", word1.swap_bytes()) +
                         &format!("{:08X}", word2.swap_bytes()) +
                         &format!("{:08X}", word3.swap_bytes());
            let port_hex = format!("{:04X}", v6.port());
            let pattern = format!("{}:{}", ip_hex, port_hex);
            debug!("IPv6 peer address: {:?}, segments: {:?}, pattern: {}", v6, segments, pattern);
            ("/proc/net/tcp6", pattern)
        }
    };
    
    debug!("Looking for client socket in {} with local_address pattern: {}", proc_file, peer_pattern);
    
    // Read the appropriate proc file
    let tcp_content = fs::read_to_string(proc_file).ok()?;
    
    // Search for a connection where local_address matches the peer
    for line in tcp_content.lines().skip(1) { // Skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() > 9 {
            let local_addr = parts[1]; // local_address column
            
            if local_addr == peer_pattern {
                if let Ok(inode) = parts[9].parse::<u64>() {
                    debug!("Found matching connection with inode {}", inode);
                    return Some(inode);
                }
            }
        }
    }
    
    debug!("No matching connection found in {}. Dumping first 5 lines:", proc_file);
    for (i, line) in tcp_content.lines().take(6).enumerate() {
        debug!("  Line {}: {}", i, line);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resolve_username_fallback() {
        // Test with a UID that likely doesn't exist
        let username = resolve_username(99999);
        assert_eq!(username, "99999");
    }
    
    #[test]
    fn test_resolve_username_root() {
        // Root user should always exist on Linux
        #[cfg(target_os = "linux")]
        {
            let username = resolve_username(0);
            assert_eq!(username, "root");
        }
    }
}
