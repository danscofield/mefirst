use crate::config::PluginConfig;
use crate::error::{InterposerError, Result};
use crate::plugin::matcher::PatternMatcher;
use crate::plugin::{InterceptionPlugin, PluginResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

/// Plugin that returns static file content as response
pub struct FilePlugin {
    matcher: PatternMatcher,
    file_path: PathBuf,
    status_code: u16,
    pattern: String,
}

impl FilePlugin {
    /// Create a new file plugin from configuration
    pub fn from_config(config: &PluginConfig) -> Result<Self> {
        let file_path = match &config.response_source {
            crate::config::ResponseSource::File { path } => path.clone(),
            _ => {
                return Err(InterposerError::PluginConfig(
                    "FilePlugin requires a File response source".to_string(),
                ))
            }
        };

        let matcher = PatternMatcher::new(config.pattern.clone(), config.pattern_type.clone())
            .map_err(|e| InterposerError::PluginConfig(e))?;

        Ok(Self {
            matcher,
            file_path,
            status_code: config.status_code,
            pattern: config.pattern.clone(),
        })
    }

    /// Load the file content
    async fn load_file(&self) -> Result<Vec<u8>> {
        debug!("Loading file: {:?}", self.file_path);
        
        tokio::fs::read(&self.file_path)
            .await
            .map_err(|e| {
                InterposerError::Plugin(format!(
                    "Failed to read file {:?}: {}",
                    self.file_path, e
                ))
            })
    }
}

#[async_trait]
impl InterceptionPlugin for FilePlugin {
    fn matches(&self, path: &str) -> bool {
        self.matcher.matches(path)
    }
    
    fn matches_process_aware(
        &self,
        _process_info: Option<&crate::process::ProcessInfo>,
        _headers: &HashMap<String, String>,
    ) -> bool {
        // FilePlugin doesn't filter by process or headers
        true
    }

    async fn get_response(&self, _request_context: Option<&crate::plugin::RequestContext>) -> Result<PluginResponse> {
        let body = self.load_file().await?;

        Ok(PluginResponse {
            status_code: self.status_code,
            headers: HashMap::new(),
            body,
        })
    }

    fn pattern(&self) -> &str {
        &self.pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PatternType, ResponseSource};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_plugin_creation() {
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

        let plugin = FilePlugin::from_config(&config);
        assert!(plugin.is_ok());
    }

    #[tokio::test]
    async fn test_file_plugin_matches() {
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

        let plugin = FilePlugin::from_config(&config).unwrap();
        assert!(plugin.matches("/test"));
        assert!(!plugin.matches("/other"));
    }

    #[tokio::test]
    async fn test_file_plugin_get_response() {
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

        let plugin = FilePlugin::from_config(&config).unwrap();
        let response = plugin.get_response(None).await.unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, b"test content");
    }

    #[tokio::test]
    async fn test_file_plugin_glob_pattern() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();

        let config = PluginConfig {
            pattern: "/test/*".to_string(),
            pattern_type: PatternType::Glob,
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

        let plugin = FilePlugin::from_config(&config).unwrap();
        assert!(plugin.matches("/test/foo"));
        assert!(plugin.matches("/test/bar"));
        assert!(!plugin.matches("/test"));
    }

    #[tokio::test]
    async fn test_file_plugin_custom_status_code() {
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

        let plugin = FilePlugin::from_config(&config).unwrap();
        let response = plugin.get_response(None).await.unwrap();

        assert_eq!(response.status_code, 404);
    }

    #[tokio::test]
    async fn test_file_plugin_wrong_response_source() {
        let config = PluginConfig {
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

        let result = FilePlugin::from_config(&config);
        assert!(result.is_err());
    }
}
