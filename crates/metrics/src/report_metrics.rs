//! Adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/blob/main/crates/storage/db/src/abstraction/database_metrics.rs)


use metrics::{Label, absolute_counter, gauge, histogram};

/// Represents a type that can report metrics. The `report_metrics`
/// method can be used as a prometheus hook.
pub trait ReportMetrics : Send + Sync + 'static {
    /// Reports metrics.
    fn report_metrics(&self) {
        for (name, value, labels) in self.gauge_metrics() {
            gauge!(name, value, labels);
        }

        for (name, value, labels) in self.counter_metrics() {
            absolute_counter!(name, value, labels);
        }

        for (name, value, labels) in self.histogram_metrics() {
            histogram!(name, value, labels);
        }
    }

     /// Returns a list of [Gauge](metrics::Gauge) metrics for the database.
     fn gauge_metrics(&self) -> Vec<(&'static str, f64, Vec<Label>)> {
        vec![]
    }

    /// Returns a list of [Counter](metrics::Counter) metrics for the database.
    fn counter_metrics(&self) -> Vec<(&'static str, u64, Vec<Label>)> {
        vec![]
    }

    /// Returns a list of [Histogram](metrics::Histogram) metrics for the database.
    fn histogram_metrics(&self) -> Vec<(&'static str, f64, Vec<Label>)> {
        vec![]
    }
}