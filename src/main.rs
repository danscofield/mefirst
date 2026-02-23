mod capability;
mod config;
mod error;
mod logging;
mod redirect;
mod proxy;
mod upstream;
mod metrics;
mod plugin;
mod process;

use anyhow::Result;
use tracing::info;

use crate::config::Config;
use crate::logging::{init_logging, LoggingConfig};
use crate::redirect::RedirectMode;
use crate::proxy::ProxyServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with environment-based configuration
    // Supports RUST_LOG for log level and LOG_FORMAT for output format
    let log_config = LoggingConfig::default();
    init_logging(log_config)?;

    // Parse configuration
    let config = Config::load()?;
    
    info!("Starting mefirst proxy");
    info!("Bind port: {} (listening on 127.0.0.1 and ::1)", config.bind_port);
    info!("Upstream target: {}:{}", config.target_address, config.target_port);
    info!("Plugins configured: {}", config.plugins.len());
    info!("eBPF redirection: enabled (required)");

    // Setup eBPF redirection (mandatory)
    let redirector = RedirectMode::from_config(&config)?;
    
    // Setup redirector - eBPF is required for this proxy to function
    redirector.setup().await.map_err(|e| {
        anyhow::anyhow!("eBPF setup failed: {}. This proxy requires eBPF support.", e)
    })?;
    
    info!("eBPF redirection setup successful");

    // Clone redirector for cleanup on shutdown
    let shutdown_redirector = redirector.clone();
    
    // Spawn cleanup task that will run when the server shuts down
    let cleanup_handle = tokio::spawn(async move {
        // This will be cancelled when the server shuts down
        tokio::signal::ctrl_c().await.ok();
    });

    // Start proxy server (this will block until shutdown signal)
    let server_result = ProxyServer::new(config, redirector).await?.run().await;

    // Cancel the cleanup task if it's still running
    cleanup_handle.abort();

    // Cleanup redirector
    info!("Cleaning up redirector...");
    if let Err(e) = shutdown_redirector.teardown().await {
        tracing::warn!("Error during redirector teardown: {}", e);
    }

    // Return server result
    server_result?;
    
    info!("Proxy shut down successfully");
    Ok(())
}
