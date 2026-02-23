// Metrics module for Prometheus metrics

use prometheus::{CounterVec, HistogramOpts, HistogramVec, Opts, Registry};

pub struct Metrics {
    pub requests_total: CounterVec,
    pub request_duration: HistogramVec,
    pub plugin_hits: CounterVec,
    pub plugin_errors: CounterVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let requests_total = CounterVec::new(
            Opts::new("proxy_requests_total", "Total number of proxy requests"),
            &["method", "status"],
        )
        .expect("metric can be created");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("metric can be registered");

        let request_duration = HistogramVec::new(
            HistogramOpts::new("proxy_request_duration_seconds", "Proxy request duration"),
            &["method", "intercepted"],
        )
        .expect("metric can be created");
        registry
            .register(Box::new(request_duration.clone()))
            .expect("metric can be registered");

        let plugin_hits = CounterVec::new(
            Opts::new("proxy_plugin_hits_total", "Total plugin interceptions"),
            &["pattern"],
        )
        .expect("metric can be created");
        registry
            .register(Box::new(plugin_hits.clone()))
            .expect("metric can be registered");

        let plugin_errors = CounterVec::new(
            Opts::new("proxy_plugin_errors_total", "Total plugin errors"),
            &["pattern"],
        )
        .expect("metric can be created");
        registry
            .register(Box::new(plugin_errors.clone()))
            .expect("metric can be registered");

        Self {
            requests_total,
            request_duration,
            plugin_hits,
            plugin_errors,
        }
    }
}
