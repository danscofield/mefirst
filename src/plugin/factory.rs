use crate::config::{PluginConfig, ResponseSource};
use crate::error::{InterposerError, Result};
use crate::plugin::command::CommandPlugin;
use crate::plugin::file::FilePlugin;
use crate::plugin::{InterceptionPlugin, PluginRegistry, ProcessAwarePlugin};
use tracing::{debug, info};

/// Factory for creating plugins from configuration
pub struct PluginFactory;

impl PluginFactory {
    /// Create a plugin from configuration
    pub fn create_plugin(config: &PluginConfig) -> Result<Box<dyn InterceptionPlugin>> {
        // Validate configuration first
        config.validate()?;

        debug!(
            "Creating plugin: pattern={}, type={}",
            config.pattern,
            config.response_source_type()
        );

        // Create the base plugin (file or command)
        let base_plugin: Box<dyn InterceptionPlugin> = match &config.response_source {
            ResponseSource::File { .. } => {
                let plugin = FilePlugin::from_config(config)?;
                Box::new(plugin)
            }
            ResponseSource::Command { .. } => {
                let plugin = CommandPlugin::from_config(config)?;
                Box::new(plugin)
            }
        };
        
        // Check if process-aware filters are configured
        let has_process_filters = config.uid.is_some()
            || config.username.is_some()
            || config.executable_pattern.is_some()
            || config.cmdline_pattern.is_some()
            || config.host_pattern.is_some();
        
        if has_process_filters {
            debug!("Wrapping plugin with process-aware filtering");
            let process_aware = ProcessAwarePlugin::new(config, base_plugin)?;
            Ok(Box::new(process_aware))
        } else {
            Ok(base_plugin)
        }
    }

    /// Create a plugin registry from a list of plugin configurations
    pub fn create_registry(configs: &[PluginConfig]) -> Result<PluginRegistry> {
        let mut registry = PluginRegistry::new();

        for (idx, config) in configs.iter().enumerate() {
            match Self::create_plugin(config) {
                Ok(plugin) => {
                    info!(
                        "Registered plugin {}: pattern={}, type={}",
                        idx,
                        config.pattern,
                        config.response_source_type()
                    );
                    registry.register(plugin);
                }
                Err(e) => {
                    return Err(InterposerError::PluginConfig(format!(
                        "Failed to create plugin {}: {}",
                        idx, e
                    )));
                }
            }
        }

        info!("Created plugin registry with {} plugins", registry.len());
        Ok(registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PatternType;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_create_file_plugin() {
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

        let plugin = PluginFactory::create_plugin(&config);
        assert!(plugin.is_ok());
    }

    #[test]
    fn test_create_command_plugin() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["test".to_string()],
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

        let plugin = PluginFactory::create_plugin(&config);
        assert!(plugin.is_ok());
    }

    #[test]
    fn test_create_plugin_invalid_config() {
        let config = PluginConfig {
            pattern: "".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["test".to_string()],
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

        let result = PluginFactory::create_plugin(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_registry_empty() {
        let configs: Vec<PluginConfig> = vec![];
        let registry = PluginFactory::create_registry(&configs);
        assert!(registry.is_ok());
        assert_eq!(registry.unwrap().len(), 0);
    }

    #[test]
    fn test_create_registry_multiple_plugins() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();

        let configs = vec![
            PluginConfig {
                pattern: "/test1".to_string(),
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
                pattern: "/test2".to_string(),
                pattern_type: PatternType::Exact,
                response_source: ResponseSource::Command {
                    command: "echo".to_string(),
                    args: vec!["test".to_string()],
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

        let registry = PluginFactory::create_registry(&configs);
        assert!(registry.is_ok());
        assert_eq!(registry.unwrap().len(), 2);
    }

    #[test]
    fn test_create_registry_invalid_plugin() {
        let configs = vec![
            PluginConfig {
                pattern: "/test1".to_string(),
                pattern_type: PatternType::Exact,
                response_source: ResponseSource::Command {
                    command: "echo".to_string(),
                    args: vec!["test".to_string()],
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
                pattern: "".to_string(), // Invalid: empty pattern
                pattern_type: PatternType::Exact,
                response_source: ResponseSource::Command {
                    command: "echo".to_string(),
                    args: vec!["test".to_string()],
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

        let result = PluginFactory::create_registry(&configs);
        assert!(result.is_err());
    }
}
