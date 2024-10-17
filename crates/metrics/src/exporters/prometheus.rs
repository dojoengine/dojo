//! Prometheus exporter

use std::sync::OnceLock;

use metrics_exporter_prometheus::PrometheusBuilder;
pub use metrics_exporter_prometheus::PrometheusHandle as Prometheus;
use metrics_util::layers::{PrefixLayer, Stack};
use tracing::info;

use crate::{Error, Exporter};

static PROMETHEUS_HANDLE: OnceLock<Prometheus> = OnceLock::new();

/// Prometheus exporter recorder.
#[derive(Debug)]
pub struct PrometheusRecorder;

impl PrometheusRecorder {
    /// Installs Prometheus as the metrics recorder.
    ///
    /// ## Arguments
    ///
    /// * `prefix` - Apply a prefix to all metrics keys.
    pub fn install(prefix: &str) -> Result<Prometheus, Error> {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();

        // Build metrics stack and install the recorder
        Stack::new(recorder)
            .push(PrefixLayer::new(prefix))
            .install()
            .map_err(|_| Error::GlobalRecorderAlreadyInstalled)?;

        info!(target: "metrics", %prefix, "Prometheus recorder installed.");

        let _ = PROMETHEUS_HANDLE.set(handle.clone());

        Ok(handle)
    }

    pub fn current() -> Option<Prometheus> {
        PROMETHEUS_HANDLE.get().cloned()
    }
}

impl Exporter for Prometheus {
    fn export(&self) -> String {
        self.render()
    }
}
