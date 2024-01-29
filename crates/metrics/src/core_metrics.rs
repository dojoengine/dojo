//! Core pool metrics.
// This is an example of metrics implementation.

use dojo_metrics_derive::Metrics;
use crate::report_metrics::ReportMetrics;

#[derive(Metrics)]
#[metrics(scope = "pool")]
pub struct PoolMetrics {
    // Describe method on the struct, which internally calls the describe statements for all metric fields.
    #[metric(describe = "Number of transactions inserted in the pool.")]
    pub inserted_transactions: metrics::Counter,
}

impl ReportMetrics for PoolMetrics {
    fn report_metrics(&self) {
        self.counter_metrics();
    }

    fn counter_metrics(&self) -> Vec<&metrics::Counter> {
        let mut metrics = Vec::new();
        
        let inserted_transactions = &self.inserted_transactions;

        metrics.push(inserted_transactions);

        metrics
    }
}