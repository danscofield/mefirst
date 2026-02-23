pub mod retriever;

use std::net::SocketAddr;

/// Userspace representation of process information
/// This is converted from the eBPF ProcessMetadata structure
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// User ID of the process
    pub uid: u32,
    /// Username (resolved from uid)
    pub username: String,
    /// Process ID
    pub pid: u32,
    /// Executable file path
    pub executable: String,
    /// Command line arguments
    pub cmdline: String,
}

impl ProcessInfo {
    /// Create a new ProcessInfo
    pub fn new(
        uid: u32,
        username: String,
        pid: u32,
        executable: String,
        cmdline: String,
    ) -> Self {
        Self {
            uid,
            username,
            pid,
            executable,
            cmdline,
        }
    }
}

/// Result type combining process info and original destination
pub type ProcessInfoWithDestination = (ProcessInfo, SocketAddr);
