use crate::config::Config;
use crate::upstream::UpstreamClient;
use crate::metrics::Metrics;
use crate::plugin::PluginRegistry;
use crate::process::{ProcessInfo, retriever::ProcessMetadataRetriever};
use crate::proxy::socket_fd_layer::SocketFd;
use axum::{
    body::Body,
    extract::{Request, State, ConnectInfo},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct ProxyState {
    pub config: Arc<Config>,
    pub plugin_registry: Arc<PluginRegistry>,
    pub metrics: Option<Arc<Metrics>>,
    pub process_retriever: Option<Arc<ProcessMetadataRetriever>>,
}

pub async fn handle_request(
    State(state): State<ProxyState>,
    request: Request,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|q| q.to_string());
    let headers = request.headers().clone();
    
    // Extract socket FD and client address from extensions
    let _socket_fd = request.extensions().get::<SocketFd>().map(|fd| fd.0);
    let client_addr = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|ci| ci.0);

    // Construct full path with query string if present
    let full_path = if let Some(q) = query {
        format!("{}?{}", path, q)
    } else {
        path.clone()
    };

    if let Some(addr) = client_addr {
        debug!("Handling request: {} {} from {}", method, full_path, addr);
    } else {
        debug!("Handling request: {} {}", method, full_path);
    }
    
    // Try to retrieve process metadata if retriever is available
    let process_info = if let (Some(ref retriever), Some(addr)) = (&state.process_retriever, client_addr) {
        debug!("Process retriever is available, attempting to get metadata for peer {}", addr);
        match retriever.get_metadata_from_peer_addr(&addr) {
            Some((info, _dest)) => {
                debug!("Successfully retrieved process metadata for peer");
                Some(info)
            }
            None => {
                debug!("Failed to retrieve process metadata for peer (get_metadata_from_peer_addr returned None)");
                None
            }
        }
    } else {
        if state.process_retriever.is_none() {
            debug!("Process retriever is not available");
        }
        if client_addr.is_none() {
            debug!("Client address is not available");
        }
        None
    };

    // Extract request body
    let body_bytes = match request.into_body().collect().await {
        Ok(collected) => Some(collected.to_bytes()),
        Err(e) => {
            error!("Failed to read request body: {}", e);
            let response = (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
            record_metrics(&state, &method, response.status(), start, false);
            return response;
        }
    };
    
    // Convert headers to HashMap for plugin matching
    let header_map: std::collections::HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str().ok().map(|val| (k.as_str().to_lowercase(), val.to_string()))
        })
        .collect();

    // Check if path matches any plugin patterns
    if let Some(plugin) = state.plugin_registry.find_match(&path, process_info.as_ref(), &header_map) {
        let pattern = plugin.pattern().to_string();
        debug!("Plugin matched for path: {} (pattern: {})", path, pattern);
        
        // Record plugin hit
        if let Some(ref metrics) = state.metrics {
            metrics.plugin_hits.with_label_values(&[&pattern]).inc();
        }
        
        // Create request context for plugins that need it (e.g., proxy_request_stdin)
        let request_context = crate::plugin::RequestContext {
            method: method.to_string(),
            path: full_path.clone(),
            headers: header_map.clone(),
            body: body_bytes.as_ref().map(|b| b.to_vec()).unwrap_or_default(),
            process_info: process_info.clone(),
        };
        
        // Get plugin response
        match plugin.get_response(Some(&request_context)).await {
            Ok(plugin_response) => {
                log_plugin_response_with_process(
                    &method,
                    &full_path,
                    plugin_response.status_code,
                    &pattern,
                    process_info.as_ref(),
                );
                
                // Build response with plugin status code and headers
                let mut response_builder = Response::builder()
                    .status(plugin_response.status_code);
                
                for (key, value) in plugin_response.headers {
                    response_builder = response_builder.header(key, value);
                }
                
                let response = response_builder
                    .body(Body::from(plugin_response.body))
                    .unwrap();
                
                record_metrics(&state, &method, response.status(), start, true);
                return response;
            }
            Err(e) => {
                error!("Plugin error for path {}: {}", path, e);
                
                // Record plugin error
                if let Some(ref metrics) = state.metrics {
                    metrics.plugin_errors.with_label_values(&[&pattern]).inc();
                }
                
                let response = (StatusCode::INTERNAL_SERVER_ERROR, "Plugin error").into_response();
                record_metrics(&state, &method, response.status(), start, true);
                return response;
            }
        }
    }

    // No plugin matched, proxy to upstream service
    match proxy_to_upstream(&state, &method, &full_path, &headers, body_bytes, process_info.as_ref()).await {
        Ok(response) => {
            log_request_with_process(&method, &full_path, response.status(), process_info.as_ref());
            record_metrics(&state, &method, response.status(), start, false);
            response
        }
        Err(e) => {
            error!("Request failed: {} {} -> {}", method, full_path, e);
            let response = error_response(e);
            record_metrics(&state, &method, response.status(), start, false);
            response
        }
    }
}

fn record_metrics(state: &ProxyState, method: &Method, status: StatusCode, start: Instant, intercepted: bool) {
    if let Some(ref metrics) = state.metrics {
        let duration = start.elapsed().as_secs_f64();
        let method_str = method.as_str();
        let status_str = status.as_str();
        let intercepted_str = if intercepted { "true" } else { "false" };
        
        metrics.requests_total
            .with_label_values(&[method_str, status_str])
            .inc();
        
        metrics.request_duration
            .with_label_values(&[method_str, intercepted_str])
            .observe(duration);
    }
}

async fn proxy_to_upstream(
    state: &ProxyState,
    method: &Method,
    path: &str,
    headers: &HeaderMap,
    body: Option<Bytes>,
    process_info: Option<&crate::process::ProcessInfo>,
) -> Result<Response, ProxyError> {
    debug!("Proxying {} request to upstream: {}", method, path);

    let upstream_client = UpstreamClient::new(Arc::clone(&state.config))
        .map_err(|e| {
            warn!("Failed to create upstream client: {}", e);
            ProxyError::UpstreamError(e.to_string())
        })?;

    // Convert headers to Vec<(String, String)>
    let mut header_vec: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str().ok().map(|val| (k.as_str().to_string(), val.to_string()))
        })
        .collect();
    
    // Inject process metadata headers if enabled
    if state.config.inject_process_headers {
        if let Some(info) = process_info {
            debug!("Injecting process metadata headers into upstream request");
            header_vec.push(("X-Forwarded-Uid".to_string(), info.uid.to_string()));
            header_vec.push(("X-Forwarded-Username".to_string(), info.username.clone()));
            header_vec.push(("X-Forwarded-Pid".to_string(), info.pid.to_string()));
            header_vec.push(("X-Forwarded-Process-Name".to_string(), info.executable.clone()));
            header_vec.push(("X-Forwarded-Process-Args".to_string(), info.cmdline.clone()));
        } else {
            debug!("inject_process_headers enabled but process metadata not available");
        }
    }

    // Convert body to Vec<u8> if present
    let body_vec = body.map(|b| b.to_vec());

    // Use the full proxy_request_full method for complete HTTP method support
    let (status, response_headers, response_body) = upstream_client
        .proxy_request_full(
            method.clone(),
            path,
            header_vec,
            body_vec,
        )
        .await
        .map_err(|e| {
            warn!("Upstream proxy request failed: {}", e);
            ProxyError::UpstreamError(e.to_string())
        })?;

    // Build response with headers
    let mut response_builder = Response::builder().status(status);
    
    for (key, value) in response_headers {
        response_builder = response_builder.header(key, value);
    }

    Ok(response_builder
        .body(Body::from(response_body))
        .unwrap())
}

/// Internal error type for proxy operations
#[derive(Debug)]
enum ProxyError {
    UpstreamError(String),
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::UpstreamError(msg) => write!(f, "Upstream error: {}", msg),
        }
    }
}

impl std::error::Error for ProxyError {}

/// Convert proxy errors to HTTP responses
fn error_response(error: ProxyError) -> Response {
    let (status, message) = match error {
        ProxyError::UpstreamError(msg) => (StatusCode::BAD_GATEWAY, msg),
    };

    (status, message).into_response()
}

/// Log request with optional process metadata
/// 
/// When process metadata is available, logs all five fields:
/// - uid: User ID
/// - username: Username (resolved from uid)
/// - pid: Process ID
/// - executable: Executable file path
/// - cmdline: Command line arguments
/// 
/// When process metadata is not available, logs request without process information
fn log_request_with_process(
    method: &Method,
    path: &str,
    status: StatusCode,
    process_info: Option<&ProcessInfo>,
) {
    if let Some(info) = process_info {
        info!(
            method = %method,
            path = %path,
            status = %status,
            uid = info.uid,
            username = %info.username,
            pid = info.pid,
            executable = %info.executable,
            cmdline = %info.cmdline,
            "Request completed with process metadata"
        );
    } else {
        info!(
            method = %method,
            path = %path,
            status = %status,
            "Request completed"
        );
    }
}

/// Log plugin response with optional process metadata
fn log_plugin_response_with_process(
    method: &Method,
    path: &str,
    status: u16,
    pattern: &str,
    process_info: Option<&ProcessInfo>,
) {
    if let Some(info) = process_info {
        info!(
            method = %method,
            path = %path,
            status = status,
            plugin = %pattern,
            uid = info.uid,
            username = %info.username,
            pid = info.pid,
            executable = %info.executable,
            cmdline = %info.cmdline,
            "Plugin response served with process metadata"
        );
    } else {
        info!(
            method = %method,
            path = %path,
            status = status,
            plugin = %pattern,
            "Plugin response served"
        );
    }
}
