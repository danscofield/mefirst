use crate::config::Config;
use crate::error::{InterposerError, Result};
use reqwest::{Client, Method};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Hop-by-hop headers that should not be forwarded
/// These headers are specific to a single connection and should not be proxied
const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Check if a header is a hop-by-hop header that should be filtered
fn is_hop_by_hop_header(header: &str) -> bool {
    let header_lower = header.to_lowercase();
    HOP_BY_HOP_HEADERS.contains(&header_lower.as_str())
}

/// HTTP client for communicating with the upstream service
pub struct UpstreamClient {
    config: Arc<Config>,
    client: Client,
}

impl UpstreamClient {
    /// Create a new upstream client
    pub fn new(config: Arc<Config>) -> Result<Self> {
        // Build client with connection pooling
        let client = Client::builder()
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| InterposerError::Upstream(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
        })
    }

    /// Get the base URL for the upstream service
    pub fn upstream_base_url(&self) -> String {
        format!("http://{}:{}", self.config.target_address, self.config.target_port)
    }

    /// Proxy a GET request to the upstream service
    /// 
    /// This forwards a GET request to the upstream service and returns the response.
    /// All headers from the original request are forwarded transparently.
    /// 
    /// For more control over HTTP method and headers, use `proxy_request_full`.
    pub async fn proxy_request(
        &self,
        path: &str,
    ) -> Result<(reqwest::StatusCode, String)> {
        let url = format!("{}{}", self.upstream_base_url(), path);

        debug!("Proxying request to {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| InterposerError::UpstreamRequest(format!("Proxy request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| InterposerError::UpstreamResponseParse(format!("Failed to read response: {}", e)))?;

        Ok((status, body))
    }

    /// Proxy a request to the upstream service with full control over method, headers, and body
    /// 
    /// This is the most flexible proxy method that supports:
    /// - Any HTTP method (GET, PUT, POST, DELETE, etc.)
    /// - Custom request headers (automatically filters hop-by-hop headers)
    /// - Optional request body
    /// 
    /// Returns the full response including status code, headers, and body.
    /// All headers are forwarded transparently (except hop-by-hop headers).
    /// 
    /// In IP-agnostic mode (target_address is empty or 0.0.0.0), the destination
    /// is extracted from the Host header.
    /// 
    /// # Arguments
    /// * `method` - HTTP method to use
    /// * `path` - Request path (e.g., "/api/endpoint")
    /// * `headers` - Request headers to forward (hop-by-hop headers are filtered)
    /// * `body` - Optional request body
    /// 
    /// # Example
    /// ```no_run
    /// # use mefirst::upstream::UpstreamClient;
    /// # use mefirst::config::Config;
    /// # use reqwest::Method;
    /// # use std::sync::Arc;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = Arc::new(Config::default());
    /// let client = UpstreamClient::new(config)?;
    /// let headers = vec![
    ///     ("User-Agent".to_string(), "MyApp/1.0".to_string()),
    /// ];
    /// let (status, response_headers, body) = client.proxy_request_full(
    ///     Method::GET,
    ///     "/api/endpoint",
    ///     headers,
    ///     None,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn proxy_request_full(
        &self,
        method: Method,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    ) -> Result<(reqwest::StatusCode, Vec<(String, String)>, Vec<u8>)> {
        // Determine the upstream URL
        // In IP-agnostic mode (target_address is empty or 0.0.0.0), extract host from Host header
        let url = if self.config.target_address.is_empty() || self.config.target_address == "0.0.0.0" {
            // IP-agnostic mode: extract host from Host header
            let host_header = headers.iter()
                .find(|(k, _)| k.to_lowercase() == "host")
                .map(|(_, v)| v.as_str())
                .ok_or_else(|| InterposerError::UpstreamRequest(
                    "Host header required in IP-agnostic mode".to_string()
                ))?;
            
            // Parse host header to separate hostname and port
            // Host header can be "hostname" or "hostname:port"
            let (hostname, port) = if let Some(colon_pos) = host_header.rfind(':') {
                // Has port - split it
                let hostname = &host_header[..colon_pos];
                let port_str = &host_header[colon_pos + 1..];
                let port = port_str.parse::<u16>()
                    .unwrap_or(self.config.target_port);
                (hostname, port)
            } else {
                // No port - use target_port
                (host_header, self.config.target_port)
            };
            
            format!("http://{}:{}{}", hostname, port, path)
        } else {
            // IP-specific mode: use configured target address
            format!("{}{}", self.upstream_base_url(), path)
        };

        debug!("Proxying {} request to {}", method, url);

        let mut request = self.client.request(method, &url);

        // Add custom headers, filtering out hop-by-hop headers
        for (key, value) in headers {
            if !is_hop_by_hop_header(&key) {
                request = request.header(key, value);
            } else {
                debug!("Filtering hop-by-hop header: {}", key);
            }
        }

        // Add body if provided
        if let Some(body) = body {
            request = request.body(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| InterposerError::UpstreamRequest(format!("Proxy request failed: {}", e)))?;

        let status = response.status();

        // Extract response headers, filtering hop-by-hop headers
        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter(|(k, _)| !is_hop_by_hop_header(k.as_str()))
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response
            .bytes()
            .await
            .map_err(|e| InterposerError::UpstreamResponseParse(format!("Failed to read response: {}", e)))?
            .to_vec();

        Ok((status, response_headers, body))
    }

    /// Make a generic HTTP request to the upstream service with full control
    /// 
    /// This is a lower-level method that allows specifying the HTTP method,
    /// headers, and body. Useful for advanced use cases.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<String>,
    ) -> Result<(reqwest::StatusCode, Vec<(String, String)>, String)> {
        let url = format!("{}{}", self.upstream_base_url(), path);

        debug!("Making {} request to {}", method, url);

        let mut request = self.client.request(method, &url);

        // Add custom headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        // Add body if provided
        if let Some(body) = body {
            request = request.body(body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| InterposerError::UpstreamRequest(format!("Request failed: {}", e)))?;

        let status = response.status();
        
        // Extract response headers
        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| InterposerError::UpstreamResponseParse(format!("Failed to read response: {}", e)))?;

        Ok((status, response_headers, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Arc<Config> {
        Arc::new(Config {
            config_file: None,
            cgroup_path: "/sys/fs/cgroup".into(),
            target_address: "169.254.169.254".to_string(),
            target_port: 80,
            bind_port: 8080,
            enable_metrics: true,
            metrics_port: 9090,
            inject_process_headers: false,
            plugins: vec![],
        })
    }

    #[test]
    fn test_client_creation() {
        let config = create_test_config();
        let client = UpstreamClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_upstream_base_url() {
        let config = create_test_config();
        let client = UpstreamClient::new(config).unwrap();
        assert_eq!(client.upstream_base_url(), "http://169.254.169.254:80");
    }

    #[test]
    fn test_is_hop_by_hop_header() {
        // Test hop-by-hop headers
        assert!(is_hop_by_hop_header("connection"));
        assert!(is_hop_by_hop_header("Connection"));
        assert!(is_hop_by_hop_header("CONNECTION"));
        assert!(is_hop_by_hop_header("keep-alive"));
        assert!(is_hop_by_hop_header("transfer-encoding"));
        assert!(is_hop_by_hop_header("upgrade"));
        
        // Test non-hop-by-hop headers
        assert!(!is_hop_by_hop_header("content-type"));
        assert!(!is_hop_by_hop_header("user-agent"));
        assert!(!is_hop_by_hop_header("authorization"));
        assert!(!is_hop_by_hop_header("x-api-key"));
    }
}

#[cfg(test)]
#[path = "proxy.test.rs"]
mod proxy_tests;
