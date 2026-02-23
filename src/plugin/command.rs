use crate::config::PluginConfig;
use crate::error::{InterposerError, Result};
use crate::plugin::matcher::PatternMatcher;
use crate::plugin::{InterceptionPlugin, PluginResponse, RequestContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, warn};

/// Default timeout for command execution (30 seconds)
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 30;

/// Plugin that executes a command and returns its output as response
pub struct CommandPlugin {
    matcher: PatternMatcher,
    command: String,
    args: Vec<String>,
    status_code: u16,
    timeout: Duration,
    pattern: String,
    proxy_request_stdin: bool,
}

impl CommandPlugin {
    /// Create a new command plugin from configuration
    pub fn from_config(config: &PluginConfig) -> Result<Self> {
        let (command, args) = match &config.response_source {
            crate::config::ResponseSource::Command { command, args } => {
                (command.clone(), args.clone())
            }
            _ => {
                return Err(InterposerError::PluginConfig(
                    "CommandPlugin requires a Command response source".to_string(),
                ))
            }
        };

        let matcher = PatternMatcher::new(config.pattern.clone(), config.pattern_type.clone())
            .map_err(|e| InterposerError::PluginConfig(e))?;

        let timeout_secs = config.timeout_secs.unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS);
        let timeout = Duration::from_secs(timeout_secs);
        let proxy_request_stdin = config.proxy_request_stdin.unwrap_or(false);

        Ok(Self {
            matcher,
            command,
            args,
            status_code: config.status_code,
            timeout,
            pattern: config.pattern.clone(),
            proxy_request_stdin,
        })
    }

    /// Execute the command and capture stdout
    async fn execute_command(&self, request_context: Option<&RequestContext>) -> Result<Vec<u8>> {
        debug!(
            "Executing command: {} {}",
            self.command,
            self.args.join(" ")
        );

        // Create command
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        // If proxy_request_stdin is enabled and we have request context, set up stdin
        if self.proxy_request_stdin && request_context.is_some() {
            cmd.stdin(Stdio::piped());
        }

        // Spawn the command
        let mut child = cmd.spawn().map_err(|e| {
            warn!("Failed to spawn command: {}", e);
            InterposerError::CommandExecution(format!("Failed to spawn command: {}", e))
        })?;
        
        // If proxy_request_stdin is enabled, write the HTTP request to stdin
        if self.proxy_request_stdin {
            if let Some(ctx) = request_context {
                if let Some(mut stdin) = child.stdin.take() {
                    let http_request = self.serialize_http_request(ctx);
                    
                    if let Err(e) = stdin.write_all(http_request.as_bytes()).await {
                        warn!("Failed to write to command stdin: {}", e);
                        return Err(InterposerError::CommandExecution(format!(
                            "Failed to write to command stdin: {}",
                            e
                        )));
                    }
                    
                    // Close stdin to signal EOF
                    drop(stdin);
                }
            }
        }

        // Execute with timeout
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                warn!("Command execution timed out after {:?}", self.timeout);
                InterposerError::CommandTimeout
            })?
            .map_err(|e| {
                warn!("Command execution failed: {}", e);
                InterposerError::CommandExecution(format!("Failed to execute command: {}", e))
            })?;

        // Check exit status
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "Command exited with status {}: {}",
                output.status, stderr
            );
            return Err(InterposerError::CommandExecution(format!(
                "Command failed with status {}: {}",
                output.status, stderr
            )));
        }

        debug!("Command executed successfully");
        Ok(output.stdout)
    }
    
    /// Serialize HTTP request with optional process metadata headers
    fn serialize_http_request(&self, ctx: &RequestContext) -> String {
        let mut request = String::new();
        
        // Request line
        request.push_str(&format!("{} {} HTTP/1.1\r\n", ctx.method, ctx.path));
        
        // Headers (with process metadata injection if available)
        let mut headers = ctx.headers.clone();
        
        // Inject process metadata headers if available
        if let Some(ref process_info) = ctx.process_info {
            headers.insert("X-Forwarded-Uid".to_string(), process_info.uid.to_string());
            headers.insert("X-Forwarded-Username".to_string(), process_info.username.clone());
            headers.insert("X-Forwarded-Pid".to_string(), process_info.pid.to_string());
            headers.insert("X-Forwarded-Process-Name".to_string(), process_info.executable.clone());
            headers.insert("X-Forwarded-Process-Args".to_string(), process_info.cmdline.clone());
        }
        
        // Write all headers
        for (key, value) in &headers {
            request.push_str(&format!("{}: {}\r\n", key, value));
        }
        
        // Add Content-Length if body is present
        if !ctx.body.is_empty() {
            request.push_str(&format!("Content-Length: {}\r\n", ctx.body.len()));
        }
        
        // Empty line to separate headers from body
        request.push_str("\r\n");
        
        // Body
        if !ctx.body.is_empty() {
            request.push_str(&String::from_utf8_lossy(&ctx.body));
        }
        
        request
    }
}

#[async_trait]
impl InterceptionPlugin for CommandPlugin {
    fn matches(&self, path: &str) -> bool {
        self.matcher.matches(path)
    }
    
    fn matches_process_aware(
        &self,
        _process_info: Option<&crate::process::ProcessInfo>,
        _headers: &HashMap<String, String>,
    ) -> bool {
        // CommandPlugin doesn't filter by process or headers
        true
    }

    async fn get_response(&self, request_context: Option<&RequestContext>) -> Result<PluginResponse> {
        let body = self.execute_command(request_context).await?;

        // If proxy_request_stdin is enabled, try to parse the response as HTTP
        if self.proxy_request_stdin {
            match self.parse_http_response(&body) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    warn!("Failed to parse HTTP response from command, using raw output: {}", e);
                    // Fall through to use configured status_code and raw body
                }
            }
        }

        // Default behavior: use configured status_code and raw command output as body
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

impl CommandPlugin {
    /// Parse HTTP response from command output
    /// 
    /// Expected format:
    /// ```
    /// HTTP/1.1 200 OK
    /// Content-Type: application/json
    /// 
    /// {"result": "success"}
    /// ```
    fn parse_http_response(&self, output: &[u8]) -> Result<PluginResponse> {
        let response_str = String::from_utf8_lossy(output);
        let mut lines = response_str.lines();
        
        // Parse status line
        let status_line = lines.next()
            .ok_or_else(|| InterposerError::CommandExecution("Empty response from command".to_string()))?;
        
        let status_code = if status_line.starts_with("HTTP/") {
            // Parse "HTTP/1.1 200 OK" format
            let parts: Vec<&str> = status_line.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(InterposerError::CommandExecution(
                    format!("Invalid HTTP status line: {}", status_line)
                ));
            }
            parts[1].parse::<u16>().map_err(|e| {
                InterposerError::CommandExecution(format!("Invalid status code: {}", e))
            })?
        } else {
            return Err(InterposerError::CommandExecution(
                "Response does not start with HTTP status line".to_string()
            ));
        };
        
        // Parse headers
        let mut headers = HashMap::new();
        let mut body_start = 0;
        
        for (idx, line) in lines.enumerate() {
            if line.is_empty() {
                // Empty line marks end of headers
                body_start = status_line.len() + 1; // +1 for newline
                for _ in 0..=idx {
                    if let Some(pos) = response_str[body_start..].find('\n') {
                        body_start += pos + 1;
                    }
                }
                break;
            }
            
            // Parse header line "Key: Value"
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }
        
        // Extract body (everything after the empty line)
        let body = if body_start < output.len() {
            output[body_start..].to_vec()
        } else {
            Vec::new()
        };
        
        Ok(PluginResponse {
            status_code,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PatternType, ResponseSource};

    #[tokio::test]
    async fn test_command_plugin_creation() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["test".to_string()],
            },
            status_code: 200,
            timeout_secs: Some(5),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };

        let plugin = CommandPlugin::from_config(&config);
        assert!(plugin.is_ok());
    }

    #[tokio::test]
    async fn test_command_plugin_matches() {
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

        let plugin = CommandPlugin::from_config(&config).unwrap();
        assert!(plugin.matches("/test"));
        assert!(!plugin.matches("/other"));
    }

    #[tokio::test]
    async fn test_command_plugin_execute() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["hello world".to_string()],
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

        let plugin = CommandPlugin::from_config(&config).unwrap();
        let response = plugin.get_response(None).await.unwrap();

        assert_eq!(response.status_code, 200);
        let output = String::from_utf8_lossy(&response.body);
        assert!(output.contains("hello world"));
    }

    #[tokio::test]
    async fn test_command_plugin_custom_status_code() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "echo".to_string(),
                args: vec!["test".to_string()],
            },
            status_code: 201,
            timeout_secs: None,
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };

        let plugin = CommandPlugin::from_config(&config).unwrap();
        let response = plugin.get_response(None).await.unwrap();

        assert_eq!(response.status_code, 201);
    }

    #[tokio::test]
    async fn test_command_plugin_timeout() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "sleep".to_string(),
                args: vec!["10".to_string()],
            },
            status_code: 200,
            timeout_secs: Some(1),
            uid: None,
            username: None,
            executable_pattern: None,
            cmdline_pattern: None,
            host_pattern: None,
            proxy_request_stdin: None,
        };

        let plugin = CommandPlugin::from_config(&config).unwrap();
        let result = plugin.get_response(None).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InterposerError::CommandTimeout));
    }

    #[tokio::test]
    async fn test_command_plugin_failed_command() {
        let config = PluginConfig {
            pattern: "/test".to_string(),
            pattern_type: PatternType::Exact,
            response_source: ResponseSource::Command {
                command: "false".to_string(),
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

        let plugin = CommandPlugin::from_config(&config).unwrap();
        let result = plugin.get_response(None).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InterposerError::CommandExecution(_)
        ));
    }

    #[tokio::test]
    async fn test_command_plugin_wrong_response_source() {
        use std::io::Write;
        use tempfile::NamedTempFile;

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

        let result = CommandPlugin::from_config(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_plugin_default_timeout() {
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

        let plugin = CommandPlugin::from_config(&config).unwrap();
        assert_eq!(plugin.timeout, Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECS));
    }
}
