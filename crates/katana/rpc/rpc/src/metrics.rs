//! This module is responsible for managing and collecting metrics related to the RPC
//! server. The metrics collected are primarily focused on connections and method calls.
//!
//! ## Connections
//!
//! Metrics related to connections:
//!
//! - Number of connections opened
//! - Number of connections closed
//! - Number of requests started
//! - Number of requests finished
//! - Response time for each request/response pair
//!
//! ## Method Calls
//!
//! Metrics are collected for each methods expose by the RPC server. The metrics collected include:
//!
//! - Number of calls started for each method
//! - Number of successful calls for each method
//! - Number of failed calls for each method
//! - Response time for each method call

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use dojo_metrics::metrics::{Counter, Histogram};
use dojo_metrics::Metrics;
use jsonrpsee::server::logger::{HttpRequest, Logger, MethodKind, Params, TransportProtocol};
use jsonrpsee::RpcModule;
use tracing::debug;

/// Metrics for the RPC server.
#[allow(missing_debug_implementations)]
#[derive(Default, Clone)]
pub struct RpcServerMetrics {
    inner: Arc<RpcServerMetricsInner>,
}

impl RpcServerMetrics {
    /// Creates a new instance of `RpcServerMetrics` for the given `RpcModule`.
    /// This will create metrics for each method in the module.
    pub fn new(module: &RpcModule<()>) -> Self {
        let call_metrics = HashMap::from_iter(module.method_names().map(|method| {
            let metrics = RpcServerCallMetrics::new_with_labels(&[("method", method)]);
            (method, metrics)
        }));

        Self {
            inner: Arc::new(RpcServerMetricsInner {
                call_metrics,
                connection_metrics: ConnectionMetrics::default(),
            }),
        }
    }
}

#[derive(Default, Clone)]
struct RpcServerMetricsInner {
    /// Connection metrics per transport type
    connection_metrics: ConnectionMetrics,
    /// Call metrics per RPC method
    call_metrics: HashMap<&'static str, RpcServerCallMetrics>,
}

#[derive(Clone)]
struct ConnectionMetrics {
    /// Metrics for WebSocket connections
    ws: RpcServerConnectionMetrics,
    /// Metrics for HTTP connections
    http: RpcServerConnectionMetrics,
}

impl ConnectionMetrics {
    /// Returns the metrics for the given transport protocol
    fn get_metrics(&self, transport: TransportProtocol) -> &RpcServerConnectionMetrics {
        match transport {
            TransportProtocol::Http => &self.http,
            TransportProtocol::WebSocket => &self.ws,
        }
    }
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self {
            ws: RpcServerConnectionMetrics::new_with_labels(&[("transport", "ws")]),
            http: RpcServerConnectionMetrics::new_with_labels(&[("transport", "http")]),
        }
    }
}

/// Metrics for the RPC connections
#[derive(Metrics, Clone)]
#[metrics(scope = "rpc_server.connections")]
struct RpcServerConnectionMetrics {
    /// The number of connections opened
    connections_opened: Counter,
    /// The number of connections closed
    connections_closed: Counter,
    /// The number of requests started
    requests_started: Counter,
    /// The number of requests finished
    requests_finished: Counter,
    /// Response for a single request/response pair
    request_time_seconds: Histogram,
}

/// Metrics for the RPC calls
#[derive(Metrics, Clone)]
#[metrics(scope = "rpc_server.calls")]
struct RpcServerCallMetrics {
    /// The number of calls started
    started: Counter,
    /// The number of successful calls
    successful: Counter,
    /// The number of failed calls
    failed: Counter,
    /// Response for a single call
    time_seconds: Histogram,
}

/// Implements the [Logger] trait so that we can collect metrics on each server request life-cycle.
impl Logger for RpcServerMetrics {
    type Instant = Instant;

    fn on_connect(&self, _: SocketAddr, _: &HttpRequest, transport: TransportProtocol) {
        self.inner.connection_metrics.get_metrics(transport).connections_opened.increment(1)
    }

    fn on_request(&self, transport: TransportProtocol) -> Self::Instant {
        self.inner.connection_metrics.get_metrics(transport).requests_started.increment(1);
        Instant::now()
    }

    fn on_call(&self, method_name: &str, _: Params<'_>, _: MethodKind, _: TransportProtocol) {
        debug!(target: "server", method = ?method_name);
        let Some(call_metrics) = self.inner.call_metrics.get(method_name) else { return };
        call_metrics.started.increment(1);
    }

    fn on_result(
        &self,
        method_name: &str,
        success: bool,
        started_at: Self::Instant,
        _: TransportProtocol,
    ) {
        let Some(call_metrics) = self.inner.call_metrics.get(method_name) else { return };

        // capture call latency
        let time_taken = started_at.elapsed().as_secs_f64();
        call_metrics.time_seconds.record(time_taken);

        if success {
            call_metrics.successful.increment(1);
        } else {
            call_metrics.failed.increment(1);
        }
    }

    fn on_response(&self, _: &str, started_at: Self::Instant, transport: TransportProtocol) {
        let metrics = self.inner.connection_metrics.get_metrics(transport);
        // capture request latency for this request/response pair
        let time_taken = started_at.elapsed().as_secs_f64();
        metrics.request_time_seconds.record(time_taken);
        metrics.requests_finished.increment(1);
    }

    fn on_disconnect(&self, _: SocketAddr, transport: TransportProtocol) {
        self.inner.connection_metrics.get_metrics(transport).connections_closed.increment(1)
    }
}
