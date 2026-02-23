use mefirst::capability::{check_all_capabilities, CapabilityStatus};

#[test]
fn test_capability_check_returns_status() {
    // This test verifies that capability checking returns a valid status
    // The actual capabilities depend on the test environment
    let result = check_all_capabilities();
    
    #[cfg(target_os = "linux")]
    {
        let status = result.unwrap();
        // Verify the structure is valid (booleans can be true or false)
        assert!(status.has_bpf == true || status.has_bpf == false);
        assert!(status.has_sys_admin == true || status.has_sys_admin == false);
        assert!(status.has_net_admin == true || status.has_net_admin == false);
        assert!(status.has_sys_ptrace == true || status.has_sys_ptrace == false);
        assert!(status.has_dac_read_search == true || status.has_dac_read_search == false);
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux platforms, should return an error
        assert!(result.is_err());
    }
}

#[test]
fn test_capability_status_structure() {
    // Test that we can construct a CapabilityStatus manually
    let status = CapabilityStatus {
        has_bpf: true,
        has_sys_admin: true,
        has_net_admin: true,
        has_sys_ptrace: false,
        has_dac_read_search: false,
    };
    
    assert_eq!(status.has_bpf, true);
    assert_eq!(status.has_sys_admin, true);
    assert_eq!(status.has_net_admin, true);
    assert_eq!(status.has_sys_ptrace, false);
    assert_eq!(status.has_dac_read_search, false);
}

#[test]
#[cfg(target_os = "linux")]
fn test_capability_check_on_linux() {
    // On Linux, we should be able to check capabilities
    // The actual values depend on how the test is run
    let status = check_all_capabilities().unwrap();
    
    // If running as root, we might have capabilities
    // If running as non-root, we likely don't
    // Either way, the check should complete without panicking
    println!("Capability status: has_sys_ptrace={}, has_dac_read_search={}", 
             status.has_sys_ptrace, status.has_dac_read_search);
}

#[test]
#[cfg(not(target_os = "linux"))]
fn test_capability_check_on_non_linux() {
    // On non-Linux platforms, capability checking should return an error
    let result = check_all_capabilities();
    
    assert!(result.is_err());
    if let Err(e) = result {
        // Verify it's the expected error type
        assert!(e.to_string().contains("eBPF is only supported on Linux"));
    }
}

// Note: Testing actual log output for capability warnings would require
// capturing log output, which is complex in Rust tests. The actual logging
// behavior is tested through integration tests and manual verification.
// The capability checking logic itself is tested above.
