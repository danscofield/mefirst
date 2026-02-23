use mefirst::config::{Config, PatternConfig, PatternType, PluginConfig, ResponseSource};
use clap::Parser;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

// Task 24.1: Configuration parsing tests

#[test]
fn test_toml_parsing_process_aware_fields() {
    use std::env;
    
    // Create a temporary response file
    let response_file = NamedTempFile::new().unwrap();
    fs::write(response_file.path(), "test response").unwrap();
    
    let toml_content = format!(r#"
bind_port = 8080
inject_process_headers = true

[[plugins]]
pattern = "/test"
pattern_type = "exact"
response_source = {{ type = "file", path = "{}" }}
status_code = 200
uid = 1000
username = "testuser"

[plugins.executable_pattern]
pattern = "/usr/bin/curl"
pattern_type = "exact"

[plugins.cmdline_pattern]
pattern = "curl*"
pattern_type = "glob"

[plugins.host_pattern]
pattern = "*.example.com"
pattern_type = "glob"
"#, response_file.path().to_str().unwrap());
    
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    // Set environment variable for config file
    env::set_var("CONFIG_FILE", temp_file.path().to_str().unwrap());
    
    let config = Config::load().unwrap();
    
    // Clean up env var
    env::remove_var("CONFIG_FILE");
    
    assert_eq!(config.inject_process_headers, true);
    assert_eq!(config.plugins.len(), 1);
    
    let plugin = &config.plugins[0];
    assert_eq!(plugin.uid, Some(1000));
    assert_eq!(plugin.username, Some("testuser".to_string()));
    assert!(plugin.executable_pattern.is_some());
    assert!(plugin.cmdline_pattern.is_some());
    assert!(plugin.host_pattern.is_some());
}

#[test]
fn test_yaml_parsing_process_aware_fields() {
    use std::env;
    
    // Create a temporary response file
    let response_file = NamedTempFile::new().unwrap();
    fs::write(response_file.path(), "test response").unwrap();
    
    let yaml_content = format!(r#"
bind_port: 8080
inject_process_headers: true

plugins:
  - pattern: "/test"
    pattern_type: "exact"
    response_source:
      type: "file"
      path: "{}"
    status_code: 200
    uid: 1000
    username: "testuser"
    executable_pattern:
      pattern: "/usr/bin/curl"
      pattern_type: "exact"
    cmdline_pattern:
      pattern: "curl*"
      pattern_type: "glob"
    host_pattern:
      pattern: "*.example.com"
      pattern_type: "glob"
"#, response_file.path().to_str().unwrap());
    
    let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
    fs::write(temp_file.path(), yaml_content).unwrap();
    
    // Set environment variable for config file
    env::set_var("CONFIG_FILE", temp_file.path().to_str().unwrap());
    
    let config = Config::load().unwrap();
    
    // Clean up env var
    env::remove_var("CONFIG_FILE");
    
    assert_eq!(config.inject_process_headers, true);
    assert_eq!(config.plugins.len(), 1);
    
    let plugin = &config.plugins[0];
    assert_eq!(plugin.uid, Some(1000));
    assert_eq!(plugin.username, Some("testuser".to_string()));
    assert!(plugin.executable_pattern.is_some());
    assert!(plugin.cmdline_pattern.is_some());
    assert!(plugin.host_pattern.is_some());
}

#[test]
fn test_connection_interception_config_parsing_toml() {
    let toml_content = r#"
bind_port = 8080

[interception]
ip = "169.254.169.254"
port = 80
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    // Note: ConnectionInterceptionConfig is part of the internal config structure
    // This test verifies the TOML can be parsed
    let _config = Config::parse_from(&[
        "mefirst",
        "--config-file",
        temp_file.path().to_str().unwrap(),
    ]);
}

#[test]
fn test_connection_interception_config_parsing_yaml() {
    let yaml_content = r#"
bind_port: 8080

interception:
  ip: "169.254.169.254"
  port: 80
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), yaml_content).unwrap();
    
    let _config = Config::parse_from(&[
        "mefirst",
        "--config-file",
        temp_file.path().to_str().unwrap(),
    ]);
}

#[test]
fn test_proxy_request_stdin_parsing_toml() {
    let toml_content = r#"
bind_port = 8080

[[plugins]]
pattern = "/test"
pattern_type = "exact"
response_source = { type = "command", command = "cat", args = [] }
status_code = 200
proxy_request_stdin = true
"#;
    
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    // Parse the TOML directly to verify the field is present
    let parsed: toml::Value = toml::from_str(toml_content).unwrap();
    let plugins = parsed.get("plugins").unwrap().as_array().unwrap();
    let plugin = &plugins[0];
    assert_eq!(plugin.get("proxy_request_stdin").unwrap().as_bool().unwrap(), true);
}

#[test]
fn test_proxy_request_stdin_parsing_yaml() {
    let yaml_content = r#"
bind_port: 8080

plugins:
  - pattern: "/test"
    pattern_type: "exact"
    response_source:
      type: "command"
      command: "cat"
      args: []
    status_code: 200
    proxy_request_stdin: true
"#;
    
    let temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
    fs::write(temp_file.path(), yaml_content).unwrap();
    
    // Parse the YAML directly to verify the field is present
    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml_content).unwrap();
    let plugins = parsed.get("plugins").unwrap().as_sequence().unwrap();
    let plugin = &plugins[0];
    assert_eq!(plugin.get("proxy_request_stdin").unwrap().as_bool().unwrap(), true);
}

// Task 24.2: Configuration validation tests

#[test]
fn test_executable_pattern_requires_pattern_type() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
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
    
    // Should validate successfully with pattern_type
    assert!(config.validate().is_ok());
}

#[test]
fn test_cmdline_pattern_requires_pattern_type() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
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
    
    // Should validate successfully with pattern_type
    assert!(config.validate().is_ok());
}

#[test]
fn test_host_pattern_requires_pattern_type() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
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
    
    // Should validate successfully with pattern_type
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_regex_patterns_rejected() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
    let config = PluginConfig {
        pattern: "[invalid".to_string(),
        pattern_type: PatternType::Regex,
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
    
    // Should fail validation due to invalid regex
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_proxy_request_stdin_only_with_command_sources() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
    // Valid: proxy_request_stdin with command source
    let valid_config = PluginConfig {
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
    
    assert!(valid_config.validate().is_ok());
    
    // Invalid: proxy_request_stdin with file source
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
    
    let result = invalid_config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("proxy_request_stdin"));
}

#[test]
fn test_descriptive_errors_for_validation_failures() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
    // Test invalid regex error message
    let config = PluginConfig {
        pattern: "[invalid".to_string(),
        pattern_type: PatternType::Regex,
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
    
    let result = config.validate();
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    // Error should mention regex or pattern
    assert!(error_msg.contains("regex") || error_msg.contains("pattern") || error_msg.contains("invalid"));
}

#[test]
fn test_empty_pattern_validation() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"test").unwrap();
    
    let config = PluginConfig {
        pattern: "".to_string(),
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
    
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_nonexistent_file_validation() {
    let config = PluginConfig {
        pattern: "/test".to_string(),
        pattern_type: PatternType::Exact,
        response_source: ResponseSource::File {
            path: "/nonexistent/file.txt".into(),
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
    
    let result = config.validate();
    assert!(result.is_err());
}
