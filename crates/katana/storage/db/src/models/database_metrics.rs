//! Adapted from Paradigm's [`reth`](https://github.com/paradigmxyz/reth/blob/main/crates/storage/db/src/abstraction/database_metrics.rs)

use metrics::{counter, gauge, histogram, Label};
use std::sync::Arc;

/// Represents a type that can report metrics, used mainly with the database. The `report_metrics`
/// method can be used as a prometheus hook.
pub trait DbMetrics {
    /// Reports metrics for the database.
    fn report_metrics(&self) {
        for (name, value, labels) in self.gauge_metrics() {
            gauge!(name, value, labels);
        }

        for (name, value, labels) in self.counter_metrics() {
            counter!(name, value, labels);
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

impl<DB: DbMetrics> DbMetrics for Arc<DB> {
    fn report_metrics(&self) {
        <DB as DbMetrics>::report_metrics(self)
    }
}

/// The type used to store metadata about the database.
#[derive(Debug, Default)]
pub struct DbMetadataValue {
    /// The freelist size
    freelist_size: Option<usize>,
}

impl DbMetadataValue {
    /// Creates a new [DatabaseMetadataValue] with the given freelist size.
    pub fn new(freelist_size: Option<usize>) -> Self {
        Self { freelist_size }
    }

    /// Returns the freelist size, if available.
    pub fn freelist_size(&self) -> Option<usize> {
        self.freelist_size
    }
}

/// Includes a method to return a [DatabaseMetadataValue] type, which can be used to dynamically
/// retrieve information about the database.
pub trait DbMetadata {
    /// Returns a metadata type, [DatabaseMetadataValue] for the database.
    fn metadata(&self) -> DbMetadataValue;
}

impl<DB: DbMetadata> DbMetadata for Arc<DB> {
    fn metadata(&self) -> DbMetadataValue {
        <DB as DbMetadata>::metadata(self)
    }
}
