// Plugin configuration structures are defined in src/config.rs
// This module provides additional utilities for working with plugin configs

use crate::config::{PatternType, PluginConfig, ResponseSource};
use crate::error::{InterposerError, Result};

impl PluginConfig {
    /// Validate the plugin configuration
    pub fn validate(&self) -> Result<()> {
        // Validate pattern
        if self.pattern.is_empty() {
            return Err(InterposerError::PluginConfig(
                "Plugin pattern cannot be empty".to_string(),
            ));
        }
        
        // Validate pattern type
        match self.pattern_type {
            PatternType::Regex => {
                // Try to compile the regex to validate it
                regex::Regex::new(&self.pattern).map_err(|e| {
                    InterposerError::PluginConfig(format!("Invalid regex pattern: {}", e))
                })?;
            }
            PatternType::Glob => {
                // Glob patterns are validated when used
            }
            PatternType::Exact => {
                // Exact patterns don't need validation
            }
        }
        
        // Validate response source
        match &self.response_source {
            ResponseSource::File { path } => {
                if !path.exists() {
                    return Err(InterposerError::PluginConfig(format!(
                        "Response file does not exist: {:?}",
                        path
                    )));
                }
                if !path.is_file() {
                    return Err(InterposerError::PluginConfig(format!(
                        "Response path is not a file: {:?}",
                        path
                    )));
                }
            }
            ResponseSource::Command { command, .. } => {
                if command.is_empty() {
                    return Err(InterposerError::PluginConfig(
                        "Command cannot be empty".to_string(),
                    ));
                }
            }
        }
        
        // Validate status code
        if self.status_code < 100 || self.status_code > 599 {
            return Err(InterposerError::PluginConfig(format!(
                "Invalid HTTP status code: {}",
                self.status_code
            )));
        }
        
        // Validate timeout
        if let Some(timeout) = self.timeout_secs {
            if timeout == 0 {
                return Err(InterposerError::PluginConfig(
                    "Timeout must be greater than 0".to_string(),
                ));
            }
        }
        
        // Validate proxy_request_stdin compatibility
        if let Some(true) = self.proxy_request_stdin {
            match &self.response_source {
                ResponseSource::Command { .. } => {
                    // Valid: proxy_request_stdin can be used with command-based sources
                }
                ResponseSource::File { .. } => {
                    return Err(InterposerError::PluginConfig(
                        "proxy_request_stdin can only be used with command-based response sources".to_string(),
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Get the response source type as a string for logging
    pub fn response_source_type(&self) -> &str {
        match &self.response_source {
            ResponseSource::File { .. } => "file",
            ResponseSource::Command { .. } => "command",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_validate_empty_pattern() {
        let config = PluginConfig {
            pattern: "".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
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
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_validate_invalid_regex() {
        let config = PluginConfig {
            pattern: "[invalid".to_string(),
            pattern_type: PatternType::Regex,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
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
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_validate_valid_regex() {
        let config = PluginConfig {
            pattern: r"^/test/.*$".to_string(),
            pattern_type: PatternType::Regex,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
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
        
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_validate_nonexistent_file() {
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
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_validate_valid_file() {
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
        
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_validate_empty_command() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "".to_string(),
                args: vec![],
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
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_validate_invalid_status_code() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
            },
            status_code: 999,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_validate_zero_timeout() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
            },
            status_code: 200,
            timeout_secs: Some(0),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_response_source_type() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test").unwrap();
        
        let file_config = PluginConfig {
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
        
        assert_eq!(file_config.response_source_type(), "file");
        
        let command_config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo test".to_string(),
                args: vec![],
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
        
        assert_eq!(command_config.response_source_type(), "command");
    }
}
