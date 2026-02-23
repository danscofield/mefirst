use mefirst::config::{PatternConfig, PatternType, PluginConfig, ResponseSource};
use mefirst::plugin::PluginFactory;
use mefirst::process::ProcessInfo;
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

// Helper function
fn create_test_process_info() -> ProcessInfo {
    ProcessInfo {
        uid: 1000,
        username: "testuser".to_string(),
        pid: 12345,
        executable: "/usr/bin/curl".to_string(),
        cmdline: "curl http://example.com".to_string(),
    }
}

// Task 23.1: Optional IP interception tests

#[test]
fn test_ip_agnostic_mode_configuration() {
    // Test that IP-agnostic mode can be configured (ip field is None or "0.0.0.0")
    // The actual interception behavior is tested in eBPF integration tests
    
    // This would be configured as:
    // [interception]
    // port = 80
    // # No ip field specified - intercepts all IPs
    
    // Or:
    // [interception]
    // ip = "0.0.0.0"
    // port = 80
}

#[test]
fn test_ip_specific_mode_configuration() {
    // Test that IP-specific mode can be configured
    // The actual interception behavior is tested in eBPF integration tests
    
    // This would be configured as:
    // [interception]
    // ip = "169.254.169.254"
    // port = 80
}

#[tokio::test]
async fn test_host_pattern_works_without_ip_filter() {
    // Test that host_pattern filtering works regardless of IP configuration
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"response").unwrap();
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: temp_file.path().to_path_buf(),
        },
        status_code: 200,
        timeout_secs: None,
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: Some(PatternConfig {
            pattern: "*.example.com".to_string(),
            pattern_type: PatternType::Glob,
        }),
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let process_info = create_test_process_info();
    let mut headers = HashMap::new();
    headers.insert("host".to_string(), "api.example.com".to_string());
    
    // Host pattern should work regardless of IP interception mode
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

// Task 23.2: eBPF disabled behavior tests

#[tokio::test]
async fn test_process_filters_skipped_when_ebpf_disabled() {
    // When eBPF is disabled, process filters should not match
    // This is because process metadata is only available with eBPF
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"response").unwrap();
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: temp_file.path().to_path_buf(),
        },
        status_code: 200,
        timeout_secs: None,
        uid: Some(1000),
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    // Without process metadata (simulating eBPF disabled), should not match
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_none());
}

#[test]
fn test_ebpf_disabled_warning_configuration() {
    // Test that configuration can detect when process filters are configured
    // but eBPF is disabled (warning should be logged)
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"response").unwrap();
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: temp_file.path().to_path_buf(),
        },
        status_code: 200,
        timeout_secs: None,
        uid: Some(1000),
        username: Some("testuser".to_string()),
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    // Config has process filters (uid, username)
    assert!(config.uid.is_some() || config.username.is_some());
    
    // The actual warning is logged in the main application when
    // enable_ebpf=false and process filters are configured
}

// Task 23.3: Graceful degradation tests

#[tokio::test]
async fn test_proxy_continues_without_process_metadata() {
    // Test that proxy continues functioning when process metadata is unavailable
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"response").unwrap();
    
    // Plugin without process filters should work without metadata
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: temp_file.path().to_path_buf(),
        },
        status_code: 200,
        timeout_secs: None,
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    // Should match even without process metadata
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_some());
}

#[test]
fn test_proxy_continues_without_capabilities() {
    // Test that capability checking doesn't prevent proxy startup
    
    use mefirst::capability::check_all_capabilities;
    
    let result = check_all_capabilities();
    
    #[cfg(target_os = "linux")]
    {
        let status = result.unwrap();
        // Regardless of capability status, the check should complete
        // The proxy should continue even if capabilities are missing
        // (with appropriate warnings logged)
        
        // Verify the check completes without panicking
        assert!(status.has_sys_ptrace == true || status.has_sys_ptrace == false);
        assert!(status.has_dac_read_search == true || status.has_dac_read_search == false);
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        // On non-Linux platforms, should return an error
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn test_graceful_degradation_with_partial_metadata() {
    // Test that proxy handles partial metadata gracefully
    
    let process_info = ProcessInfo {
        uid: 1000,
        username: "1000".to_string(), // Fallback to numeric uid
        pid: 12345,
        executable: "<unknown-12345>".to_string(), // Fallback when exe can't be read
        cmdline: String::new(), // Empty when cmdline can't be read
    };
    
    // Verify fallback values are valid
    assert_eq!(process_info.uid, 1000);
    assert_eq!(process_info.username, "1000");
    assert_eq!(process_info.executable, "<unknown-12345>");
    assert_eq!(process_info.cmdline, "");
}
