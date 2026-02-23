use mefirst::config::Config;
use clap::Parser;
use std::env;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_inject_process_headers_defaults_to_false() {
    let config = Config::parse_from(&["mefirst"]);
    assert_eq!(config.inject_process_headers, false);
}

#[test]
fn test_inject_process_headers_via_config_file_toml() {
    let toml_content = r#"
bind_port = 8080
inject_process_headers = true
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    let config = Config::parse_from(&[
        "mefirst",
        "--config",
        temp_file.path().to_str().unwrap(),
    ]);
    
    assert_eq!(config.inject_process_headers, true);
}

#[test]
fn test_inject_process_headers_via_config_file_yaml() {
    let yaml_content = r#"
bind_port: 8080
inject_process_headers: true
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), yaml_content).unwrap();
    
    let config = Config::parse_from(&[
        "mefirst",
        "--config",
        temp_file.path().to_str().unwrap(),
    ]);
    
    assert_eq!(config.inject_process_headers, true);
}

#[test]
fn test_inject_process_headers_via_cli_argument() {
    let config = Config::parse_from(&[
        "mefirst",
        "--inject-process-headers",
        "true",
    ]);
    
    assert_eq!(config.inject_process_headers, true);
    
    let config = Config::parse_from(&[
        "mefirst",
        "--inject-process-headers",
        "false",
    ]);
    
    assert_eq!(config.inject_process_headers, false);
}

#[test]
fn test_inject_process_headers_via_environment_variable() {
    env::set_var("INJECT_PROCESS_HEADERS", "true");
    
    let config = Config::parse_from(&["mefirst"]);
    assert_eq!(config.inject_process_headers, true);
    
    env::remove_var("INJECT_PROCESS_HEADERS");
}

#[test]
fn test_cli_argument_overrides_config_file() {
    let toml_content = r#"
bind_port = 8080
inject_process_headers = false
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    let config = Config::parse_from(&[
        "mefirst",
        "--config",
        temp_file.path().to_str().unwrap(),
        "--inject-process-headers",
        "true",
    ]);
    
    assert_eq!(config.inject_process_headers, true);
}

#[test]
fn test_environment_variable_overrides_config_file() {
    let toml_content = r#"
bind_port = 8080
inject_process_headers = false
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), toml_content).unwrap();
    
    env::set_var("INJECT_PROCESS_HEADERS", "true");
    
    let config = Config::parse_from(&[
        "mefirst",
        "--config",
        temp_file.path().to_str().unwrap(),
    ]);
    
    assert_eq!(config.inject_process_headers, true);
    
    env::remove_var("INJECT_PROCESS_HEADERS");
}
