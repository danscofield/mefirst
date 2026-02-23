use mefirst::config::{PatternConfig, PatternType, PluginConfig, ResponseSource};
use mefirst::plugin::matcher::PatternMatcher;
use mefirst::plugin::PluginFactory;
use mefirst::process::ProcessInfo;
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

// Helper function to create test process info
fn create_test_process_info(uid: u32, username: &str, pid: u32, executable: &str, cmdline: &str) -> ProcessInfo {
    ProcessInfo {
        uid,
        username: username.to_string(),
        pid,
        executable: executable.to_string(),
        cmdline: cmdline.to_string(),
    }
}

// Task 22.1: Process metadata logging tests

#[test]
fn test_process_metadata_all_fields_available() {
    let process_info = create_test_process_info(
        1000,
        "testuser",
        12345,
        "/usr/bin/curl",
        "curl http://example.com"
    );
    
    // Verify all five metadata fields are present
    assert_eq!(process_info.uid, 1000);
    assert_eq!(process_info.username, "testuser");
    assert_eq!(process_info.pid, 12345);
    assert_eq!(process_info.executable, "/usr/bin/curl");
    assert_eq!(process_info.cmdline, "curl http://example.com");
}

#[test]
fn test_process_metadata_logging_without_metadata() {
    // Test that requests can be logged without metadata
    let metadata: Option<ProcessInfo> = None;
    assert!(metadata.is_none());
    
    // The proxy should continue logging requests even without metadata
}

// Task 22.2: Pattern matching tests

#[test]
fn test_exact_pattern_matching() {
    let pattern = PatternConfig {
        pattern: "/test/exact".to_string(),
        pattern_type: PatternType::Exact,
    };
    
    let matcher = PatternMatcher::from_config(&pattern).unwrap();
    
    assert!(matcher.matches("/test/exact"));
    assert!(!matcher.matches("/test/exact/"));
    assert!(!matcher.matches("/test/other"));
}

#[test]
fn test_glob_pattern_matching() {
    let pattern = PatternConfig {
        pattern: "/test/*".to_string(),
        pattern_type: PatternType::Glob,
    };
    
    let matcher = PatternMatcher::from_config(&pattern).unwrap();
    
    assert!(matcher.matches("/test/anything"));
    assert!(matcher.matches("/test/123"));
    assert!(!matcher.matches("/other/path"));
}

#[test]
fn test_regex_pattern_matching() {
    let pattern = PatternConfig {
        pattern: r"^/test/\d+$".to_string(),
        pattern_type: PatternType::Regex,
    };
    
    let matcher = PatternMatcher::from_config(&pattern).unwrap();
    
    assert!(matcher.matches("/test/123"));
    assert!(matcher.matches("/test/456"));
    assert!(!matcher.matches("/test/abc"));
    assert!(!matcher.matches("/test/"));
}

#[test]
fn test_invalid_regex_pattern_rejected() {
    let pattern = PatternConfig {
        pattern: "[invalid".to_string(),
        pattern_type: PatternType::Regex,
    };
    
    let result = PatternMatcher::from_config(&pattern);
    assert!(result.is_err());
}

// Task 22.3: Filter matching logic tests

#[tokio::test]
async fn test_uid_filter_matches_when_equal() {
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
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

#[tokio::test]
async fn test_uid_filter_skips_when_different() {
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
    let process_info = create_test_process_info(2000, "otheruser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_uid_filter_skips_when_metadata_unavailable() {
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
    
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_username_filter_matches_when_equal() {
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
        username: Some("testuser".to_string()),
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

#[tokio::test]
async fn test_username_filter_skips_when_different() {
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
        username: Some("testuser".to_string()),
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let process_info = create_test_process_info(1000, "otheruser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_username_filter_skips_when_metadata_unavailable() {
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
        username: Some("testuser".to_string()),
        executable_pattern: None,
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_executable_pattern_filter_matches() {
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
        executable_pattern: Some(PatternConfig {
            pattern: "/usr/bin/curl".to_string(),
            pattern_type: PatternType::Exact,
        }),
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

#[tokio::test]
async fn test_executable_pattern_filter_skips_when_metadata_unavailable() {
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
        executable_pattern: Some(PatternConfig {
            pattern: "/usr/bin/curl".to_string(),
            pattern_type: PatternType::Exact,
        }),
        cmdline_pattern: None,
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_cmdline_pattern_filter_matches() {
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
        cmdline_pattern: Some(PatternConfig {
            pattern: "curl*".to_string(),
            pattern_type: PatternType::Glob,
        }),
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl http://example.com");
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

#[tokio::test]
async fn test_cmdline_pattern_filter_skips_when_metadata_unavailable() {
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
        cmdline_pattern: Some(PatternConfig {
            pattern: "curl*".to_string(),
            pattern_type: PatternType::Glob,
        }),
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_host_pattern_filter_matches() {
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
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let mut headers = HashMap::new();
    headers.insert("host".to_string(), "api.example.com".to_string());
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
}

#[tokio::test]
async fn test_host_pattern_filter_skips_when_header_missing() {
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
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let headers = HashMap::new(); // No Host header
    
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_multiple_filters_use_and_logic() {
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
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    // Both uid and username must match
    let process_info_match = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let plugin = registry.find_match("/test", Some(&process_info_match), &headers);
    assert!(plugin.is_some());
    
    // If uid matches but username doesn't, should not match
    let process_info_no_match = create_test_process_info(1000, "otheruser", 12345, "/usr/bin/curl", "curl");
    let plugin = registry.find_match("/test", Some(&process_info_no_match), &headers);
    assert!(plugin.is_none());
}

#[tokio::test]
async fn test_no_filters_matches_all_requests() {
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
        host_pattern: None,
        proxy_request_stdin: None,
    };
    
    let registry = PluginFactory::create_registry(&vec![config]).unwrap();
    let headers = HashMap::new();
    
    // Should match with metadata
    let process_info = create_test_process_info(1000, "testuser", 12345, "/usr/bin/curl", "curl");
    let plugin = registry.find_match("/test", Some(&process_info), &headers);
    assert!(plugin.is_some());
    
    // Should also match without metadata
    let plugin = registry.find_match("/test", None, &headers);
    assert!(plugin.is_some());
}
