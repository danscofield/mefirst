use thiserror::Error;

/// Main error type for the Interposer
#[derive(Error, Debug)]
pub enum InterposerError {
    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid configuration value for {field}: {reason}")]
    InvalidConfig { field: String, reason: String },

    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    // Upstream client errors
    #[error("Upstream client error: {0}")]
    Upstream(String),

    #[error("Authentication validation failed: {0}")]
    AuthValidation(String),

    #[error("Upstream request failed: {0}")]
    UpstreamRequest(String),

    #[error("Upstream response parse error: {0}")]
    UpstreamResponseParse(String),

    // Plugin errors
    #[error("Plugin error: {0}")]
    Plugin(String),
    
    #[error("Plugin configuration error: {0}")]
    PluginConfig(String),
    
    #[error("Pattern matching error: {0}")]
    PatternMatch(String),
    
    #[error("Command execution error: {0}")]
    CommandExecution(String),
    
    #[error("Command timeout exceeded")]
    CommandTimeout,

    // eBPF errors
    #[error("eBPF error: {0}")]
    Ebpf(String),

    #[error("eBPF program load failed: {0}")]
    EbpfLoad(String),

    #[error("eBPF program attach failed: {0}")]
    EbpfAttach(String),

    #[error("eBPF not supported on this system: {0}")]
    EbpfNotSupported(String),

    #[error("Cgroup not found: {0}")]
    CgroupNotFound(String),

    // HTTP/Network errors
    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP server error: {0}")]
    HttpServer(String),

    #[error("Request timeout")]
    Timeout,

    #[error("Connection failed: {0}")]
    Connection(String),

    // Serialization errors
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("TOML serialization error: {0}")]
    Toml(#[from] toml::de::Error),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    // Proxy/routing errors
    #[error("Invalid request path: {0}")]
    InvalidPath(String),

    #[error("Request filtered: {0}")]
    RequestFiltered(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    // Shutdown/lifecycle errors
    #[error("Shutdown timeout exceeded")]
    ShutdownTimeout,

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    // Generic errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("{0}")]
    Other(String),
}

/// Convenient Result type alias for the Interposer
pub type Result<T> = std::result::Result<T, InterposerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = InterposerError::Config("invalid port".to_string());
        assert_eq!(err.to_string(), "Configuration error: invalid port");

        let err = InterposerError::InvalidConfig {
            field: "bind_port".to_string(),
            reason: "must be between 1 and 65535".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid configuration value for bind_port: must be between 1 and 65535"
        );
    }

    #[test]
    fn test_error_from_conversions() {
        // Test automatic conversion from serde_json::Error
        let result: Result<serde_json::Value> =
            serde_json::from_str("invalid json").map_err(InterposerError::from);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InterposerError::Json(_)));

        // Test automatic conversion from std::io::Error
        let result: Result<String> =
            std::fs::read_to_string("/nonexistent/file").map_err(InterposerError::from);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InterposerError::Io(_)));
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> Result<i32> {
            Ok(42)
        }

        fn returns_error() -> Result<i32> {
            Err(InterposerError::Internal("test error".to_string()))
        }

        assert_eq!(returns_result().unwrap(), 42);
        assert!(returns_error().is_err());
    }
}
