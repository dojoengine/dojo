//! Adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/blob/main/crates/storage/db/src/abstraction/database_metrics.rs)

use metrics::{Counter, Gauge, Histogram};

/// Represents a type that can report metrics. The `report_metrics`
/// method can be used as a prometheus hook.
pub trait ReportMetrics: Send + Sync + 'static {
    /// Reports metrics.
    fn report_metrics(&self) {
        self.gauge_metrics();
        self.counter_metrics();
        self.histogram_metrics();
    }

    /// Returns a list of [Gauge](metrics::Gauge) metrics for the database.
    fn gauge_metrics(&self) -> Vec<&'static Gauge> {
        vec![]
    }

    /// Returns a list of [Counter](metrics::Counter) metrics for the database.
    fn counter_metrics(&self) -> Vec<&Counter> {
        vec![] 
    }

    /// Returns a list of [Histogram](metrics::Histogram) metrics for the database.
    fn histogram_metrics(&self) -> Vec<&'static Histogram> {
        vec![]
    }
}
