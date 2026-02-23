mod handler;
pub mod socket_fd_layer;

use crate::config::Config;
use crate::error::{InterposerError, Result};
use crate::metrics::Metrics;
use crate::redirect::RedirectMode;
use axum::{routing::get, Router};
use prometheus::{Encoder, Registry, TextEncoder};
use std::sync::Arc;
use tokio::signal;
use tower::ServiceBuilder;
use tower::ServiceExt;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{info, Level};
use socket_fd_layer::SocketFd;

pub struct ProxyServer {
    config: Arc<Config>,
    _redirector: RedirectMode,
}

impl ProxyServer {
    pub async fn new(config: Config, redirector: RedirectMode) -> Result<Self> {
        let config = Arc::new(config);

        Ok(Self {
            config,
            _redirector: redirector,
        })
    }

    pub async fn run(self) -> Result<()> {
        // Load plugins from config
        let plugin_registry = crate::plugin::PluginFactory::create_registry(&self.config.plugins)?;
        
        // Setup metrics if enabled
        let metrics = if self.config.enable_metrics {
            let registry = Registry::new();
            let metrics = Metrics::new(&registry);
            info!("Metrics enabled on port {}", self.config.metrics_port);
            
            // Spawn metrics server (always binds to 127.0.0.1)
            let metrics_addr = format!("127.0.0.1:{}", self.config.metrics_port);
            let metrics_registry = Arc::new(registry);
            tokio::spawn(run_metrics_server(metrics_addr, metrics_registry));
            
            Some(Arc::new(metrics))
        } else {
            info!("Metrics disabled");
            None
        };
        
        // Initialize process metadata retriever if on Linux
        let process_retriever = if cfg!(target_os = "linux") {
            match crate::process::retriever::ProcessMetadataRetriever::new() {
                Ok(retriever) => {
                    info!("Process metadata retriever initialized");
                    Some(Arc::new(retriever))
                }
                Err(e) => {
                    tracing::warn!("Failed to initialize process metadata retriever: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let state = handler::ProxyState {
            config: Arc::clone(&self.config),
            plugin_registry: Arc::new(plugin_registry),
            metrics,
            process_retriever,
        };

        // Build the router with middleware
        let app = Router::new()
            .fallback(handler::handle_request)
            .layer(
                ServiceBuilder::new()
                    // Add tracing middleware for request/response logging
                    .layer(
                        TraceLayer::new_for_http()
                            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                            .on_response(DefaultOnResponse::new().level(Level::INFO)),
                    ),
            )
            .with_state(state);

        // Always bind to localhost on both IPv4 and IPv6
        // This provides dual-stack support without exposing to remote connections
        let ipv4_addr = format!("127.0.0.1:{}", self.config.bind_port);
        let ipv6_addr = format!("[::1]:{}", self.config.bind_port);
        
        info!("Starting dual-stack proxy server on localhost:");
        info!("  IPv4: {}", ipv4_addr);
        info!("  IPv6: {}", ipv6_addr);

        // Create IPv4 listener
        let listener_v4 = tokio::net::TcpListener::bind(&ipv4_addr).await
            .map_err(|e| InterposerError::HttpServer(format!("Failed to bind to {}: {}", ipv4_addr, e)))?;
        info!("✓ IPv4 listener bound successfully to {}", ipv4_addr);

        // Create IPv6 listener
        let listener_v6 = tokio::net::TcpListener::bind(&ipv6_addr).await
            .map_err(|e| InterposerError::HttpServer(format!("Failed to bind to {}: {}", ipv6_addr, e)))?;
        info!("✓ IPv6 listener bound successfully to {}", ipv6_addr);

        // Spawn IPv4 listener task
        let app_v4 = app.clone();
        let v4_task = tokio::spawn(async move {
            accept_connections(listener_v4, app_v4, "IPv4").await
        });

        // Spawn IPv6 listener task
        let app_v6 = app.clone();
        let v6_task = tokio::spawn(async move {
            accept_connections(listener_v6, app_v6, "IPv6").await
        });

        // Wait for shutdown signal
        shutdown_signal().await;
        info!("Shutdown signal received, stopping proxy server...");

        // Tasks will be cancelled when dropped
        drop(v4_task);
        drop(v6_task);

        info!("Proxy server shut down gracefully");
        Ok(())
    }
}

/// Accept connections from a listener and handle them
async fn accept_connections(
    listener: tokio::net::TcpListener,
    app: axum::Router,
    label: &str,
) {
    info!("[{}] Accept loop started", label);
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                tracing::debug!("[{}] Accepted connection from {}", label, addr);
                #[cfg(target_os = "linux")]
                let fd = {
                    use std::os::unix::io::AsRawFd;
                    stream.as_raw_fd()
                };
                
                #[cfg(not(target_os = "linux"))]
                let fd = -1;
                
                let app = app.clone();
                tokio::spawn(async move {
                    // Create a hyper service that wraps our axum app
                    let service = hyper::service::service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                        let app = app.clone();
                        async move {
                            // Convert hyper request to axum request
                            let (parts, body) = req.into_parts();
                            let body = axum::body::Body::new(body);
                            let mut req = axum::extract::Request::from_parts(parts, body);
                            
                            // Inject SocketFd and SocketAddr into request extensions
                            req.extensions_mut().insert(SocketFd(fd));
                            req.extensions_mut().insert(axum::extract::ConnectInfo(addr));
                            
                            // Call the axum app
                            Ok::<_, std::convert::Infallible>(app.oneshot(req).await.unwrap())
                        }
                    });
                    
                    if let Err(err) = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(stream), service)
                        .await
                    {
                        tracing::error!("Error serving connection: {}", err);
                    }
                });
            }
            Err(e) => {
                tracing::error!("[{}] Failed to accept connection: {}", label, e);
            }
        }
    }
}

/// Run metrics server on separate port
async fn run_metrics_server(addr: String, registry: Arc<Registry>) {
    let app = Router::new()
        .route("/metrics", get(move || metrics_handler(Arc::clone(&registry))));
    
    info!("Metrics server listening on {}", addr);
    
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
            {
                tracing::error!("Metrics server error: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to bind metrics server to {}: {}", addr, e);
        }
    }
}

/// Metrics endpoint handler
async fn metrics_handler(registry: Arc<Registry>) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received SIGTERM signal");
        },
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::redirect::RedirectMode;
    use clap::Parser;

    #[tokio::test]
    async fn test_proxy_server_creation() {
        let config = Config::parse_from(&["mefirst"]);
        let redirector = RedirectMode::Noop;
        
        let server = ProxyServer::new(config, redirector).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_proxy_server_with_custom_config() {
        let config = Config::parse_from(&[
            "mefirst",
            "--bind-port", "9999",
        ]);
        
        assert_eq!(config.bind_port, 9999);
        
        let redirector = RedirectMode::Noop;
        let server = ProxyServer::new(config, redirector).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_proxy_state_creation() {
        let config = Config::parse_from(&["mefirst"]);
        let config_arc = Arc::new(config);
        
        let state = handler::ProxyState {
            config: config_arc.clone(),
            plugin_registry: Arc::new(crate::plugin::PluginRegistry::new()),
            metrics: None,
            process_retriever: None,
        };
        
        assert_eq!(state.config.bind_port, 8080);
    }
}
