pub mod command;
pub mod factory;
pub mod file;
pub mod matcher;
pub mod process_aware;

use crate::error::Result;
use crate::process::ProcessInfo;
use async_trait::async_trait;
use std::collections::HashMap;

pub use factory::PluginFactory;
pub use process_aware::ProcessAwarePlugin;
// RequestContext is already defined above, no need to re-export

/// Request context for plugins that need to access the original HTTP request
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub process_info: Option<ProcessInfo>,
}

/// Response from an interception plugin
#[derive(Debug, Clone)]
pub struct PluginResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Trait for interception plugins
/// 
/// Plugins can intercept requests and return custom responses
/// based on path patterns, process metadata, and HTTP headers.
#[async_trait]
pub trait InterceptionPlugin: Send + Sync {
    /// Check if this plugin matches the given path
    fn matches(&self, path: &str) -> bool;
    
    /// Check if this plugin matches based on process metadata and HTTP headers
    /// 
    /// This method evaluates process-aware filters (uid, username, executable, cmdline)
    /// and HTTP header filters (host pattern). Returns true if all configured filters match.
    /// 
    /// If process_info is None and process filters are configured, returns false.
    /// If Host header is missing and host_pattern is configured, returns false.
    fn matches_process_aware(
        &self,
        process_info: Option<&ProcessInfo>,
        headers: &HashMap<String, String>,
    ) -> bool;
    
    /// Get the response for a matched request
    /// 
    /// This method is called when `matches()` and `matches_process_aware()` both return true.
    /// It should return the custom response to be sent to the client.
    /// 
    /// The request_context parameter provides access to the original HTTP request details,
    /// which is needed for plugins that use proxy_request_stdin to forward the request to a command.
    async fn get_response(&self, request_context: Option<&RequestContext>) -> Result<PluginResponse>;
    
    /// Get the plugin's pattern for debugging/logging
    fn pattern(&self) -> &str;
}

/// Registry for managing multiple interception plugins
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn InterceptionPlugin>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }
    
    /// Register a plugin
    pub fn register(&mut self, plugin: Box<dyn InterceptionPlugin>) {
        self.plugins.push(plugin);
    }
    
    /// Find the first plugin that matches the given path, process metadata, and headers
    /// 
    /// Plugins are evaluated in the order they were registered.
    /// Returns the first matching plugin, or None if no plugin matches.
    pub fn find_match(
        &self,
        path: &str,
        process_info: Option<&ProcessInfo>,
        headers: &HashMap<String, String>,
    ) -> Option<&dyn InterceptionPlugin> {
        self.plugins
            .iter()
            .find(|p| p.matches(path) && p.matches_process_aware(process_info, headers))
            .map(|p| p.as_ref())
    }
    
    /// Get the number of registered plugins
    pub fn len(&self) -> usize {
        self.plugins.len()
    }
    
    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestPlugin {
        pattern: String,
        response: PluginResponse,
    }
    
    #[async_trait]
    impl InterceptionPlugin for TestPlugin {
        fn matches(&self, path: &str) -> bool {
            path == self.pattern
        }
        
        fn matches_process_aware(
            &self,
            _process_info: Option<&ProcessInfo>,
            _headers: &HashMap<String, String>,
        ) -> bool {
            // Test plugin doesn't filter by process or headers
            true
        }
        
        async fn get_response(&self, _request_context: Option<&RequestContext>) -> Result<PluginResponse> {
            Ok(self.response.clone())
        }
        
        fn pattern(&self) -> &str {
            &self.pattern
        }
    }
    
    #[test]
    fn test_plugin_registry_empty() {
        let registry = PluginRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
    
    #[test]
    fn test_plugin_registry_register() {
        let mut registry = PluginRegistry::new();
        
        let plugin = TestPlugin {
            pattern: "/test".to_string(),
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test".to_vec(),
            },
        };
        
        registry.register(Box::new(plugin));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }
    
    #[test]
    fn test_plugin_registry_find_match() {
        let mut registry = PluginRegistry::new();
        
        let plugin1 = TestPlugin {
            pattern: "/test1".to_string(),
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test1".to_vec(),
            },
        };
        
        let plugin2 = TestPlugin {
            pattern: "/test2".to_string(),
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"test2".to_vec(),
            },
        };
        
        registry.register(Box::new(plugin1));
        registry.register(Box::new(plugin2));
        
        let headers = HashMap::new();
        
        // Test matching
        assert!(registry.find_match("/test1", None, &headers).is_some());
        assert!(registry.find_match("/test2", None, &headers).is_some());
        assert!(registry.find_match("/test3", None, &headers).is_none());
    }
    
    #[test]
    fn test_plugin_registry_first_match() {
        let mut registry = PluginRegistry::new();
        
        // Register two plugins with the same pattern
        let plugin1 = TestPlugin {
            pattern: "/test".to_string(),
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"first".to_vec(),
            },
        };
        
        let plugin2 = TestPlugin {
            pattern: "/test".to_string(),
            response: PluginResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"second".to_vec(),
            },
        };
        
        registry.register(Box::new(plugin1));
        registry.register(Box::new(plugin2));
        
        let headers = HashMap::new();
        
        // Should return the first matching plugin
        let matched = registry.find_match("/test", None, &headers);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().pattern(), "/test");
    }
}
