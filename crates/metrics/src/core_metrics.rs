//! Core pool metrics.
// This is an example of metrics implementation.

use std::sync::atomic::{AtomicU64, Ordering};
use metrics::{counter, Label};
use crate::report_metrics::ReportMetrics;

pub struct PoolMetrics {
    pub inserted_transactions: AtomicU64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            inserted_transactions: AtomicU64::new(0),
        }
    }
}

impl ReportMetrics for PoolMetrics {
    fn report_metrics(&self) {
        for (name, value, labels) in self.counter_metrics() {
            counter!(name, value, labels);
    }
}

    fn counter_metrics(&self) -> Vec<(&'static str, u64, Vec<Label>)> {
        let mut metrics = Vec::new();
        
        metrics.push((
            "inserted_transactions",
            self.inserted_transactions.load(Ordering::SeqCst),
            vec![],
        ));
        
        metrics
    }
}

impl std::fmt::Debug for PoolMetrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoolMetrics").finish()
    }
}

//impl PoolMetrics {
//    pub fn describe() {
//         metrics::describe_gauge!(
//            "metrics_custom_gauge",
//            "A gauge with doc comment description."
//        );
//    }
//}