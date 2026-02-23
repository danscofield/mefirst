/// Integration test to verify config file loading works correctly
/// This test verifies that the example config files are valid and can be parsed

use std::fs;

#[test]
fn test_example_toml_config_is_valid() {
    let content = fs::read_to_string("examples/config.toml")
        .expect("Failed to read examples/config.toml");
    
    // Parse as TOML to verify it's valid
    let _config: toml::Value = toml::from_str(&content)
        .expect("Failed to parse examples/config.toml as valid TOML");
}

#[test]
fn test_example_yaml_config_is_valid() {
    let content = fs::read_to_string("examples/config.yaml")
        .expect("Failed to read examples/config.yaml");
    
    // Parse as YAML to verify it's valid
    let _config: serde_yaml::Value = serde_yaml::from_str(&content)
        .expect("Failed to parse examples/config.yaml as valid YAML");
}

#[test]
fn test_external_api_config_is_valid() {
    let content = fs::read_to_string("examples/config-external-api.toml")
        .expect("Failed to read examples/config-external-api.toml");
    
    // Parse as TOML to verify it's valid
    let config: toml::Value = toml::from_str(&content)
        .expect("Failed to parse examples/config-external-api.toml as valid TOML");
    
    // Verify it has the required fields for external-api mode
    assert_eq!(config.get("provider_mode").and_then(|v| v.as_str()), Some("external-api"));
    assert!(config.get("endpoint").is_some());
}
