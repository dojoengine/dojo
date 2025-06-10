use core::fmt;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::exporters::Exporter;
use crate::{Error, Report};

/// A helper trait for defining the type for hooks that are called when the metrics are being
/// collected by the server.
trait Hook: Fn() + Send + Sync {}
impl<T: Fn() + Send + Sync> Hook for T {}

/// A boxed [`Hook`].
type BoxedHook = Box<dyn Hook<Output = ()>>;
/// A list of [BoxedHook].
type Hooks = Vec<BoxedHook>;

/// Server for serving metrics.
// TODO: allow configuring the server executor to allow cancelling on invidiual connection tasks.
// See, [hyper::server::server::Builder::executor]
pub struct Server<MetricsExporter> {
    /// Hooks or callable functions for collecting metrics in the cases where
    /// the metrics are not being collected in the main program flow.
    ///
    /// These are called when metrics are being served through the server.
    hooks: Hooks,
    /// The exporter that is used to export the collected metrics.
    exporter: MetricsExporter,
}

impl<MetricsExporter> Server<MetricsExporter>
where
    MetricsExporter: Exporter + 'static,
{
    /// Creates a new metrics server using the given exporter.
    pub fn new(exporter: MetricsExporter) -> Self {
        Self { exporter, hooks: Vec::new() }
    }

    /// Add new metrics reporter to the server.
    pub fn with_reports<I>(mut self, reports: I) -> Self
    where
        I: IntoIterator<Item = Box<dyn Report>>,
    {
        // convert the report types into callable hooks
        let hooks = reports.into_iter().map(|r| Box::new(move || r.report()) as BoxedHook);
        self.hooks.extend(hooks);
        self
    }

    pub fn with_process_metrics(mut self) -> Self {
        use crate::process::{collect_memory_stats, describe_memory_stats};

        let process = metrics_process::Collector::default();
        process.describe();
        describe_memory_stats();

        let hooks: Hooks =
            vec![Box::new(collect_memory_stats), Box::new(move || process.collect())];

        self.hooks.extend(hooks);
        self
    }

    /// Starts an endpoint at the given address to serve Prometheus metrics.
    pub async fn start(self, addr: SocketAddr) -> Result<(), Error> {
        let hooks = Arc::new(move || self.hooks.iter().for_each(|hook| hook()));

        // Bind to the port and listen for incoming TCP connections
        let listener = TcpListener::bind(addr).await
            .map_err(|_| Error::FailedToBindAddress { addr })?;

        loop {
            // Accept incoming TCP connections
            let (tcp, _) = listener.accept().await
                .map_err(|_| Error::FailedToBindAddress { addr })?;

            // Use an adapter to access something implementing `tokio::io` traits as if they implement
            // `hyper::rt` IO traits.
            let io = TokioIo::new(tcp);

            // Clone the hooks and exporter for each connection
            let hooks = Arc::clone(&hooks);
            let exporter = self.exporter.clone();

            // Spawn a new task to handle each connection
            tokio::task::spawn(async move {
                // Handle the connection using HTTP1
                if let Err(err) = http1::Builder::new()
                    .serve_connection(io, service_fn(move |_: Request<hyper::body::Incoming>| {
                        // call the hooks to collect metrics before exporting them
                        (hooks)();
                        // export the metrics from the installed exporter and send as response
                        let metrics = Full::new(Bytes::from(exporter.export()));
                        async move { Ok::<_, Infallible>(Response::new(metrics)) }
                    }))
                    .await
                {
                    eprintln!("Error serving connection: {:?}", err);
                }
            });
        }
    }
}

impl<MetricsExporter> fmt::Debug for Server<MetricsExporter>
where
    MetricsExporter: fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server").field("hooks", &"...").field("exporter", &self.exporter).finish()
    }
}
