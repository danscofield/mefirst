use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// RedirectModeType removed - eBPF is the only supported mode

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    Exact,
    Glob,
    Regex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ResponseSource {
    File { path: PathBuf },
    Command { command: String, args: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub pattern: String,
    pub pattern_type: PatternType,
    pub response_source: ResponseSource,
    #[serde(default = "default_status_code")]
    pub status_code: u16,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    
    // Process-aware routing fields
    #[serde(default)]
    pub uid: Option<u32>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub executable_pattern: Option<PatternConfig>,
    #[serde(default)]
    pub cmdline_pattern: Option<PatternConfig>,
    #[serde(default)]
    pub host_pattern: Option<PatternConfig>,
    
    // Proxy request stdin feature
    #[serde(default)]
    pub proxy_request_stdin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternConfig {
    pub pattern: String,
    pub pattern_type: PatternType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInterceptionConfig {
    #[serde(default)]
    pub ip: Option<String>,
    pub port: u16,
}

impl Default for ConnectionInterceptionConfig {
    fn default() -> Self {
        Self {
            ip: Some("169.254.169.254".to_string()),
            port: 80,
        }
    }
}

fn default_status_code() -> u16 {
    200
}

/// Configuration file structure (for TOML/YAML loading)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[cfg(feature = "allow-external-ebpf")]
    #[serde(default)]
    pub ebpf_program_path: Option<PathBuf>,
    #[serde(default = "default_cgroup_path")]
    pub cgroup_path: PathBuf,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    
    // New interception config (preferred)
    #[serde(default)]
    pub interception: Option<ConnectionInterceptionConfig>,
    
    // Legacy fields for backward compatibility
    #[serde(default = "default_target_address")]
    pub target_address: String,
    #[serde(default = "default_target_port")]
    pub target_port: u16,
    
    #[serde(default = "default_bind_port")]
    pub bind_port: u16,
    #[serde(default = "default_enable_metrics")]
    pub enable_metrics: bool,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

// Default value functions for serde
fn default_cgroup_path() -> PathBuf {
    PathBuf::from("/sys/fs/cgroup")
}
fn default_target_address() -> String {
    "169.254.169.254".to_string()
}
fn default_target_port() -> u16 {
    80
}
fn default_bind_port() -> u16 {
    8080
}
fn default_enable_metrics() -> bool {
    true
}
fn default_metrics_port() -> u16 {
    9090
}

#[derive(Parser, Debug, Clone)]
#[command(name = "mefirst")]
#[command(about = "BPF-enabled intercepting HTTP proxy with plugin-based request interception", long_about = None)]
pub struct Config {
    /// Configuration file path (TOML or YAML)
    #[arg(short = 'c', long, env = "CONFIG_FILE")]
    pub config_file: Option<PathBuf>,

    /// Path to compiled eBPF object file (optional, uses embedded bytecode if not specified)
    /// NOTE: This option is only available when compiled with the 'allow-external-ebpf' feature
    #[cfg(feature = "allow-external-ebpf")]
    #[arg(long, env = "EBPF_PROGRAM_PATH")]
    pub ebpf_program_path: Option<PathBuf>,

    /// Cgroup path for eBPF attachment
    #[arg(long, default_value = "/sys/fs/cgroup", env = "CGROUP_PATH")]
    pub cgroup_path: PathBuf,

    /// Target address to intercept
    #[arg(short = 't', long, default_value = "169.254.169.254", env = "TARGET_ADDRESS")]
    pub target_address: String,

    /// Target port to intercept
    #[arg(short = 'T', long, default_value = "80", env = "TARGET_PORT")]
    pub target_port: u16,

    /// Bind port for proxy (always binds to localhost: 127.0.0.1 and ::1)
    #[arg(short = 'p', long, default_value = "8080", env = "BIND_PORT")]
    pub bind_port: u16,

    /// Enable metrics endpoint
    #[arg(long, default_value = "true", env = "ENABLE_METRICS")]
    pub enable_metrics: bool,

    /// Metrics port
    #[arg(long, default_value = "9090", env = "METRICS_PORT")]
    pub metrics_port: u16,
    
    /// Interception plugins (loaded from config file)
    #[arg(skip)]
    pub plugins: Vec<PluginConfig>,
}

impl Config {
    /// Load configuration with precedence: defaults < config file < env vars < CLI args
    pub fn load() -> crate::error::Result<Self> {
        // Parse CLI arguments first to get config file path
        let mut config = Config::parse();
        
        // If config file is specified, load and merge it
        if let Some(config_path) = &config.config_file {
            let file_config = Self::load_from_file(config_path)?;
            config.merge_from_file(file_config);
        }
        
        // Validate the final configuration
        config.validate()?;
        
        Ok(config)
    }
    
    /// Load configuration from a file (TOML or YAML)
    fn load_from_file(path: &PathBuf) -> crate::error::Result<ConfigFile> {
        use crate::error::InterposerError;
        use std::fs;
        
        let content = fs::read_to_string(path)
            .map_err(|e| InterposerError::Config(format!("Failed to read config file: {}", e)))?;
        
        // Try to determine format from extension
        let config: ConfigFile = if path.extension().and_then(|s| s.to_str()) == Some("yaml") 
            || path.extension().and_then(|s| s.to_str()) == Some("yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| InterposerError::Config(format!("Failed to parse YAML config: {}", e)))?
        } else {
            // Default to TOML
            toml::from_str(&content)
                .map_err(|e| InterposerError::Config(format!("Failed to parse TOML config: {}", e)))?
        };
        
        Ok(config)
    }
    
    /// Merge file configuration into CLI configuration
    /// CLI args and env vars take precedence over file values
    fn merge_from_file(&mut self, file_config: ConfigFile) {
        // Only use file values if CLI/env didn't provide them
        // For clap, we need to check if values are at their defaults
        
        // For ebpf_program_path, use file value if CLI didn't provide one
        #[cfg(feature = "allow-external-ebpf")]
        {
            if self.ebpf_program_path.is_none() && file_config.ebpf_program_path.is_some() {
                self.ebpf_program_path = file_config.ebpf_program_path;
            }
        }
        
        // For cgroup_path, check if it's still at default
        if self.cgroup_path == PathBuf::from("/sys/fs/cgroup") && file_config.cgroup_path != PathBuf::from("/sys/fs/cgroup") {
            self.cgroup_path = file_config.cgroup_path;
        }
        
        // Always load plugins from config file
        self.plugins = file_config.plugins;
        
        // Handle interception config with backward compatibility
        if let Some(interception) = file_config.interception {
            // New format: use interception config
            if let Some(ip) = interception.ip {
                if self.target_address == "169.254.169.254" {
                    self.target_address = ip;
                }
            } else {
                // No IP specified - use "0.0.0.0" to indicate IP-agnostic mode
                if self.target_address == "169.254.169.254" {
                    self.target_address = "0.0.0.0".to_string();
                }
            }
            if self.target_port == 80 {
                self.target_port = interception.port;
            }
        } else {
            // Legacy format: use target_address and target_port
            if self.target_address == "169.254.169.254" && file_config.target_address != "169.254.169.254" {
                self.target_address = file_config.target_address;
            }
            
            if self.target_port == 80 && file_config.target_port != 80 {
                self.target_port = file_config.target_port;
            }
        }
        
        if self.bind_port == 8080 && file_config.bind_port != 8080 {
            self.bind_port = file_config.bind_port;
        }
        
        if self.enable_metrics && !file_config.enable_metrics {
            self.enable_metrics = file_config.enable_metrics;
        }
        
        if self.metrics_port == 9090 && file_config.metrics_port != 9090 {
            self.metrics_port = file_config.metrics_port;
        }
    }
    
    pub fn validate(&self) -> crate::error::Result<()> {
        use crate::error::InterposerError;
        
        // Validate plugin configurations
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if plugin.pattern.is_empty() {
                return Err(InterposerError::Config(
                    format!("Plugin {} has empty pattern", idx),
                ));
            }
            
            // Validate response source
            match &plugin.response_source {
                ResponseSource::File { path } => {
                    if !path.exists() {
                        return Err(InterposerError::Config(
                            format!("Plugin {} response file does not exist: {:?}", idx, path),
                        ));
                    }
                }
                ResponseSource::Command { command, .. } => {
                    if command.is_empty() {
                        return Err(InterposerError::Config(
                            format!("Plugin {} has empty command", idx),
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::parse_from(&["mefirst"]);
        assert_eq!(config.bind_port, 8080);
        assert_eq!(config.target_address, "169.254.169.254");
        assert_eq!(config.target_port, 80);
        assert!(config.plugins.is_empty());
    }

    #[test]
    fn test_cli_override() {
        let config = Config::parse_from(&[
            "mefirst",
            "--bind-port", "9000",
        ]);
        assert_eq!(config.bind_port, 9000);
    }

    #[test]
    fn test_load_toml_config() {
        let toml_content = r#"
bind_port = 9000
target_address = "192.168.1.1"
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();
        
        let config_file = Config::load_from_file(&path).unwrap();
        assert_eq!(config_file.bind_port, 9000);
        assert_eq!(config_file.target_address, "192.168.1.1");
    }

    #[test]
    fn test_load_yaml_config() {
        let yaml_content = r#"
bind_port: 9000
"#;
        
        let mut temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        temp_file.write_all(yaml_content.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();
        
        let config_file = Config::load_from_file(&path).unwrap();
        assert_eq!(config_file.bind_port, 9000);
    }

    #[test]
    fn test_config_file_defaults() {
        let toml_content = r#"
# Minimal config with only one override
bind_port = 9000
"#;
        
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(toml_content.as_bytes()).unwrap();
        let path = temp_file.path().to_path_buf();
        
        let config_file = Config::load_from_file(&path).unwrap();
        // Check that defaults are applied
        assert_eq!(config_file.bind_port, 9000); // Overridden
        assert_eq!(config_file.target_address, "169.254.169.254"); // Default
    }
}
