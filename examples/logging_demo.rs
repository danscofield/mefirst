/// Example demonstrating the logging configuration options
///
/// This example shows how logging would be used in the mefirst proxy.
/// Since the logging module is part of the binary crate, we simulate the usage here.
///
/// Run with different configurations:
/// - Default (INFO level, text format):
///   cargo run --example logging_demo
///
/// - Debug level:
///   RUST_LOG=debug cargo run --example logging_demo
///
/// - JSON format:
///   LOG_FORMAT=json cargo run --example logging_demo
///
/// - Trace level with JSON:
///   RUST_LOG=trace LOG_FORMAT=json cargo run --example logging_demo

use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() -> anyhow::Result<()> {
    // Initialize logging similar to how the main application does it
    // This respects RUST_LOG and LOG_FORMAT environment variables
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let format = std::env::var("LOG_FORMAT")
        .unwrap_or_else(|_| "text".to_string());

    if format == "json" {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer())
            .init();
    }

    info!("Logging demo started");
    info!("This demonstrates the logging capabilities of mefirst proxy");

    // Log at different levels
    trace!("This is a trace message (only visible with RUST_LOG=trace)");
    debug!("This is a debug message (only visible with RUST_LOG=debug)");
    info!("This is an info message (default level)");
    warn!("This is a warning message");
    error!("This is an error message");

    // Structured logging with fields
    info!(
        bind_address = "127.0.0.1",
        bind_port = 8080,
        "Server configuration"
    );

    info!(
        redirect_mode = "ebpf",
        cgroup_path = "/sys/fs/cgroup",
        "Redirection settings"
    );

    // Simulate some operations
    simulate_request_handling();

    info!("Logging demo completed");

    Ok(())
}

fn simulate_request_handling() {
    info!("Simulating request handling");

    // Simulate a successful request
    info!(
        method = "GET",
        path = "/latest/meta-data/instance-id",
        status = 200,
        duration_ms = 5,
        "Request completed"
    );

    // Simulate a proxied request
    debug!(
        method = "GET",
        path = "/latest/meta-data/ami-id",
        proxied = true,
        "Forwarding to upstream service"
    );

    // Simulate an intercepted request
    info!(
        method = "GET",
        path = "/latest/meta-data/iam/security-credentials/my-role",
        intercepted = true,
        plugin = "credential-plugin",
        "Request intercepted by plugin"
    );

    // Simulate an error
    warn!(
        method = "GET",
        path = "/invalid/path",
        status = 404,
        "Path not found"
    );
}

