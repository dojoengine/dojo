//! Prometheus exporter

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use metrics_exporter_prometheus::PrometheusBuilder;
pub use metrics_exporter_prometheus::PrometheusHandle;
use metrics_util::layers::{PrefixLayer, Stack};
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::process::collect_memory_stats;
use crate::{BoxedHook, Hooks};

pub(crate) const LOG_TARGET: &str = "metrics::prometheus_exporter";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Global metrics recorder already installed.")]
    GlobalRecorderAlreadyInstalled,
    #[error("Metrics server has already been stopped.")]
    AlreadyStopped,
    #[error("Could not bind to address: {addr}")]
    FailedToBindAddress { addr: SocketAddr },
}

/// Prometheus exporter recorder.
pub struct PrometheusRecorder;

impl PrometheusRecorder {
    /// Installs Prometheus as the metrics recorder.
    ///
    /// ## Arguments
    /// * `prefix` - Apply a prefix to all metrics keys.
    pub fn install(prefix: &str) -> Result<PrometheusHandle, Error> {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();

        // Build metrics stack and install the recorder
        Stack::new(recorder)
            .push(PrefixLayer::new(prefix))
            .install()
            .map_err(|_| Error::GlobalRecorderAlreadyInstalled)?;

        Ok(handle)
    }
}

/// The handle to the metrics server.
#[derive(Debug, Clone)]
pub struct ServerHandle(Arc<watch::Sender<()>>);

impl ServerHandle {
    /// Tell the server to stop without waiting for the server to stop.
    pub fn stop(&self) -> Result<(), Error> {
        self.0.send(()).map_err(|_| Error::AlreadyStopped)
    }

    /// Wait for the server to stop.
    pub async fn stopped(self) {
        self.0.closed().await
    }

    /// Check if the server has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.0.is_closed()
    }
}

pub struct ServerBuilder {
    hooks: Hooks,
    handle: PrometheusHandle,
    tokio_runtime: Option<tokio::runtime::Handle>,
}

impl ServerBuilder {
    pub fn new(
        prefix: &'static str, // , process: metrics_process::Collector
    ) -> Result<Self, Error> {
        let handle = PrometheusRecorder::install(prefix)?;
        let hooks: Hooks = vec![Box::new(collect_memory_stats)];
        Ok(Self { handle, hooks, tokio_runtime: None })
        // let hooks: Hooks =
        //     vec![Box::new(move || process.collect()), Box::new(collect_memory_stats)];

        // // Convert the reports into hooks
        // let report_hooks =
        //     reports.into_iter().map(|r| Box::new(move || r.report()) as Box<dyn Hook<Output = ()>>);

        // hooks.extend(report_hooks);
    }

    pub fn hooks<I: IntoIterator<Item = BoxedHook<()>>>(mut self, hooks: I) -> Self {
        self.hooks.extend(hooks);
        self
    }

    /// Set a custom tokio runtime to use for the server.
    ///
    /// Otherwise, it will use the ambient runtime.
    pub fn with_tokio_runtime(mut self, rt: Handle) -> Self {
        self.tokio_runtime = Some(rt);
        self
    }

    /// Starts an endpoint at the given address to serve Prometheus metrics.
    pub async fn start(self, addr: SocketAddr) -> Result<ServerHandle, Error> {
        let (tx, mut rx) = watch::channel(());
        let hooks = Arc::new(move || self.hooks.iter().for_each(|hook| hook()));

        let make_svc = make_service_fn(move |_| {
            let handle = self.handle.clone();
            let hook = Arc::clone(&hooks);
            async move {
                Ok::<_, Infallible>(service_fn(move |_: Request<Body>| {
                    (hook)();
                    let metrics = handle.render();
                    async move { Ok::<_, Infallible>(Response::new(Body::from(metrics))) }
                }))
            }
        });

        let server = Server::try_bind(&addr)
            .map_err(|_| Error::FailedToBindAddress { addr })?
            .serve(make_svc)
            .with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            });

        let fut = async move { server.await.expect("Metrics endpoint crashed") };

        if let Some(rt) = self.tokio_runtime {
            rt.spawn(fut);
        } else {
            tokio::spawn(fut);
        }

        Ok(ServerHandle(Arc::new(tx)))
    }
}
