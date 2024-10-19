use core::fmt;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};

use crate::exporters::Exporter;
use crate::process::{collect_memory_stats, describe_memory_stats};
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
        describe_memory_stats();
        let hooks: Hooks = vec![Box::new(collect_memory_stats)];
        Self { exporter, hooks }
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
        let process = metrics_process::Collector::default();
        process.describe();
        self.hooks.push(Box::new(move || process.collect()) as BoxedHook);
        self
    }

    /// Starts an endpoint at the given address to serve Prometheus metrics.
    pub async fn start(self, addr: SocketAddr) -> Result<(), Error> {
        let hooks = Arc::new(move || self.hooks.iter().for_each(|hook| hook()));

        hyper::Server::try_bind(&addr)
            .map_err(|_| Error::FailedToBindAddress { addr })?
            .serve(make_service_fn(move |_| {
                let hook = Arc::clone(&hooks);
                let exporter = self.exporter.clone();
                async move {
                    Ok::<_, Infallible>(service_fn(move |_: Request<Body>| {
                        // call the hooks to collect metrics before exporting them
                        (hook)();
                        // export the metrics from the installed exporter and send as response
                        let metrics = Body::from(exporter.export());
                        async move { Ok::<_, Infallible>(Response::new(metrics)) }
                    }))
                }
            }))
            .await?;

        Ok(())
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
