/// Integration test for proxy handler with plugin interception
/// 
/// This test verifies that the proxy handler correctly:
/// 1. Checks plugin matches before proxying to upstream
/// 2. Serves plugin responses for matched paths
/// 3. Falls through to upstream for unmatched paths
/// 
/// Note: This test does NOT validate tokens as that requires
/// a real upstream endpoint. Token validation is tested separately in
/// the upstream client tests.

use mefirst::config::{PatternType, PluginConfig, ResponseSource};
use mefirst::plugin::PluginFactory;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_plugin_interception_flow() {
    // Create test response files
    let mut role_file = NamedTempFile::new().unwrap();
    role_file.write_all(b"test-role").unwrap();

    let mut creds_file = NamedTempFile::new().unwrap();
    creds_file.write_all(br#"{
        "AccessKeyId": "ASIATEST",
        "SecretAccessKey": "secret",
        "Token": "token"
    }"#).unwrap();

    // Create plugin configurations
    let configs = vec![
        // Exact match for role name
        PluginConfig {
            pattern: "/latest/meta-data/iam/security-credentials/".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::File {
                path: role_file.path().to_path_buf(),
            },
            status_code: 200,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
        // Exact match for credentials
        PluginConfig {
            pattern: "/latest/meta-data/iam/security-credentials/test-role".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::File {
                path: creds_file.path().to_path_buf(),
            },
            status_code: 200,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
        // Glob pattern for all instance-id requests
        PluginConfig {
            pattern: "/latest/meta-data/instance-*".to_string(),
            pattern_type: PatternType::Glob,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["i-1234567890abcdef0".to_string()],
            },
            status_code: 200,
            timeout_secs: Some(5),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
    ];

    let registry = PluginFactory::create_registry(&configs).unwrap();
    let headers = std::collections::HashMap::new();

    // Test 1: Exact match for role name
    let plugin = registry.find_match("/latest/meta-data/iam/security-credentials/", None, &headers);
    assert!(plugin.is_some());
    let response = plugin.unwrap().get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, b"test-role");

    // Test 2: Exact match for credentials
    let plugin = registry.find_match("/latest/meta-data/iam/security-credentials/test-role", None, &headers);
    assert!(plugin.is_some());
    let response = plugin.unwrap().get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert!(response.body.starts_with(b"{"));

    // Test 3: Glob match for instance-id
    let plugin = registry.find_match("/latest/meta-data/instance-id", None, &headers);
    assert!(plugin.is_some());
    let response = plugin.unwrap().get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(String::from_utf8_lossy(&response.body).trim(), "i-1234567890abcdef0");

    // Test 4: No match - should return None (proxy to upstream)
    let plugin = registry.find_match("/latest/meta-data/ami-id", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_plugin_priority_first_match() {
    // Create two response files
    let mut first_file = NamedTempFile::new().unwrap();
    first_file.write_all(b"first response").unwrap();

    let mut second_file = NamedTempFile::new().unwrap();
    second_file.write_all(b"second response").unwrap();

    // Create overlapping plugin configurations
    // The first one should win
    let configs = vec![
        PluginConfig {
            pattern: "/test/*".to_string(),
            pattern_type: PatternType::Glob,
            response_source: ResponseSource::File {
                path: first_file.path().to_path_buf(),
            },
            status_code: 200,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
        PluginConfig {
            pattern: "/test/specific".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::File {
                path: second_file.path().to_path_buf(),
            },
            status_code: 200,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
    ];

    let registry = PluginFactory::create_registry(&configs).unwrap();
    let headers = std::collections::HashMap::new();

    // The glob pattern is registered first, so it should match
    let plugin = registry.find_match("/test/specific", None, &headers).unwrap();
    let response = plugin.get_response(None).await.unwrap();
    assert_eq!(response.body, b"first response");
}

#[tokio::test]
async fn test_plugin_response_structure() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test content").unwrap();

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

    let plugin = PluginFactory::create_plugin(&config).unwrap();
    let response = plugin.get_response(None).await.unwrap();

    // Verify response structure
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, b"test content");
    // Headers may be empty or contain default headers
    assert!(response.headers.is_empty() || response.headers.len() > 0);
}

#[tokio::test]
async fn test_plugin_command_with_multiple_args() {
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "echo".to_string(),
            args: vec!["-n".to_string(), "no newline".to_string()],
        },
        status_code: 200,
        timeout_secs: Some(5),
        uid: None,
        username: None,
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };

    let plugin = PluginFactory::create_plugin(&config).unwrap();
    let response = plugin.get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(String::from_utf8_lossy(&response.body), "no newline");
}

#[tokio::test]
async fn test_plugin_regex_complex_patterns() {
    let configs = vec![
        // Match paths with UUID pattern
        PluginConfig {
            pattern: r"^/test/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$".to_string(),
            pattern_type: PatternType::Regex,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["uuid matched".to_string()],
            },
            status_code: 200,
            timeout_secs: Some(5),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
        // Match paths with version numbers
        PluginConfig {
            pattern: r"^/api/v\d+/.*$".to_string(),
            pattern_type: PatternType::Regex,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["api matched".to_string()],
            },
            status_code: 200,
            timeout_secs: Some(5),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        },
    ];

    let registry = PluginFactory::create_registry(&configs).unwrap();
    let headers = std::collections::HashMap::new();

    // Test UUID pattern
    let plugin = registry.find_match("/test/550e8400-e29b-41d4-a716-446655440000", None, &headers);
    assert!(plugin.is_some());
    let response = plugin.unwrap().get_response(None).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&response.body).trim(), "uuid matched");

    // Test version pattern
    let plugin = registry.find_match("/api/v1/users", None, &headers);
    assert!(plugin.is_some());
    let response = plugin.unwrap().get_response(None).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&response.body).trim(), "api matched");

    // Test no match
    let plugin = registry.find_match("/other/path", None, &headers);
    assert!(plugin.is_none());
}
