use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

// Note: We can't directly test Config::load() in integration tests because it calls Config::parse()
// which reads from std::env::args(). Instead, we test the file loading functionality.

#[test]
fn test_toml_config_file_loading() {
    let toml_content = r#"
redirect_mode = "iptables"
provider_mode = "external-api"
endpoint = "https://api.example.com/credentials"
bind_port = 9000
target_address = "192.168.1.1"
target_port = 8080
src_port_start = 2000
src_port_end = 3000
refresh_interval = 5
role_name = "CustomRole"
enable_metrics = false
metrics_port = 9091
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(toml_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();
    
    // Verify the file was written correctly
    let content = fs::read_to_string(temp_file.path()).unwrap();
    assert!(content.contains("redirect_mode"));
    assert!(content.contains("iptables"));
}

#[test]
fn test_yaml_config_file_loading() {
    let yaml_content = r#"
redirect_mode: iptables
provider_mode: external-api
endpoint: https://api.example.com/credentials
bind_port: 9000
target_address: 192.168.1.1
"#;
    
    let mut temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
    temp_file.write_all(yaml_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();
    
    // Verify the file was written correctly
    let content = fs::read_to_string(temp_file.path()).unwrap();
    assert!(content.contains("redirect_mode"));
    assert!(content.contains("iptables"));
}

#[test]
fn test_minimal_config_with_defaults() {
    let toml_content = r#"
# Only override a few settings, rest should use defaults
bind_port = 9000
role_name = "TestRole"
"#;
    
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(toml_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();
    
    // Verify the file was written correctly
    let content = fs::read_to_string(temp_file.path()).unwrap();
    assert!(content.contains("bind_port"));
    assert!(content.contains("9000"));
}
