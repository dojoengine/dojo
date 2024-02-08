//! Core pool metrics.
// This is an example of metrics implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use metrics::{absolute_counter, Label};
use crate::report_metrics::ReportMetrics;

pub struct PoolMetrics {
    pub pool_inserted_transactions: AtomicU64,
    pub pool_removed_transactions: AtomicU64,
    pub pool_invalid_transactions: AtomicU64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            pool_inserted_transactions: AtomicU64::new(0),
            pool_removed_transactions: AtomicU64::new(0),
            pool_invalid_transactions: AtomicU64::new(0),
        }
    }
}

impl std::fmt::Debug for PoolMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolMetrics").finish()
    }
}

impl ReportMetrics for PoolMetrics {
    fn report_metrics(&self) {
        for (name, value, labels) in self.counter_metrics() {
            absolute_counter!(name, value, labels);
        }
    }

    fn counter_metrics(&self) -> Vec<(&'static str, u64, Vec<Label>)> {
        let mut metrics = Vec::new();

        metrics.push((
            "pool_inserted_transactions",
            self.pool_inserted_transactions.load(Ordering::SeqCst),
            vec![],
        ));
        
        metrics.push((
            "pool_removed_transactions",
            self.pool_removed_transactions.load(Ordering::Acquire),
            vec![],
        ));

        metrics.push((
            "pool_invalid_transactions",
            self.pool_invalid_transactions.load(Ordering::Acquire),
            vec![],
        ));
        
        metrics
    }
}