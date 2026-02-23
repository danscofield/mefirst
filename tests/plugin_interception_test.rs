use mefirst::config::{PatternType, PluginConfig, ResponseSource};
use mefirst::plugin::PluginFactory;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_plugin_registry_from_config() {
    // Create a temporary response file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test response").unwrap();

    let configs = vec![
        PluginConfig {
            pattern: "/test/exact".to_string(),
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
        },
        PluginConfig {
            pattern: "/test/glob/*".to_string(),
            pattern_type: PatternType::Glob,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
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
    assert_eq!(registry.len(), 2);

    let headers = std::collections::HashMap::new();

    // Test exact match
    let plugin = registry.find_match("/test/exact", None, &headers);
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().pattern(), "/test/exact");

    // Test glob match
    let plugin = registry.find_match("/test/glob/something", None, &headers);
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().pattern(), "/test/glob/*");

    // Test no match
    let plugin = registry.find_match("/other/path", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_plugin_response_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"file content").unwrap();

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
    assert!(plugin.matches("/test"));

    let response = plugin.get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, b"file content");
}

#[tokio::test]
async fn test_plugin_response_command() {
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::Command {
            command: "echo".to_string(),
            args: vec!["hello world".to_string()],
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
    assert!(plugin.matches("/test"));

    let response = plugin.get_response(None).await.unwrap();
    assert_eq!(response.status_code, 200);
    // echo adds a newline
    assert_eq!(String::from_utf8_lossy(&response.body).trim(), "hello world");
}

#[tokio::test]
async fn test_plugin_first_match_routing() {
    let mut temp_file1 = NamedTempFile::new().unwrap();
    temp_file1.write_all(b"first").unwrap();

    let mut temp_file2 = NamedTempFile::new().unwrap();
    temp_file2.write_all(b"second").unwrap();

    let configs = vec![
        PluginConfig {
            pattern: "/test/*".to_string(),
            pattern_type: PatternType::Glob,
            response_source: ResponseSource::File {
                path: temp_file1.path().to_path_buf(),
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
                path: temp_file2.path().to_path_buf(),
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

    // First plugin should match (glob pattern)
    let plugin = registry.find_match("/test/specific", None, &headers).unwrap();
    let response = plugin.get_response(None).await.unwrap();
    assert_eq!(response.body, b"first");
}

#[tokio::test]
async fn test_plugin_custom_status_code() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"not found").unwrap();

    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: temp_file.path().to_path_buf(),
        },
        status_code: 404,
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
    assert_eq!(response.status_code, 404);
}

#[tokio::test]
async fn test_plugin_regex_pattern() {
    let config = PluginConfig {
        pattern: r"^/test/\d+$".to_string(),
        pattern_type: PatternType::Regex,
        response_source: ResponseSource::Command {
            command: "echo".to_string(),
            args: vec!["matched".to_string()],
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

    // Should match paths with numbers
    assert!(plugin.matches("/test/123"));
    assert!(plugin.matches("/test/456"));

    // Should not match paths without numbers
    assert!(!plugin.matches("/test/abc"));
    assert!(!plugin.matches("/test/"));
}
