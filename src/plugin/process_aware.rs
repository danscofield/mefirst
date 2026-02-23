use crate::config::PluginConfig;
use crate::error::{InterposerError, Result};
use crate::plugin::matcher::PatternMatcher;
use crate::plugin::{InterceptionPlugin, PluginResponse};
use crate::process::ProcessInfo;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::debug;

/// Plugin that filters requests based on process metadata and HTTP headers
/// 
/// This plugin extends the basic path matching with process-aware filters:
/// - uid: Filter by user ID
/// - username: Filter by username
/// - executable_pattern: Filter by executable path pattern
/// - cmdline_pattern: Filter by command line pattern
/// - host_pattern: Filter by HTTP Host header pattern
/// 
/// All configured filters use AND logic - all must match for the plugin to match.
pub struct ProcessAwarePlugin {
    path_matcher: PatternMatcher,
    uid_filter: Option<u32>,
    username_filter: Option<String>,
    executable_matcher: Option<PatternMatcher>,
    cmdline_matcher: Option<PatternMatcher>,
    host_matcher: Option<PatternMatcher>,
    delegate: Box<dyn InterceptionPlugin>,
    pattern: String,
}

impl ProcessAwarePlugin {
    /// Create a new ProcessAwarePlugin from configuration
    pub fn new(config: &PluginConfig, delegate: Box<dyn InterceptionPlugin>) -> Result<Self> {
        let path_matcher = PatternMatcher::new(config.pattern.clone(), config.pattern_type.clone())
            .map_err(|e| InterposerError::PluginConfig(e))?;
        
        // Build optional pattern matchers
        let executable_matcher = if let Some(ref pattern_config) = config.executable_pattern {
            Some(PatternMatcher::from_config(pattern_config)
                .map_err(|e| InterposerError::PluginConfig(format!("Invalid executable pattern: {}", e)))?)
        } else {
            None
        };
        
        let cmdline_matcher = if let Some(ref pattern_config) = config.cmdline_pattern {
            Some(PatternMatcher::from_config(pattern_config)
                .map_err(|e| InterposerError::PluginConfig(format!("Invalid cmdline pattern: {}", e)))?)
        } else {
            None
        };
        
        let host_matcher = if let Some(ref pattern_config) = config.host_pattern {
            Some(PatternMatcher::from_config(pattern_config)
                .map_err(|e| InterposerError::PluginConfig(format!("Invalid host pattern: {}", e)))?)
        } else {
            None
        };
        
        Ok(Self {
            path_matcher,
            uid_filter: config.uid,
            username_filter: config.username.clone(),
            executable_matcher,
            cmdline_matcher,
            host_matcher,
            delegate,
            pattern: config.pattern.clone(),
        })
    }
    
    /// Check if process metadata matches all configured process filters
    fn matches_process_filters(&self, process_info: &ProcessInfo) -> bool {
        // Check UID filter
        if let Some(required_uid) = self.uid_filter {
            if process_info.uid != required_uid {
                debug!("UID mismatch: required={}, actual={}", required_uid, process_info.uid);
                return false;
            }
        }
        
        // Check username filter
        if let Some(ref required_username) = self.username_filter {
            if &process_info.username != required_username {
                debug!("Username mismatch: required={}, actual={}", required_username, process_info.username);
                return false;
            }
        }
        
        // Check executable pattern
        if let Some(ref matcher) = self.executable_matcher {
            if !matcher.matches(&process_info.executable) {
                debug!("Executable pattern mismatch: pattern={}, actual={}", matcher.pattern(), process_info.executable);
                return false;
            }
        }
        
        // Check cmdline pattern
        if let Some(ref matcher) = self.cmdline_matcher {
            if !matcher.matches(&process_info.cmdline) {
                debug!("Cmdline pattern mismatch: pattern={}, actual={}", matcher.pattern(), process_info.cmdline);
                return false;
            }
        }
        
        // All process filters matched
        true
    }
    
    /// Check if HTTP headers match all configured header filters
    fn matches_header_filters(&self, headers: &HashMap<String, String>) -> bool {
        // Check Host header pattern
        if let Some(ref matcher) = self.host_matcher {
            if let Some(host) = headers.get("host") {
                if !matcher.matches(host) {
                    debug!("Host pattern mismatch: pattern={}, actual={}", matcher.pattern(), host);
                    return false;
                }
            } else {
                debug!("Host header required but not present");
                return false;
            }
        }
        
        // All header filters matched
        true
    }
}

#[async_trait]
impl InterceptionPlugin for ProcessAwarePlugin {
    fn matches(&self, path: &str) -> bool {
        self.path_matcher.matches(path)
    }
    
    fn matches_process_aware(
        &self,
        process_info: Option<&ProcessInfo>,
        headers: &HashMap<String, String>,
    ) -> bool {
        // If any process filters are configured, we need process metadata
        let has_process_filters = self.uid_filter.is_some()
            || self.username_filter.is_some()
            || self.executable_matcher.is_some()
            || self.cmdline_matcher.is_some();
        
        if has_process_filters {
            if let Some(info) = process_info {
                if !self.matches_process_filters(info) {
                    return false;
                }
            } else {
                debug!("Process filters configured but no process metadata available");
                return false;
            }
        }
        
        // Check header filters
        if !self.matches_header_filters(headers) {
            return false;
        }
        
        // All filters matched
        true
    }
    
    async fn get_response(&self, request_context: Option<&crate::plugin::RequestContext>) -> Result<PluginResponse> {
        // Delegate to the underlying plugin (file or command)
        self.delegate.get_response(request_context).await
    }
    
    fn pattern(&self) -> &str {
        &self.pattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PatternConfig, PatternType, ResponseSource};
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    // Helper to create a test delegate plugin
    struct TestDelegate {
        response: PluginResponse,
    }
    
    #[async_trait]
    impl InterceptionPlugin for TestDelegate {
        fn matches(&self, _path: &str) -> bool {
            true
        }
        
        fn matches_process_aware(
            &self,
            _process_info: Option<&ProcessInfo>,
            _headers: &HashMap<String, String>,
        ) -> bool {
            true
        }
        
        async fn get_response(&self, _request_context: Option<&crate::plugin::RequestContext>) -> Result<PluginResponse> {
            Ok(self.response.clone())
        }
        
        fn pattern(&self) -> &str {
            "/test"
        }
    }
    
    #[tokio::test]
    async fn test_no_filters_matches_all() {
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
            host_pattern: None,
            proxy_request_stdin: None,
        };
        
        let delegate = Box::new(TestDelegate {
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test".to_vec(),
            },
        });
        
        let plugin = ProcessAwarePlugin::new(&config, delegate).unwrap();
        let headers = HashMap::new();
        
        // Should match without process info when no filters configured
        assert!(plugin.matches_process_aware(None, &headers));
    }
    
    #[tokio::test]
    async fn test_uid_filter_matches() {
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
            uid: Some(1000),
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };
        
        let delegate = Box::new(TestDelegate {
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test".to_vec(),
            },
        });
        
        let plugin = ProcessAwarePlugin::new(&config, delegate).unwrap();
        let headers = HashMap::new();
        
        let process_info = ProcessInfo::new(
            1000,
            "testuser".to_string(),
            12345,
            "/usr/bin/curl".to_string(),
            "curl example.com".to_string(),
        );
        
        // Should match when UID matches
        assert!(plugin.matches_process_aware(Some(&process_info), &headers));
        
        let wrong_uid_info = ProcessInfo::new(
            2000,
            "testuser".to_string(),
            12345,
            "/usr/bin/curl".to_string(),
            "curl example.com".to_string(),
        );
        
        // Should not match when UID doesn't match
        assert!(!plugin.matches_process_aware(Some(&wrong_uid_info), &headers));
    }
    
    #[tokio::test]
    async fn test_executable_pattern_filter() {
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
                pattern: "/usr/bin/*".to_string(),
                pattern_type: PatternType::Glob,
            }),
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };
        
        let delegate = Box::new(TestDelegate {
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test".to_vec(),
            },
        });
        
        let plugin = ProcessAwarePlugin::new(&config, delegate).unwrap();
        let headers = HashMap::new();
        
        let matching_info = ProcessInfo::new(
            1000,
            "testuser".to_string(),
            12345,
            "/usr/bin/curl".to_string(),
            "curl example.com".to_string(),
        );
        
        assert!(plugin.matches_process_aware(Some(&matching_info), &headers));
        
        let non_matching_info = ProcessInfo::new(
            1000,
            "testuser".to_string(),
            12345,
            "/usr/local/bin/curl".to_string(),
            "curl example.com".to_string(),
        );
        
        assert!(!plugin.matches_process_aware(Some(&non_matching_info), &headers));
    }
    
    #[tokio::test]
    async fn test_host_pattern_filter() {
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
                pattern: "example.com".to_string(),
                pattern_type: PatternType::Exact,
            }),
            proxy_request_stdin: None,
        };
        
        let delegate = Box::new(TestDelegate {
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test".to_vec(),
            },
        });
        
        let plugin = ProcessAwarePlugin::new(&config, delegate).unwrap();
        
        let mut headers = HashMap::new();
        headers.insert("host".to_string(), "example.com".to_string());
        
        assert!(plugin.matches_process_aware(None, &headers));
        
        let mut wrong_headers = HashMap::new();
        wrong_headers.insert("host".to_string(), "other.com".to_string());
        
        assert!(!plugin.matches_process_aware(None, &wrong_headers));
    }
}
