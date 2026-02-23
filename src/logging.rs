//! Logging configuration and initialization for the mefirst proxy.
//!
//! This module provides structured logging using the `tracing` crate with support for:
//! - Multiple output formats (text and JSON)
//! - Environment variable configuration (RUST_LOG, LOG_FORMAT)
//! - Configurable log levels and formatting options
//! - Integration with tower-http for HTTP request/response tracing
//!
//! # Environment Variables
//!
//! - `RUST_LOG`: Controls log level filtering (e.g., "info", "debug", "mefirst=trace")
//! - `LOG_FORMAT`: Sets output format ("text" or "json")
//!
//! # Examples
//!
//! ```no_run
//! use mefirst::logging::{init_logging, LoggingConfig, LogFormat};
//! use tracing::Level;
//!
//! // Initialize with default configuration (INFO level, text format)
//! init_logging(LoggingConfig::default()).unwrap();
//!
//! // Initialize with custom configuration
//! let config = LoggingConfig::new(Level::DEBUG)
//!     .with_format(LogFormat::Json)
//!     .with_span_events();
//! init_logging(config).unwrap();
//! ```

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Logging output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable text format
    Text,
    /// JSON format for structured logging
    Json,
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" | "pretty" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => Err(format!("Invalid log format: {}", s)),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: Level,
    /// Output format (text or json)
    pub format: LogFormat,
    /// Whether to include span events (enter, exit, close)
    pub span_events: bool,
    /// Whether to include file and line numbers
    pub with_location: bool,
    /// Whether to include thread IDs
    pub with_thread_ids: bool,
    /// Whether to include thread names
    pub with_thread_names: bool,
    /// Whether to include target (module path)
    pub with_target: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            format: LogFormat::Text,
            span_events: false,
            with_location: true,
            with_thread_ids: false,
            with_thread_names: false,
            with_target: true,
        }
    }
}

#[allow(dead_code)]
impl LoggingConfig {
    /// Create a new logging configuration with the specified level
    pub fn new(level: Level) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Set the output format
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Enable span events (enter, exit, close)
    pub fn with_span_events(mut self) -> Self {
        self.span_events = true;
        self
    }

    /// Enable file and line number logging
    pub fn with_location(mut self, enabled: bool) -> Self {
        self.with_location = enabled;
        self
    }

    /// Enable thread ID logging
    pub fn with_thread_ids(mut self, enabled: bool) -> Self {
        self.with_thread_ids = enabled;
        self
    }

    /// Enable thread name logging
    pub fn with_thread_names(mut self, enabled: bool) -> Self {
        self.with_thread_names = enabled;
        self
    }

    /// Enable target (module path) logging
    pub fn with_target(mut self, enabled: bool) -> Self {
        self.with_target = enabled;
        self
    }
}

/// Initialize the global tracing subscriber
///
/// This should be called once at application startup. It configures the tracing
/// subscriber based on the provided configuration and environment variables.
///
/// # Environment Variables
///
/// - `RUST_LOG`: Sets the log level filter (e.g., "info", "debug", "mefirst=trace")
/// - `LOG_FORMAT`: Sets the output format ("text" or "json")
///
/// # Examples
///
/// ```no_run
/// use tracing::Level;
/// use mefirst::logging::{init_logging, LoggingConfig, LogFormat};
///
/// // Initialize with default configuration
/// init_logging(LoggingConfig::default()).unwrap();
///
/// // Initialize with custom configuration
/// let config = LoggingConfig::new(Level::DEBUG)
///     .with_format(LogFormat::Json)
///     .with_span_events();
/// init_logging(config).unwrap();
/// ```
pub fn init_logging(config: LoggingConfig) -> Result<()> {
    // Build the environment filter
    // Priority: RUST_LOG env var > config.level
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!("mefirst={}", config.level.as_str()))
    });

    // Determine format from environment or config
    let format = std::env::var("LOG_FORMAT")
        .ok()
        .and_then(|s| s.parse::<LogFormat>().ok())
        .unwrap_or(config.format);

    // Configure span events
    let span_events = if config.span_events {
        FmtSpan::ENTER | FmtSpan::EXIT | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    // Build the subscriber based on format
    match format {
        LogFormat::Text => {
            let fmt_layer = fmt::layer()
                .with_span_events(span_events)
                .with_file(config.with_location)
                .with_line_number(config.with_location)
                .with_thread_ids(config.with_thread_ids)
                .with_thread_names(config.with_thread_names)
                .with_target(config.with_target);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
        LogFormat::Json => {
            let fmt_layer = fmt::layer()
                .json()
                .with_span_events(span_events)
                .with_file(config.with_location)
                .with_line_number(config.with_location)
                .with_thread_ids(config.with_thread_ids)
                .with_thread_names(config.with_thread_names)
                .with_target(config.with_target);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .init();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_str() {
        assert_eq!("text".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("pretty".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("TEXT".parse::<LogFormat>().unwrap(), LogFormat::Text);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert!("invalid".parse::<LogFormat>().is_err());
    }

    #[test]
    fn test_logging_config_builder() {
        let config = LoggingConfig::new(Level::DEBUG)
            .with_format(LogFormat::Json)
            .with_span_events()
            .with_location(false)
            .with_thread_ids(true);

        assert_eq!(config.level, Level::DEBUG);
        assert_eq!(config.format, LogFormat::Json);
        assert!(config.span_events);
        assert!(!config.with_location);
        assert!(config.with_thread_ids);
    }

    #[test]
    fn test_default_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert_eq!(config.format, LogFormat::Text);
        assert!(!config.span_events);
        assert!(config.with_location);
        assert!(!config.with_thread_ids);
        assert!(!config.with_thread_names);
        assert!(config.with_target);
    }
}
