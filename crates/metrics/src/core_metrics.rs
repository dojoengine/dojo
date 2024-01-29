//! Core pool metrics.

use dojo_metrics_derive::Metrics;
use crate::report_metrics::ReportMetrics;

#[derive(Metrics)]
#[metrics(scope = "core")]
pub struct PoolMetrics {
    /// Some doc comment
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