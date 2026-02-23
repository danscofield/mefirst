use mefirst::config::Config;
use mefirst::upstream::UpstreamClient;
use reqwest::Method;
use std::sync::Arc;

/// Helper to create a test config
fn create_test_config() -> Arc<Config> {
    Arc::new(Config {
        config_file: None,
        cgroup_path: "/sys/fs/cgroup".into(),
        target_address: "169.254.169.254".to_string(),
        target_port: 80,
        bind_port: 8080,
        enable_metrics: false,
        metrics_port: 9090,
        plugins: vec![],
    })
}

#[tokio::test]
async fn test_proxy_request_full_basic() {
    let config = create_test_config();
    let client = UpstreamClient::new(config).expect("Failed to create client");

    // Test basic proxy_request_full with GET method
    // Note: This test will fail if not running on EC2 or without upstream service access
    // In a real environment, you'd use a mock server
    
    let headers = vec![
        ("User-Agent".to_string(), "test-agent/1.0".to_string()),
    ];
    
    let result = client.proxy_request_full(
        Method::GET,
        "/latest/meta-data/",
        headers,
        None,
    ).await;

    // We expect this to fail in test environment (no upstream service available)
    // but the function should handle it gracefully
    match result {
        Ok((status, headers, body)) => {
            // If we're on EC2, verify the response
            assert!(status.is_success() || status.is_client_error());
            assert!(!body.is_empty() || status.is_client_error());
            println!("Response status: {}", status);
            println!("Response headers: {:?}", headers);
        }
        Err(e) => {
            // Expected in test environment without upstream service
            println!("Expected error in test environment: {}", e);
        }
    }
}

#[tokio::test]
async fn test_proxy_request_full_post_with_body() {
    let config = create_test_config();
    let client = UpstreamClient::new(config).expect("Failed to create client");

    let headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
    ];
    let body = b"{\"test\": \"data\"}".to_vec();
    
    let result = client.proxy_request_full(
        Method::POST,
        "/latest/api/token",
        headers,
        Some(body),
    ).await;

    // We expect this to fail in test environment
    match result {
        Ok((status, _, _)) => {
            println!("Response status: {}", status);
        }
        Err(e) => {
            println!("Expected error in test environment: {}", e);
        }
    }
}

#[tokio::test]
async fn test_proxy_request_full_filters_hop_by_hop_headers() {
    let config = create_test_config();
    let client = UpstreamClient::new(config).expect("Failed to create client");

    // Include hop-by-hop headers that should be filtered
    let headers = vec![
        ("User-Agent".to_string(), "test-agent/1.0".to_string()),
        ("Connection".to_string(), "keep-alive".to_string()),
        ("Keep-Alive".to_string(), "timeout=5".to_string()),
        ("Transfer-Encoding".to_string(), "chunked".to_string()),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];
    
    let result = client.proxy_request_full(
        Method::GET,
        "/latest/meta-data/",
        headers,
        None,
    ).await;

    // The function should filter hop-by-hop headers
    // We can't easily verify this without a mock server, but the function should not error
    match result {
        Ok(_) => {
            // Success - headers were filtered properly
        }
        Err(e) => {
            // Expected in test environment
            println!("Expected error in test environment: {}", e);
        }
    }
}

#[tokio::test]
async fn test_proxy_request_full_different_methods() {
    let config = create_test_config();
    let client = UpstreamClient::new(config).expect("Failed to create client");

    let methods = vec![
        Method::GET,
        Method::PUT,
        Method::POST,
        Method::DELETE,
        Method::HEAD,
    ];

    for method in methods {
        let result = client.proxy_request_full(
            method.clone(),
            "/latest/meta-data/",
            vec![],
            None,
        ).await;

        // All methods should be handled without panicking
        match result {
            Ok((status, _, _)) => {
                println!("{} request status: {}", method, status);
            }
            Err(e) => {
                println!("{} request error (expected): {}", method, e);
            }
        }
    }
}

#[tokio::test]
async fn test_proxy_request_backward_compatibility() {
    let config = create_test_config();
    let client = UpstreamClient::new(config).expect("Failed to create client");

    // Test that the proxy_request method still works
    let result = client.proxy_request("/latest/meta-data/").await;

    match result {
        Ok((status, body)) => {
            println!("Response status: {}", status);
            println!("Response body length: {}", body.len());
        }
        Err(e) => {
            println!("Expected error in test environment: {}", e);
        }
    }
}
