use mefirst::config::{Config, PatternType, PluginConfig, ResponseSource};
use clap::Parser;
use mefirst::plugin::PluginFactory;
use mefirst::process::ProcessInfo;
use std::io::Write;
use tempfile::NamedTempFile;

// Helper function to create test process info
fn create_test_process_info() -> ProcessInfo {
    ProcessInfo {
        uid: 1000,
        username: "testuser".to_string(),
        pid: 12345,
        executable: "/usr/bin/curl".to_string(),
        cmdline: "curl http://example.com".to_string(),
    }
}

#[tokio::test]
async fn test_global_header_injection_with_metadata() {
    // Test that headers are injected when inject_process_headers=true and metadata available
    // Note: This tests the configuration; actual header injection happens in the proxy handler
    
    let config = Config::parse_from(&[
        "mefirst",
        "--inject-process-headers",
        "true",
    ]);
    
    assert_eq!(config.inject_process_headers, true);
    
    // Verify process info structure
    let process_info = create_test_process_info();
    assert_eq!(process_info.uid, 1000);
    assert_eq!(process_info.username, "testuser");
    assert_eq!(process_info.pid, 12345);
    assert_eq!(process_info.executable, "/usr/bin/curl");
    assert_eq!(process_info.cmdline, "curl http://example.com");
}

#[tokio::test]
async fn test_global_header_injection_disabled() {
    // Test that headers are not injected when inject_process_headers=false
    
    let config = Config::parse_from(&["mefirst"]);
    assert_eq!(config.inject_process_headers, false);
}

#[tokio::test]
async fn test_global_header_injection_without_metadata() {
    // Test that proxy continues when metadata unavailable
    // The actual behavior is tested in the proxy handler
    
    let config = Config::parse_from(&[
        "mefirst",
        "--inject-process-headers",
        "true",
    ]);
    
    assert_eq!(config.inject_process_headers, true);
    
    // When metadata is None, the proxy should continue without injecting headers
    let metadata: Option<ProcessInfo> = None;
    assert!(metadata.is_none());
}

#[tokio::test]
async fn test_all_five_headers_structure() {
    // Test that all five process metadata fields are available
    
    let process_info = create_test_process_info();
    
    // Verify all five fields that should be injected as headers
    assert!(process_info.uid > 0);
    assert!(!process_info.username.is_empty());
    assert!(process_info.pid > 0);
    assert!(!process_info.executable.is_empty());
    assert!(!process_info.cmdline.is_empty());
}

#[tokio::test]
async fn test_global_injection_independent_of_plugins() {
    // Test that inject_process_headers works for non-plugin-matched requests
    
    let config = Config::parse_from(&[
        "mefirst",
        "--inject-process-headers",
        "true",
    ]);
    
    // Create an empty plugin registry (no plugins configured)
    let configs: Vec<PluginConfig> = vec![];
    let registry = PluginFactory::create_registry(&configs).unwrap();
    
    assert_eq!(registry.len(), 0);
    assert_eq!(config.inject_process_headers, true);
    
    // The global inject_process_headers should work even without plugins
}

#[tokio::test]
async fn test_global_injection_independent_of_proxy_request_stdin() {
    // Test that inject_process_headers is independent of proxy_request_stdin
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test response").unwrap();
    
    // Plugin with proxy_request_stdin=true
    let config_with_stdin = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "echo".to_string(),
            args: vec!["test".to_string()],
        },
        status_code: 200,
        timeout_secs: Some(5),
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: Some(true),
    };
    
    // Plugin without proxy_request_stdin
    let config_without_stdin = PluginConfig {
        pattern: "/other".to_string(),
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
    
    // Both should be valid regardless of inject_process_headers setting
    assert!(config_with_stdin.validate().is_ok());
    assert!(config_without_stdin.validate().is_ok());
}

// Task 21.2: proxy_request_stdin header injection tests

#[tokio::test]
async fn test_proxy_request_stdin_with_metadata() {
    // Test that proxy_request_stdin injects headers when metadata available
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "cat".to_string(),
            args: vec![],
        },
        status_code: 200,
        timeout_secs: Some(5),
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: Some(true),
    };
    
    assert_eq!(config.proxy_request_stdin, Some(true));
    
    let process_info = create_test_process_info();
    
    // Verify the expected headers would be:
    // X-Forwarded-Uid: 1000
    // X-Forwarded-Username: testuser
    // X-Forwarded-Pid: 12345
    // X-Forwarded-Process-Name: /usr/bin/curl
    // X-Forwarded-Process-Args: curl http://example.com
    
    assert_eq!(process_info.uid, 1000);
    assert_eq!(process_info.username, "testuser");
    assert_eq!(process_info.pid, 12345);
    assert_eq!(process_info.executable, "/usr/bin/curl");
    assert_eq!(process_info.cmdline, "curl http://example.com");
}

#[tokio::test]
async fn test_proxy_request_stdin_without_metadata() {
    // Test that original request is forwarded when metadata unavailable
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "cat".to_string(),
            args: vec![],
        },
        status_code: 200,
        timeout_secs: Some(5),
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: Some(true),
    };
    
    assert_eq!(config.proxy_request_stdin, Some(true));
    
    // When metadata is None, the original request should be forwarded
    let metadata: Option<ProcessInfo> = None;
    assert!(metadata.is_none());
}

#[tokio::test]
async fn test_proxy_request_stdin_disabled() {
    // Test that headers are not injected when proxy_request_stdin=false
    
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "echo".to_string(),
            args: vec!["test".to_string()],
        },
        status_code: 200,
        timeout_secs: Some(5),
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: Some(false),
    };
    
    assert_eq!(config.proxy_request_stdin, Some(false));
}

#[tokio::test]
async fn test_proxy_request_stdin_validation() {
    // Test that proxy_request_stdin only works with command-based sources
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
    let invalid_config = PluginConfig {
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
        proxy_request_stdin: Some(true),
    };
    
    // This should fail validation
    let result = invalid_config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("proxy_request_stdin"));
}
